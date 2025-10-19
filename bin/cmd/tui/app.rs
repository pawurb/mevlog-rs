//! TUI application state and main run loop

mod data;
mod keys;
mod state;

use std::io;

use crossbeam_channel::{Receiver, Sender, select};
use crossterm::event::KeyCode;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Clear, Paragraph, TableState},
};

use crate::cmd::tui::{
    app::keys::spawn_input_reader,
    data::{BlockId, DataRequest, DataResponse, MEVTransactionJson, worker::spawn_data_worker},
    views::TxsTable,
};

/// Unified event type for the application
#[allow(clippy::large_enum_variant)]
pub(crate) enum AppEvent {
    /// Keyboard input
    Key(KeyCode),
    /// Data fetched from worker
    Data(DataResponse),
}

pub struct App {
    pub(crate) table_state: TableState,
    pub(crate) items: Vec<MEVTransactionJson>,
    pub(crate) current_block: u64,
    pub(crate) is_loading: bool,
    pub(crate) loading_block: Option<u64>,
    pub(crate) error_message: Option<String>,
    data_req_tx: Sender<DataRequest>,
    state_rx: Receiver<AppEvent>,
    exit: bool,
}

impl App {
    pub fn new(items: Vec<MEVTransactionJson>) -> Self {
        let current_block = items.first().map(|tx| tx.block_number).unwrap_or(0);

        let (data_req_tx, data_req_rx) = crossbeam_channel::unbounded();
        let (state_tx, state_rx) = crossbeam_channel::unbounded();

        spawn_data_worker(data_req_rx, state_tx.clone());
        spawn_input_reader(state_tx);

        // Fetch latest block on launch
        let _ = data_req_tx.send(DataRequest::Block(BlockId::Latest));

        Self {
            table_state: TableState::default().with_selected(if items.is_empty() {
                None
            } else {
                Some(0)
            }),
            items,
            current_block,
            is_loading: true,
            loading_block: None,
            error_message: None,
            data_req_tx,
            state_rx,
            exit: false,
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
        TxsTable::new(&self.items).render(frame.area(), frame, &mut self.table_state);

        if let Some(error_msg) = &self.error_message {
            self.render_error_popup(frame, error_msg);
        } else if self.is_loading {
            self.render_loading_popup(frame);
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
                self.current_block = block_num;
                self.items = txs;
                self.is_loading = false;
                self.loading_block = None;
                self.table_state
                    .select(if self.items.is_empty() { None } else { Some(0) });
            }
            DataResponse::Tx(_hash, _tx) => {
                // TODO: handle individual tx updates
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
