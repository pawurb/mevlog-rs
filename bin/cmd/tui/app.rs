//! TUI application state and main run loop

mod data;
mod keys;
mod state;

use std::io;

use crossbeam_channel::{Receiver, Sender, select};
use crossterm::event::KeyCode;
use ratatui::{DefaultTerminal, Frame, widgets::TableState};

use crate::cmd::tui::{
    app::keys::spawn_input_reader,
    data::{DataRequest, DataResponse, TxRow, worker::spawn_data_worker},
    views::TxsTable,
};

/// Unified event type for the application
pub(crate) enum AppEvent {
    /// Keyboard input
    Key(KeyCode),
    /// Data fetched from worker
    Data(DataResponse),
    /// Terminal resize
    Resize(u16, u16),
}

pub struct App {
    pub(crate) table_state: TableState,
    pub(crate) items: Vec<TxRow>,
    pub(crate) current_block: u64,
    data_req_tx: Sender<DataRequest>,
    state_rx: Receiver<AppEvent>,
    exit: bool,
}

impl App {
    pub fn new(items: Vec<TxRow>) -> Self {
        let current_block = items.first().map(|tx| tx.block_number).unwrap_or(0);

        let (data_req_tx, data_req_rx) = crossbeam_channel::unbounded();
        let (state_tx, state_rx) = crossbeam_channel::unbounded();

        spawn_data_worker(data_req_rx, state_tx.clone());
        spawn_input_reader(state_tx);

        // Fetch latest block on launch
        let _ = data_req_tx.send(DataRequest::FetchLatest);

        Self {
            table_state: TableState::default().with_selected(if items.is_empty() {
                None
            } else {
                Some(0)
            }),
            items,
            current_block,
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
    }

    fn handle_events(&mut self) -> io::Result<()> {
        select! {
            recv(self.state_rx) -> event => {
                if let Ok(event) = event {
                    match event {
                        AppEvent::Key(key_code) => self.handle_key_event(key_code),
                        AppEvent::Data(response) => self.handle_data_response(response),
                        AppEvent::Resize(_, _) => {
                            // Terminal will redraw on next iteration
                        }
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
                self.table_state
                    .select(if self.items.is_empty() { None } else { Some(0) });
            }
            DataResponse::Tx(_hash, _tx) => {
                // TODO: handle individual tx updates
            }
        }
    }

    pub(crate) fn exit(&mut self) {
        self.exit = true;
    }
}
