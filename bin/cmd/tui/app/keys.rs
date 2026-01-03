//! Keyboard input handling

use crossbeam_channel::Sender;
use crossterm::event::{self, KeyCode};

use super::{App, AppMode};
use crate::cmd::tui::app::AppEvent;

impl App {
    pub(crate) fn handle_key_event(&mut self, key_code: KeyCode) {
        if self.error_message.is_some() {
            self.error_message = None;
            return;
        }

        match self.mode {
            AppMode::SelectNetwork => self.handle_network_selection_keys(key_code),
            AppMode::Main => self.handle_main_mode_keys(key_code),
        }
    }

    fn handle_main_mode_keys(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.exit(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('h') | KeyCode::Left => self.load_previous_block(),
            KeyCode::Char('l') | KeyCode::Right => self.load_next_block(),
            _ => {}
        }
    }

    fn handle_network_selection_keys(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.exit(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_network(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous_network(),
            KeyCode::Enter => self.confirm_network_selection(),
            KeyCode::Backspace => {
                if !self.search_query.is_empty() {
                    self.search_query.pop();
                    self.request_filtered_chains();
                }
            }
            KeyCode::Char(c) if c.is_alphanumeric() || c == ' ' || c == '-' => {
                self.search_query.push(c);
                self.request_filtered_chains();
            }
            _ => {}
        }
    }
}

use crossterm::event::{Event, KeyEventKind};

pub(crate) fn spawn_input_reader(event_tx: Sender<AppEvent>) {
    std::thread::spawn(move || {
        loop {
            if let Ok(evt) = event::read() {
                let app_event = match evt {
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        Some(AppEvent::Key(key_event.code))
                    }
                    _ => None,
                };

                if let Some(app_event) = app_event
                    && event_tx.send(app_event).is_err()
                {
                    // Channel closed, exit thread
                    break;
                }
            }
        }
    });
}
