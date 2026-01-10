//! TUI application state and main run loop

mod data;
mod keys;
mod state;
mod tabs;

use std::io;

use crossbeam_channel::{Receiver, Sender, select};
use crossterm::event::KeyCode;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, TableState, Wrap},
};
use tui_input::Input;

use mevlog::{ChainEntryJson, misc::shared_init::ConnOpts};

use crate::cmd::tui::{
    app::keys::spawn_input_reader,
    data::{
        BlockId, CallExtract, DataRequest, DataResponse, MEVOpcodeJson, MEVTransactionJson,
        RpcOpts, TraceMode, worker::spawn_data_worker,
    },
    views::{
        NetworkSelector, SearchView, StatusBar, TabBar, TxsTable, render_info_popup,
        render_key_bindings, render_tx_popup,
    },
};

const DEFAULT_CHAINS: [(u64, &str, &str, &str); 10] = [
    (1, "Ethereum Mainnet", "ETH", "https://etherscan.io"),
    (10, "OP Mainnet", "ETH", "https://optimistic.etherscan.io"),
    (56, "BNB Smart Chain Mainnet", "BSC", "https://bscscan.com"),
    (130, "Unichain", "ETH", "https://unichain.blockscout.com"),
    (137, "Polygon Mainnet", "Polygon", "https://polygonscan.com"),
    (324, "zkSync Mainnet", "ETH", "https://era.zksync.network"),
    (8453, "Base", "ETH", "https://basescan.org"),
    (42161, "Arbitrum One", "ETH", "https://arbiscan.io"),
    (43114, "Avalanche C-Chain", "AVAX", "https://snowtrace.io"),
    (534352, "Scroll Mainnet", "ETH", "https://scrollscan.com"),
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AppMode {
    SelectNetwork,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PrimaryTab {
    Explore,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum TxPopupTab {
    #[default]
    Info,
    Opcodes,
    Traces,
    Transfers,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum AppEvent {
    Key(KeyCode),
    Data(DataResponse),
}

pub struct App {
    pub(crate) table_state: TableState,
    pub(crate) items: Vec<MEVTransactionJson>,
    pub(crate) current_block: Option<u64>,
    pub(crate) is_loading: bool,
    pub(crate) loading_block: Option<u64>,
    pub(crate) error_message: Option<String>,
    data_req_tx: Sender<DataRequest>,
    state_rx: Receiver<AppEvent>,
    exit: bool,
    pub(crate) mode: AppMode,
    pub(crate) network_table_state: TableState,
    pub(crate) available_chains: Vec<ChainEntryJson>,
    pub(crate) search_query: String,
    pub(crate) search_popup_open: bool,
    pub(crate) tx_popup_open: bool,
    pub(crate) tx_popup_scroll: u16,
    pub(crate) tx_popup_tab: TxPopupTab,
    pub(crate) rpc_url: Option<String>,
    pub(crate) chain_id: Option<u64>,
    pub(crate) rpc_timeout_ms: u64,
    pub(crate) block_timeout_ms: u64,
    pub(crate) active_tab: PrimaryTab,
    pub(crate) selected_chain: Option<ChainEntryJson>,
    #[allow(dead_code)]
    state_tx: Sender<AppEvent>,
    pub(crate) opcodes: Option<Vec<MEVOpcodeJson>>,
    pub(crate) opcodes_loading: bool,
    pub(crate) opcodes_tx_hash: Option<String>,
    pub(crate) traces: Option<Vec<CallExtract>>,
    pub(crate) traces_loading: bool,
    pub(crate) traces_tx_hash: Option<String>,
    pub(crate) tx_trace_loading: bool,
    pub(crate) tx_trace_hash: Option<String>,
    pub(crate) block_input_popup_open: bool,
    pub(crate) block_input_query: String,
    pub(crate) trace_mode: Option<TraceMode>,
    pub(crate) info_popup_open: bool,
    pub(crate) rpc_refreshing: bool,
    pub(crate) filter_blocks: Input,
    pub(crate) filter_position: Input,
    pub(crate) filter_from: Input,
    pub(crate) filter_to: Input,
    pub(crate) filter_event: Input,
    pub(crate) filter_not_event: Input,
    pub(crate) filter_method: Input,
    pub(crate) filter_erc20_transfer: Input,
    pub(crate) filter_tx_cost: Input,
    pub(crate) filter_gas_price: Input,
    pub(crate) search_active_field: usize,
    pub(crate) search_editing: bool,
}

impl App {
    pub fn new(items: Vec<MEVTransactionJson>, conn_opts: &ConnOpts) -> Self {
        let current_block = items.first().map(|tx| tx.block_number);

        let (data_req_tx, data_req_rx) = crossbeam_channel::unbounded();
        let (state_tx, state_rx) = crossbeam_channel::unbounded();

        let mode = if conn_opts.rpc_url.is_none() && conn_opts.chain_id.is_none() {
            AppMode::SelectNetwork
        } else {
            AppMode::Main
        };

        let selected_chain = conn_opts.chain_id.and_then(|chain_id| {
            DEFAULT_CHAINS
                .iter()
                .find(|(id, _, _, _)| *id == chain_id)
                .map(|(id, name, chain, explorer)| ChainEntryJson {
                    chain_id: *id,
                    name: name.to_string(),
                    chain: chain.to_string(),
                    explorer_url: Some(explorer.to_string()),
                })
        });

        let rpc_url = conn_opts.rpc_url.clone();
        let chain_id = conn_opts.chain_id;
        let rpc_timeout_ms = conn_opts.rpc_timeout_ms;
        let block_timeout_ms = conn_opts.block_timeout_ms;

        spawn_data_worker(data_req_rx, state_tx.clone());
        spawn_input_reader(state_tx.clone());

        if mode == AppMode::Main {
            if let Some(ref url) = rpc_url {
                if let Some(cid) = chain_id {
                    let opts = RpcOpts {
                        rpc_url: url.clone(),
                        chain_id: cid,
                        block_timeout_ms,
                    };
                    let _ = data_req_tx.send(DataRequest::Block(BlockId::Latest, opts));
                }
                let _ = data_req_tx.send(DataRequest::DetectTraceMode(url.clone()));
                if chain_id.is_none() {
                    let _ = data_req_tx.send(DataRequest::ChainInfo(url.clone()));
                }
            } else if let Some(cid) = chain_id {
                let _ = data_req_tx.send(DataRequest::RefreshRpc(cid, rpc_timeout_ms));
            }
        }

        let available_chains = if mode == AppMode::SelectNetwork {
            DEFAULT_CHAINS
                .iter()
                .map(|(id, name, chain, explorer)| ChainEntryJson {
                    chain_id: *id,
                    name: name.to_string(),
                    chain: chain.to_string(),
                    explorer_url: Some(explorer.to_string()),
                })
                .collect()
        } else {
            vec![]
        };

        let rpc_refreshing = mode == AppMode::Main && rpc_url.is_none() && chain_id.is_some();

        Self {
            table_state: TableState::default().with_selected(if items.is_empty() {
                None
            } else {
                Some(0)
            }),
            items,
            current_block,
            is_loading: mode == AppMode::Main,
            loading_block: None,
            error_message: None,
            data_req_tx,
            state_rx,
            exit: false,
            mode,
            network_table_state: TableState::default().with_selected(Some(0)),
            available_chains,
            search_query: String::new(),
            search_popup_open: false,
            tx_popup_open: false,
            tx_popup_scroll: 0,
            tx_popup_tab: TxPopupTab::default(),
            rpc_url,
            chain_id,
            rpc_timeout_ms,
            block_timeout_ms,
            active_tab: PrimaryTab::Explore,
            selected_chain,
            state_tx,
            opcodes: None,
            opcodes_loading: false,
            opcodes_tx_hash: None,
            traces: None,
            traces_loading: false,
            traces_tx_hash: None,
            tx_trace_loading: false,
            tx_trace_hash: None,
            block_input_popup_open: false,
            block_input_query: String::new(),
            trace_mode: None,
            info_popup_open: false,
            rpc_refreshing,
            filter_blocks: Input::default(),
            filter_position: Input::default(),
            filter_from: Input::default(),
            filter_to: Input::default(),
            filter_event: Input::default(),
            filter_not_event: Input::default(),
            filter_method: Input::default(),
            filter_erc20_transfer: Input::default(),
            filter_tx_cost: Input::default(),
            filter_gas_price: Input::default(),
            search_active_field: 0,
            search_editing: false,
        }
    }

    pub(crate) fn rpc_opts(&self) -> Option<RpcOpts> {
        Some(RpcOpts {
            rpc_url: self.rpc_url.clone()?,
            chain_id: self.chain_id?,
            block_timeout_ms: self.block_timeout_ms,
        })
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        match self.mode {
            AppMode::SelectNetwork => {
                // Split area into content and key bindings footer
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(0),    // Content area
                        Constraint::Length(3), // Key bindings footer
                    ])
                    .split(frame.area());

                NetworkSelector::new(&self.available_chains, &self.search_query, self.is_loading)
                    .render(
                        chunks[0],
                        frame,
                        &mut self.network_table_state,
                        self.search_popup_open,
                    );

                // Render key bindings footer
                render_key_bindings(
                    frame,
                    chunks[1],
                    &self.mode,
                    None,
                    self.search_popup_open,
                    false,
                    TxPopupTab::default(),
                    false,
                    false,
                    self.can_return_to_main(),
                );

                if let Some(error_msg) = &self.error_message {
                    self.render_error_popup(frame, error_msg);
                }
            }
            AppMode::Main => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // Tab bar (no border)
                        Constraint::Length(3), // Status bar
                        Constraint::Min(0),    // Content area
                        Constraint::Length(3), // Key bindings footer
                    ])
                    .split(frame.area());

                TabBar::new(self.active_tab).render(chunks[0], frame);

                StatusBar::new(
                    self.selected_chain.as_ref(),
                    self.current_block,
                    self.is_loading,
                    self.loading_block,
                    self.trace_mode.as_ref(),
                )
                .render(chunks[1], frame);

                match self.active_tab {
                    PrimaryTab::Explore => {
                        TxsTable::new(&self.items).render(chunks[2], frame, &mut self.table_state);

                        if self.tx_popup_open
                            && let Some(idx) = self.table_state.selected()
                            && let Some(tx) = self.items.get(idx)
                        {
                            let explorer_url = self
                                .selected_chain
                                .as_ref()
                                .and_then(|c| c.explorer_url.clone());
                            render_tx_popup(
                                tx,
                                frame.area(),
                                frame,
                                self.tx_popup_scroll,
                                self.tx_popup_tab,
                                explorer_url.as_deref(),
                                self.opcodes.as_deref(),
                                self.opcodes_loading,
                                self.traces.as_deref(),
                                self.traces_loading,
                                self.tx_trace_loading,
                            );
                        }

                        if self.block_input_popup_open {
                            self.render_block_input_popup(frame);
                        } else if self.info_popup_open {
                            render_info_popup(
                                frame.area(),
                                frame,
                                self.selected_chain.as_ref(),
                                self.rpc_url.as_deref(),
                                self.rpc_refreshing,
                            );
                        }
                    }
                    PrimaryTab::Search => {
                        SearchView::new(
                            &[
                                &self.filter_blocks,
                                &self.filter_position,
                                &self.filter_from,
                                &self.filter_to,
                                &self.filter_event,
                                &self.filter_not_event,
                                &self.filter_method,
                                &self.filter_erc20_transfer,
                                &self.filter_tx_cost,
                                &self.filter_gas_price,
                            ],
                            self.search_active_field,
                            self.search_editing,
                        )
                        .render(chunks[2], frame);
                    }
                }

                render_key_bindings(
                    frame,
                    chunks[3],
                    &self.mode,
                    Some(self.active_tab),
                    false,
                    self.tx_popup_open,
                    self.tx_popup_tab,
                    self.block_input_popup_open,
                    self.info_popup_open,
                    false,
                );

                if let Some(error_msg) = &self.error_message {
                    self.render_error_popup(frame, error_msg);
                } else if self.is_loading {
                    self.render_loading_popup(frame);
                }
            }
        }
    }

    fn render_loading_popup(&self, frame: &mut Frame) {
        let text = match self.loading_block {
            Some(block) => format!("Loading block {}...", block),
            None => "Loading latest block...".to_string(),
        };

        let popup_area = centered_rect(text.len() as u16 + 4, 3, frame.area());

        let popup = Paragraph::new(text)
            .style(Style::default().fg(Color::Yellow))
            .block(Block::bordered().style(Style::default().bg(Color::DarkGray)));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup, popup_area);
    }

    fn render_error_popup(&self, frame: &mut Frame, error_msg: &str) {
        let text = format!("Error: {} (press any key, r to refresh RPC)", error_msg);
        let max_width = frame.area().width.saturating_sub(4).min(80);
        let inner_width = max_width.saturating_sub(2);
        let lines_needed = (text.len() as u16).div_ceil(inner_width).max(1);
        let popup_height = (lines_needed + 2).min(frame.area().height.saturating_sub(4));
        let popup_area = centered_rect(max_width, popup_height, frame.area());

        let popup = Paragraph::new(text)
            .style(Style::default().fg(Color::White).bg(Color::Red))
            .wrap(Wrap { trim: false })
            .block(Block::bordered().style(Style::default().fg(Color::White).bg(Color::Red)));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup, popup_area);
    }

    fn render_block_input_popup(&self, frame: &mut Frame) {
        let popup_width = 40.min(frame.area().width - 4);
        let popup_area = centered_rect(popup_width, 3, frame.area());

        let input_text = if self.block_input_query.is_empty() {
            Line::from(vec![
                Span::styled("Block: ", Style::default().fg(Color::Yellow)),
                Span::styled("(enter number)", Style::default().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(vec![
                Span::styled("Block: ", Style::default().fg(Color::Yellow)),
                Span::raw(&self.block_input_query),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ])
        };

        let popup = Paragraph::new(input_text).block(
            Block::bordered()
                .title(" Go to Block ")
                .style(Style::default().bg(Color::DarkGray))
                .border_set(border::THICK),
        );

        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup, popup_area);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        select! {
            recv(self.state_rx) -> event => {
                if let Ok(event) = event {
                    match event {
                        AppEvent::Key(key_code) => self.handle_key_event(key_code),
                        AppEvent::Data(response) => self.handle_data_response(response),
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_data_response(&mut self, response: DataResponse) {
        match response {
            DataResponse::Block(block_num, txs) => {
                self.current_block = Some(block_num);
                let prev_selection = self.table_state.selected();
                self.items = txs;
                self.is_loading = false;
                self.loading_block = None;
                self.tx_popup_scroll = 0;

                let new_selection = if self.tx_popup_open
                    && let Some(prev_idx) = prev_selection
                {
                    if self.items.is_empty() {
                        None
                    } else {
                        Some(prev_idx.min(self.items.len() - 1))
                    }
                } else if self.items.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.table_state.select(new_selection);

                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Opcodes {
                    self.request_opcodes_if_needed();
                }
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Traces {
                    self.request_traces_if_needed();
                }
            }
            DataResponse::Tx(_hash, _tx) => {
                // TODO: handle individual tx updates
            }
            DataResponse::Opcodes(tx_hash, opcodes) => {
                if self.opcodes_tx_hash.as_ref() == Some(&tx_hash) {
                    self.opcodes = Some(opcodes);
                    self.opcodes_loading = false;
                }
            }
            DataResponse::Traces(tx_hash, traces) => {
                if self.traces_tx_hash.as_ref() == Some(&tx_hash) {
                    self.traces = Some(traces);
                    self.traces_loading = false;
                }
            }
            DataResponse::TxTraced(tx_hash, traced_tx) => {
                if self.tx_trace_hash.as_ref() == Some(&tx_hash) {
                    self.tx_trace_loading = false;
                    self.tx_trace_hash = None;
                    if let Some(tx) = self
                        .items
                        .iter_mut()
                        .find(|t| t.tx_hash.to_string() == tx_hash)
                    {
                        tx.coinbase_transfer = traced_tx.coinbase_transfer;
                        tx.display_coinbase_transfer = traced_tx.display_coinbase_transfer;
                        tx.display_coinbase_transfer_usd = traced_tx.display_coinbase_transfer_usd;
                        tx.full_tx_cost = traced_tx.full_tx_cost;
                        tx.display_full_tx_cost = traced_tx.display_full_tx_cost;
                        tx.display_full_tx_cost_usd = traced_tx.display_full_tx_cost_usd;
                    }
                }
            }
            DataResponse::Chains(chains) => {
                self.available_chains = chains;
                self.is_loading = false;

                if !self.available_chains.is_empty() {
                    self.network_table_state.select(Some(0));
                }
            }
            DataResponse::ChainInfo(chain) => {
                self.chain_id = Some(chain.chain_id);
                self.selected_chain = Some(chain);
                if let Some(opts) = self.rpc_opts() {
                    let _ = self
                        .data_req_tx
                        .send(DataRequest::Block(BlockId::Latest, opts));
                }
            }
            DataResponse::TraceMode(trace_mode) => {
                self.trace_mode = Some(trace_mode);
            }
            DataResponse::RpcRefreshed(new_rpc_url) => {
                self.handle_rpc_refreshed(new_rpc_url);
            }
            DataResponse::Error(error_msg) => {
                self.is_loading = false;
                self.loading_block = None;
                self.error_message = Some(error_msg);
            }
        }
    }

    pub(crate) fn exit(&mut self) {
        self.exit = true;
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
