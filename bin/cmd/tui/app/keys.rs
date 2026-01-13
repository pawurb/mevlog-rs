//! Keyboard input handling

use crossbeam_channel::Sender;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::cmd::tui::app::{App, AppEvent, AppMode, PrimaryTab, TxPopupTab};

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

    fn handle_main_mode_keys(&mut self, key_code: KeyCode) {
        if self.block_input_popup_open || self.info_popup_open {
            self.handle_explore_keys(key_code);
            return;
        }

        if self.query_popup_open {
            self.handle_search_keys(key_code);
            return;
        }

        if self.search_editing {
            self.handle_search_keys(key_code);
            return;
        }

        match key_code {
            KeyCode::Char('n') if !self.tx_popup_open && !self.info_popup_open => {
                self.open_network_selection();
            }

            KeyCode::Char('1') if self.tx_popup_open => self.tx_popup_tab = TxPopupTab::Info,
            KeyCode::Char('2') if self.tx_popup_open => self.tx_popup_tab = TxPopupTab::Transfers,
            KeyCode::Char('3') if self.tx_popup_open => {
                self.tx_popup_tab = TxPopupTab::Opcodes;
                if self.active_tab == PrimaryTab::Results {
                    self.request_results_opcodes_if_needed();
                } else {
                    self.request_opcodes_if_needed();
                }
            }
            KeyCode::Char('4') if self.tx_popup_open => {
                self.tx_popup_tab = TxPopupTab::Traces;
                if self.active_tab == PrimaryTab::Results {
                    self.request_results_traces_if_needed();
                } else {
                    self.request_traces_if_needed();
                }
            }
            KeyCode::Char('t') if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Info => {
                if self.active_tab == PrimaryTab::Results {
                    self.request_results_tx_trace();
                } else {
                    self.request_tx_trace();
                }
            }

            KeyCode::Char('1') => self.switch_to_tab(PrimaryTab::Explore),
            KeyCode::Char('2') => self.switch_to_tab(PrimaryTab::Search),
            KeyCode::Char('3') => self.switch_to_tab(PrimaryTab::Results),
            KeyCode::Tab => self.cycle_tab(),

            _ if self.active_tab == PrimaryTab::Explore => {
                self.handle_explore_keys(key_code);
            }

            _ if self.active_tab == PrimaryTab::Search => {
                self.handle_search_keys(key_code);
            }

            _ if self.active_tab == PrimaryTab::Results => {
                self.handle_results_keys(key_code);
            }

            _ => {}
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
            KeyCode::Char('n') if !self.tx_popup_open => {
                self.return_to_network_selection();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Opcodes {
                    self.request_opcodes_if_needed();
                }
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Traces {
                    self.request_traces_if_needed();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_previous();
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Opcodes {
                    self.request_opcodes_if_needed();
                }
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Traces {
                    self.request_traces_if_needed();
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.clear_opcodes();
                self.clear_traces();
                self.load_previous_block();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.clear_opcodes();
                self.clear_traces();
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
                    self.clear_opcodes();
                    self.clear_traces();
                }
            }
            KeyCode::Char('i') => {
                self.info_popup_open = true;
            }
            KeyCode::Esc if self.tx_popup_open => {
                self.tx_popup_open = false;
                self.tx_popup_scroll = 0;
                self.tx_popup_tab = TxPopupTab::default();
                self.clear_opcodes();
                self.clear_traces();
            }
            KeyCode::Char('n') if self.tx_popup_open => {
                self.tx_popup_scroll = self.tx_popup_scroll.saturating_add(1);
            }
            KeyCode::Char('m') if self.tx_popup_open => {
                self.tx_popup_scroll = self.tx_popup_scroll.saturating_sub(1);
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
            match key_code {
                KeyCode::Char('n') | KeyCode::Esc if self.can_return_to_main() => {
                    self.return_to_main();
                }
                KeyCode::Char('s') => {
                    self.search_popup_open = true;
                }
                KeyCode::Char('c') => {
                    if !self.search_query.is_empty() {
                        self.search_query.clear();
                        self.request_filtered_chains();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => self.select_next_network(),
                KeyCode::Up | KeyCode::Char('k') => self.select_previous_network(),
                KeyCode::Enter | KeyCode::Char('o') => self.confirm_network_selection(),
                _ => {}
            }
        }
    }

    fn handle_search_keys(&mut self, key_code: KeyCode) {
        const NUM_FIELDS: usize = 12;

        if self.query_popup_open {
            match key_code {
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.query_popup_open = false;
                }
                KeyCode::Char('y') => {
                    self.query_popup_open = false;
                    self.search_results.clear();
                    self.results_table_state.select(None);
                    self.active_tab = PrimaryTab::Results;
                    self.is_loading = true;
                    self.execute_search();
                }
                _ => {}
            }
            return;
        }

        if self.search_editing {
            match key_code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.search_editing = false;
                }
                _ => {
                    let event = Event::Key(KeyEvent::new(key_code, KeyModifiers::empty()));
                    let input = match self.search_active_field {
                        0 => &mut self.filter_limit,
                        1 => &mut self.filter_txhash,
                        2 => &mut self.filter_blocks,
                        3 => &mut self.filter_position,
                        4 => &mut self.filter_from,
                        5 => &mut self.filter_to,
                        6 => &mut self.filter_event,
                        7 => &mut self.filter_not_event,
                        8 => &mut self.filter_method,
                        9 => &mut self.filter_erc20_transfer,
                        10 => &mut self.filter_tx_cost,
                        _ => &mut self.filter_gas_price,
                    };
                    input.handle_event(&event);
                }
            }
        } else {
            match key_code {
                KeyCode::Enter | KeyCode::Char('o') => {
                    self.search_editing = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.search_active_field < NUM_FIELDS - 1 {
                        self.search_active_field += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.search_active_field = self.search_active_field.saturating_sub(1);
                }
                KeyCode::Char('s') => {
                    self.query_popup_open = true;
                }
                KeyCode::Char('c') => {
                    let input = match self.search_active_field {
                        0 => &mut self.filter_limit,
                        1 => &mut self.filter_txhash,
                        2 => &mut self.filter_blocks,
                        3 => &mut self.filter_position,
                        4 => &mut self.filter_from,
                        5 => &mut self.filter_to,
                        6 => &mut self.filter_event,
                        7 => &mut self.filter_not_event,
                        8 => &mut self.filter_method,
                        9 => &mut self.filter_erc20_transfer,
                        10 => &mut self.filter_tx_cost,
                        _ => &mut self.filter_gas_price,
                    };
                    *input = Input::default();
                }
                _ => {}
            }
        }
    }

    fn handle_results_keys(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next_result();
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Opcodes {
                    self.request_results_opcodes_if_needed();
                }
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Traces {
                    self.request_results_traces_if_needed();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_previous_result();
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Opcodes {
                    self.request_results_opcodes_if_needed();
                }
                if self.tx_popup_open && self.tx_popup_tab == TxPopupTab::Traces {
                    self.request_results_traces_if_needed();
                }
            }
            KeyCode::Char('o') => {
                if !self.search_results.is_empty() {
                    self.tx_popup_open = !self.tx_popup_open;
                    if !self.tx_popup_open {
                        self.tx_popup_scroll = 0;
                        self.tx_popup_tab = TxPopupTab::default();
                        self.clear_opcodes();
                        self.clear_traces();
                    }
                }
            }
            KeyCode::Esc if self.tx_popup_open => {
                self.tx_popup_open = false;
                self.tx_popup_scroll = 0;
                self.tx_popup_tab = TxPopupTab::default();
                self.clear_opcodes();
                self.clear_traces();
            }
            KeyCode::Char('n') if self.tx_popup_open => {
                self.tx_popup_scroll = self.tx_popup_scroll.saturating_add(1);
            }
            KeyCode::Char('m') if self.tx_popup_open => {
                self.tx_popup_scroll = self.tx_popup_scroll.saturating_sub(1);
            }
            KeyCode::Char('c') => {
                self.search_results.clear();
                self.results_table_state.select(None);
            }
            _ => {}
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
