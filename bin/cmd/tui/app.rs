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
    data::{BlockId, DataRequest, DataResponse, MEVTransactionJson, worker::spawn_data_worker},
    views::{NetworkSelector, SearchView, StatusBar, TabBar, TxsTable, render_key_bindings},
};

const DEFAULT_CHAINS: [(u64, &str, &str); 10] = [
    (1, "Ethereum Mainnet", "ETH"),
    (10, "OP Mainnet", "ETH"),
    (56, "BNB Smart Chain Mainnet", "BSC"),
    (130, "Unichain", "ETH"),
    (137, "Polygon Mainnet", "Polygon"),
    (324, "zkSync Mainnet", "ETH"),
    (8453, "Base", "ETH"),
    (42161, "Arbitrum One", "ETH"),
    (43114, "Avalanche C-Chain", "AVAX"),
    (534352, "Scroll Mainnet", "ETH"),
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AppMode {
    SelectNetwork,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Tab {
    Explore,
    Search,
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
    pub(crate) conn_opts: ConnOpts,
    pub(crate) active_tab: Tab,
    pub(crate) selected_chain: Option<ChainEntryJson>,
    state_tx: Sender<AppEvent>,
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
                .find(|(id, _, _)| *id == chain_id)
                .map(|(id, name, chain)| ChainEntryJson {
                    chain_id: *id,
                    name: name.to_string(),
                    chain: chain.to_string(),
                })
        });

        spawn_data_worker(data_req_rx, state_tx.clone(), conn_opts);
        spawn_input_reader(state_tx.clone());

        if mode == AppMode::Main {
            let _ = data_req_tx.send(DataRequest::Block(BlockId::Latest));
        }

        let available_chains = if mode == AppMode::SelectNetwork {
            DEFAULT_CHAINS
                .iter()
                .map(|(id, name, chain)| ChainEntryJson {
                    chain_id: *id,
                    name: name.to_string(),
                    chain: chain.to_string(),
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
            conn_opts: conn_opts.clone(),
            active_tab: Tab::Explore,
            selected_chain,
            state_tx,
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
                    .render(chunks[0], frame, &mut self.network_table_state);

                // Render key bindings footer
                render_key_bindings(frame, chunks[1], &self.mode, None);

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
                    Tab::Explore => {
                        TxsTable::new(&self.items).render(chunks[2], frame, &mut self.table_state);
                    }
                    Tab::Search => {
                        SearchView::new().render(chunks[2], frame);
                    }
                }

                render_key_bindings(frame, chunks[3], &self.mode, Some(self.active_tab));

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
                self.items = txs;
                self.is_loading = false;
                self.loading_block = None;
                self.table_state
                    .select(if self.items.is_empty() { None } else { Some(0) });
            }
            DataResponse::Tx(_hash, _tx) => {
                // TODO: handle individual tx updates
            }
            DataResponse::Chains(chains) => {
                self.available_chains = chains;
                self.is_loading = false;

                if !self.available_chains.is_empty() {
                    self.network_table_state.select(Some(0));
                }
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
