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
    widgets::{Block, Clear, Paragraph, TableState},
};

use mevlog::{ChainEntryJson, misc::shared_init::ConnOpts};

use crate::cmd::tui::{
    app::keys::spawn_input_reader,
    data::{
        BlockId, DataRequest, DataResponse, MEVOpcodeJson, MEVTransactionJson,
        worker::spawn_data_worker,
    },
    views::{
        NetworkSelector, SearchView, StatusBar, TabBar, TxsTable, render_key_bindings,
        render_tx_popup,
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
    pub(crate) conn_opts: ConnOpts,
    pub(crate) active_tab: PrimaryTab,
    pub(crate) selected_chain: Option<ChainEntryJson>,
    state_tx: Sender<AppEvent>,
    pub(crate) opcodes: Option<Vec<MEVOpcodeJson>>,
    pub(crate) opcodes_loading: bool,
    pub(crate) opcodes_tx_hash: Option<String>,
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

        spawn_data_worker(data_req_rx, state_tx.clone(), conn_opts);
        spawn_input_reader(state_tx.clone());

        if mode == AppMode::Main {
            let _ = data_req_tx.send(DataRequest::Block(BlockId::Latest));
            if conn_opts.rpc_url.is_some() && conn_opts.chain_id.is_none() {
                let _ =
                    data_req_tx.send(DataRequest::ChainInfo(conn_opts.rpc_url.clone().unwrap()));
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
            conn_opts: conn_opts.clone(),
            active_tab: PrimaryTab::Explore,
            selected_chain,
            state_tx,
            opcodes: None,
            opcodes_loading: false,
            opcodes_tx_hash: None,
        }
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
                render_key_bindings(frame, chunks[1], &self.mode, None, self.search_popup_open);

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
                            );
                        }
                    }
                    PrimaryTab::Search => {
                        SearchView::new().render(chunks[2], frame);
                    }
                }

                render_key_bindings(
                    frame,
                    chunks[3],
                    &self.mode,
                    Some(self.active_tab),
                    self.tx_popup_open,
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
        let text = format!("Error: {} (press any key)", error_msg);
        let popup_width = (text.len() as u16 + 4).min(frame.area().width - 4);
        let popup_area = centered_rect(popup_width, 3, frame.area());

        let popup = Paragraph::new(text)
            .style(Style::default().fg(Color::Red))
            .block(Block::bordered().style(Style::default().bg(Color::DarkGray)));

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
            DataResponse::Chains(chains) => {
                self.available_chains = chains;
                self.is_loading = false;

                if !self.available_chains.is_empty() {
                    self.network_table_state.select(Some(0));
                }
            }
            DataResponse::ChainInfo(chain) => {
                self.selected_chain = Some(chain);
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
