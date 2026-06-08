//! TUI application state and main run loop

mod data;
mod keys;
mod state;

use std::{io, time::Instant};

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

use mevlog::{ChainEntryJson, misc::shared_init::ConnOpts};

use crate::cmd::tui::{
    app::keys::spawn_input_reader,
    data::{
        BlockId, CallExtract, DataRequest, DataResponse, RpcOpts, StateDiffJson, TraceMode,
        TransactionJson, worker::spawn_data_worker,
    },
    views::{
        NetworkSelector, StatusBar, TxsTable, render_info_popup, render_key_bindings,
        render_tx_popup,
    },
};

pub(crate) struct DefaultChain {
    pub chain_id: u64,
    pub name: &'static str,
    pub chain: &'static str,
    pub explorer_url: &'static str,
}

impl DefaultChain {
    pub(super) const fn new(
        chain_id: u64,
        name: &'static str,
        chain: &'static str,
        explorer_url: &'static str,
    ) -> Self {
        Self {
            chain_id,
            name,
            chain,
            explorer_url,
        }
    }

    pub(super) fn to_chain_entry(&self) -> ChainEntryJson {
        ChainEntryJson {
            chain_id: self.chain_id,
            name: self.name.to_string(),
            chain: self.chain.to_string(),
            explorer_url: Some(self.explorer_url.to_string()),
        }
    }
}

pub(crate) const DEFAULT_CHAINS: [DefaultChain; 10] = [
    DefaultChain::new(1, "Ethereum Mainnet", "ETH", "https://etherscan.io"),
    DefaultChain::new(10, "OP Mainnet", "ETH", "https://optimistic.etherscan.io"),
    DefaultChain::new(56, "BNB Smart Chain Mainnet", "BSC", "https://bscscan.com"),
    DefaultChain::new(130, "Unichain", "ETH", "https://unichain.blockscout.com"),
    DefaultChain::new(137, "Polygon Mainnet", "Polygon", "https://polygonscan.com"),
    DefaultChain::new(324, "zkSync Mainnet", "ETH", "https://era.zksync.network"),
    DefaultChain::new(8453, "Base", "ETH", "https://basescan.org"),
    DefaultChain::new(42161, "Arbitrum One", "ETH", "https://arbiscan.io"),
    DefaultChain::new(43114, "Avalanche C-Chain", "AVAX", "https://snowtrace.io"),
    DefaultChain::new(534352, "Scroll Mainnet", "ETH", "https://scrollscan.com"),
];

