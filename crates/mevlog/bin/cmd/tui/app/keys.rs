//! Keyboard input handling

use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use hotpath::wrap::crossbeam_channel::Sender;

use crate::cmd::tui::app::{App, AppEvent, AppMode, TxPopupTab};

#[hotpath::measure_all]
impl App {
    pub(crate) fn handle_key_event(&mut self, key_code: KeyCode) {
        if matches!(key_code, KeyCode::Char('q')) {
            self.exit();
            return;
        }

        if self.error_message.is_some() {
            if matches!(key_code, KeyCode::Char('r')) {
                self.error_message = None;
                self.request_rpc_refresh();
            } else {
                self.error_message = None;
            }
            return;
        }

        match self.mode {
            AppMode::SelectNetwork => self.handle_network_selection_keys(key_code),
            AppMode::Main => self.handle_main_mode_keys(key_code),
        }
    }

    fn handle_g_key(&mut self) -> bool {
        if let Some(last_g) = self.pending_g
            && last_g.elapsed() < Duration::from_millis(500)
        {
            self.pending_g = None;
            return true;
        }
        self.pending_g = Some(Instant::now());
        false
    }

    fn handle_main_mode_keys(&mut self, key_code: KeyCode) {
        if self.block_input_popup_open || self.info_popup_open {
            self.handle_explore_keys(key_code);
            return;
        }

        match key_code {
            KeyCode::Char('n') if !self.tx_popup_open && !self.info_popup_open => {
                self.open_network_selection();
            }

            KeyCode::Char('1') if self.tx_popup_open => {
                self.tx_popup_tab = TxPopupTab::Info;
            }
            KeyCode::Char('2') if self.tx_popup_open => {
                self.tx_popup_tab = TxPopupTab::Transfers;
            }

            _ => self.handle_explore_keys(key_code),
        }
    }

    fn handle_explore_keys(&mut self, key_code: KeyCode) {
        if self.block_input_popup_open {
            match key_code {
                KeyCode::Esc => {
                    self.block_input_popup_open = false;
                    self.block_input_query.clear();
                }
                KeyCode::Enter | KeyCode::Char('o') => {
                    if let Ok(block_num) = self.block_input_query.parse::<u64>() {
                        self.load_block(block_num);
                    }
                    self.block_input_popup_open = false;
                    self.block_input_query.clear();
                }
                KeyCode::Char('l') => {
                    self.load_latest_block();
                    self.block_input_popup_open = false;
                    self.block_input_query.clear();
                }
                KeyCode::Backspace => {
                    self.block_input_query.pop();
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    self.block_input_query.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.info_popup_open {
            match key_code {
                KeyCode::Char('r') => self.request_rpc_refresh(),
                KeyCode::Char('i') | KeyCode::Esc => {
                    self.info_popup_open = false;
                }
                _ => {}
            }
            return;
        }

        match key_code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_previous();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.load_previous_block();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.load_next_block();
            }
            KeyCode::Char('b') => {
                self.block_input_popup_open = true;
            }
            KeyCode::Char('o') => {
                self.tx_popup_open = !self.tx_popup_open;
                if !self.tx_popup_open {
                    self.tx_popup_scroll = 0;
                    self.tx_popup_tab = TxPopupTab::default();
                }
            }
            KeyCode::Char('i') => {
                self.info_popup_open = true;
            }
            KeyCode::Esc if self.tx_popup_open => {
                self.tx_popup_open = false;
                self.tx_popup_scroll = 0;
                self.tx_popup_tab = TxPopupTab::default();
            }
            KeyCode::Char('n') if self.tx_popup_open => {
                self.tx_popup_scroll = self
                    .tx_popup_scroll
                    .saturating_add(1)
                    .min(self.tx_popup_max_scroll);
            }
            KeyCode::Char('m') if self.tx_popup_open => {
                self.tx_popup_scroll = self.tx_popup_scroll.saturating_sub(1);
            }
            KeyCode::Char('G') => {
                self.pending_g = None;
                if self.tx_popup_open {
                    self.tx_popup_scroll = self.tx_popup_max_scroll;
                } else {
                    self.select_last();
                }
            }
            KeyCode::Char('g') if self.handle_g_key() => {
                if self.tx_popup_open {
                    self.tx_popup_scroll = 0;
                } else {
                    self.select_first();
                }
            }
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
                KeyCode::Backspace if !self.search_query.is_empty() => {
                    self.search_query.pop();
                    self.request_filtered_chains();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.request_filtered_chains();
                }
                _ => {}
            }
        } else {
            match key_code {
                KeyCode::Char('n') | KeyCode::Esc if self.can_return_to_main() => {
                    self.return_to_main();
                }
                KeyCode::Char('s') => {
                    self.search_popup_open = true;
                }
                KeyCode::Char('c') if !self.search_query.is_empty() => {
                    self.search_query.clear();
                    self.request_filtered_chains();
                }
                KeyCode::Down | KeyCode::Char('j') => self.select_next_network(),
                KeyCode::Up | KeyCode::Char('k') => self.select_previous_network(),
                KeyCode::Enter | KeyCode::Char('o') => self.confirm_network_selection(),
                _ => {}
            }
        }
    }
}

pub(crate) fn spawn_input_reader(event_tx: Sender<AppEvent>) {
    std::thread::spawn(move || {
        while let Ok(evt) = event::read() {
            if let Event::Key(key_event) = evt
                && key_event.kind == KeyEventKind::Press
                && event_tx.send(AppEvent::Key(key_event.code)).is_err()
            {
                break;
            }
        }
    });
}
