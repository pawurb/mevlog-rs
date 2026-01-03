//! Keyboard input handling

use crossbeam_channel::Sender;
use crossterm::event::{self, KeyCode};

use super::{App, AppMode, Tab};
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

            KeyCode::Char('1') => self.switch_to_tab(Tab::Explore),
            KeyCode::Char('2') => self.switch_to_tab(Tab::Search),
            KeyCode::Tab => self.cycle_tab(),

            _ if self.active_tab == Tab::Explore => {
                self.handle_explore_keys(key_code);
            }

            _ if self.active_tab == Tab::Search => {
                // TODO: WIP
            }

            _ => {}
        }
    }

    fn handle_explore_keys(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('h') | KeyCode::Left => self.load_previous_block(),
            KeyCode::Char('l') | KeyCode::Right => self.load_next_block(),
            _ => {}
        }
    }

    fn handle_network_selection_keys(&mut self, key_code: KeyCode) {
        if self.search_popup_open {
            // Popup open mode - only typing and popup controls
            match key_code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.search_popup_open = false;
                }
                KeyCode::Backspace => {
                    if !self.search_query.is_empty() {
                        self.search_query.pop();
                        self.request_filtered_chains();
                    }
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.request_filtered_chains();
                }
                _ => {}
            }
        } else {
            // Popup closed mode - navigation and popup opener
            match key_code {
                KeyCode::Char('q') | KeyCode::Char('Q') => self.exit(),
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.search_popup_open = true;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if !self.search_query.is_empty() {
                        self.search_query.clear();
                        self.request_filtered_chains();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => self.select_next_network(),
                KeyCode::Up | KeyCode::Char('k') => self.select_previous_network(),
                KeyCode::Enter | KeyCode::Char('o') | KeyCode::Char('O') => {
                    self.confirm_network_selection()
                }
                _ => {}
            }
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