const DB_INITIALIZING_ERRORS: [&str; 3] = [
    "Database file missing",
    "error returned from database",
    "Creating index",
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AppMode {
    SelectNetwork,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum TxPopupTab {
    #[default]
    Info,
    Traces,
    Transfers,
    State,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AppEvent {
    Key(KeyCode),
    Data(DataResponse),
}

pub(super) struct App {
    pub(crate) table_state: TableState,
    pub(crate) items: Vec<TransactionJson>,
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
    pub(crate) selected_chain: Option<ChainEntryJson>,
    #[allow(dead_code)]
    state_tx: Sender<AppEvent>,
    pub(crate) traces: Option<Vec<CallExtract>>,
    pub(crate) traces_loading: bool,
    pub(crate) traces_tx_hash: Option<String>,
    pub(crate) state_diff: Option<StateDiffJson>,
    pub(crate) state_diff_loading: bool,
    pub(crate) state_diff_tx_hash: Option<String>,
    pub(crate) pending_g: Option<Instant>,
    pub(crate) tx_popup_max_scroll: u16,
    pub(crate) tx_trace_loading: bool,
    pub(crate) tx_trace_hash: Option<String>,
    pub(crate) block_input_popup_open: bool,
    pub(crate) block_input_query: String,
    pub(crate) trace_mode: Option<TraceMode>,
    pub(crate) info_popup_open: bool,
    pub(crate) rpc_refreshing: bool,
}

#[hotpath::measure_all]
impl App {
    pub(crate) fn new(items: Vec<TransactionJson>, conn_opts: &ConnOpts) -> Self {
        let current_block = items.first().map(|tx| tx.block_number);

        let (data_req_tx, data_req_rx) =
            hotpath::channel!(crossbeam_channel::unbounded(), log = true);
        let (state_tx, state_rx) = hotpath::channel!(crossbeam_channel::unbounded(), log = true);

        let mode = if conn_opts.rpc_url.is_none() && conn_opts.chain_id.is_none() {
            AppMode::SelectNetwork
        } else {
            AppMode::Main
        };

        let selected_chain = conn_opts.chain_id.and_then(|chain_id| {
            DEFAULT_CHAINS
                .iter()
                .find(|c| c.chain_id == chain_id)
                .map(|c| c.to_chain_entry())
        });

        let rpc_url = conn_opts.rpc_url.clone();
        let chain_id = conn_opts.chain_id;
        let rpc_timeout_ms = conn_opts.rpc_timeout_ms;
        let block_timeout_ms = conn_opts.block_timeout_ms;

        spawn_data_worker(data_req_rx, state_tx.clone());
        spawn_input_reader(state_tx.clone());

        if mode == AppMode::Main {
            if let Some(ref url) = rpc_url {
                if chain_id.is_some() {
                    let opts = RpcOpts {
                        rpc_url: url.clone(),
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
            DEFAULT_CHAINS.iter().map(|c| c.to_chain_entry()).collect()
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
            selected_chain,
            state_tx,
            traces: None,
            traces_loading: false,
            traces_tx_hash: None,
            state_diff: None,
            state_diff_loading: false,
            state_diff_tx_hash: None,
            pending_g: None,
            tx_popup_max_scroll: 0,
            tx_trace_loading: false,
            tx_trace_hash: None,
            block_input_popup_open: false,
            block_input_query: String::new(),
            trace_mode: None,
            info_popup_open: false,
            rpc_refreshing,
        }
    }

    pub(crate) fn rpc_opts(&self) -> Option<RpcOpts> {
        // A selected chain is still a precondition for fetching, even though the
        // chain id itself is no longer needed once we have the RPC URL.
        self.chain_id?;
        Some(RpcOpts {
            rpc_url: self.rpc_url.clone()?,
            block_timeout_ms: self.block_timeout_ms,
        })
    }

    pub(crate) fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
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
                        Constraint::Length(3), // Status bar
                        Constraint::Min(0),    // Content area
                        Constraint::Length(3), // Key bindings footer
                    ])
                    .split(frame.area());

                StatusBar::new(
                    self.selected_chain.as_ref(),
                    self.current_block,
                    self.is_loading,
                    self.loading_block,
                    self.trace_mode.as_ref(),
                )
                .render(chunks[0], frame);

                let explorer_url = self
                    .selected_chain
                    .as_ref()
                    .and_then(|c| c.explorer_url.as_deref());
                TxsTable::new(&self.items)
                    .with_explorer_url(explorer_url)
                    .render(chunks[1], frame, &mut self.table_state);

                if self.tx_popup_open
                    && let Some(idx) = self.table_state.selected()
                    && let Some(tx) = self.items.get(idx).cloned()
                {
                    let explorer_url = self
                        .selected_chain
                        .as_ref()
                        .and_then(|c| c.explorer_url.clone());
                    self.tx_popup_max_scroll = render_tx_popup(
                        &tx,
                        frame.area(),
                        frame,
                        self.tx_popup_scroll,
                        self.tx_popup_tab,
                        explorer_url.as_deref(),
                        self.traces.as_deref(),
                        self.traces_loading,
                        self.state_diff.as_ref(),
                        self.state_diff_loading,
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

                render_key_bindings(
                    frame,
                    chunks[2],
                    &self.mode,
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
        let text = format!("Error: {} - press 'r' to refresh", error_msg);
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

                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Traces {
                    self.request_traces_if_needed();
                }
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::State {
                    self.request_state_diff_if_needed();
                }
            }
            DataResponse::Traces(tx_hash, traces) => {
                if self.traces_tx_hash.as_ref() == Some(&tx_hash) {
                    self.traces = Some(traces);
                    self.traces_loading = false;
                }
            }
            DataResponse::StateDiff(tx_hash, state_diff) => {
                if self.state_diff_tx_hash.as_ref() == Some(&tx_hash) {
                    self.state_diff = Some(state_diff);
                    self.state_diff_loading = false;
                }
            }
            DataResponse::TxTraced(tx_hash, traced_tx) => {
                if self.tx_trace_hash.as_ref() == Some(&tx_hash) {
                    self.tx_trace_loading = false;
                    self.tx_trace_hash = None;
                    let tx = self
                        .items
                        .iter_mut()
                        .find(|t| t.tx_hash.to_string() == tx_hash);
                    if let Some(tx) = tx {
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
                let display_msg = if DB_INITIALIZING_ERRORS.iter().any(|e| error_msg.contains(e)) {
                    "Local database initializing, please wait a moment and".to_string()
                } else {
                    error_msg.to_string()
                };
                self.error_message = Some(display_msg);
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
