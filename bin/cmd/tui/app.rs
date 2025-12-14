//! TUI application state and main run loop

mod data;
mod keys;
mod state;

use std::io;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{DefaultTerminal, Frame, widgets::TableState};

use crate::cmd::tui::data::TxRow;
use crate::cmd::tui::views::TxsTable;

pub struct App {
    pub(crate) table_state: TableState,
    pub(crate) items: Vec<TxRow>,
    exit: bool,
}

impl App {
    pub fn new(items: Vec<TxRow>) -> Self {
        Self {
            table_state: TableState::default().with_selected(if items.is_empty() {
                None
            } else {
                Some(0)
            }),
            items,
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
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event.code)
            }
            _ => {}
        };
        Ok(())
    }

    pub(crate) fn exit(&mut self) {
        self.exit = true;
    }
}
