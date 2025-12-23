//! TUI application state and main run loop

mod data;
mod keys;
mod state;

use std::io;

use crossbeam_channel::{Receiver, Sender, select};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{DefaultTerminal, Frame, widgets::TableState};

use crate::cmd::tui::{
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
    event_rx: Receiver<AppEvent>,
    exit: bool,
}

impl App {
    pub fn new(items: Vec<TxRow>) -> Self {
        let current_block = items.first().map(|tx| tx.block_number).unwrap_or(0);

        // Create channels
        let (data_req_tx, data_req_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();

        // Spawn data worker
        let data_event_tx = event_tx.clone();
        spawn_data_worker(data_req_rx, data_event_tx);

        // Spawn keyboard event reader thread
        spawn_input_reader(event_tx);

        Self {
            table_state: TableState::default().with_selected(if items.is_empty() {
                None
            } else {
                Some(0)
            }),
            items,
            current_block,
            data_req_tx,
            event_rx,
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
            recv(self.event_rx) -> event => {
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

/// Spawns a thread that reads terminal events and sends them through the channel
fn spawn_input_reader(event_tx: Sender<AppEvent>) {
    std::thread::spawn(move || {
        loop {
            if let Ok(evt) = event::read() {
                let app_event = match evt {
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        Some(AppEvent::Key(key_event.code))
                    }
                    Event::Resize(w, h) => Some(AppEvent::Resize(w, h)),
                    _ => None,
                };

                if let Some(app_event) = app_event {
                    if event_tx.send(app_event).is_err() {
                        // Channel closed, exit thread
                        break;
                    }
                }
            }
        }
    });
}
