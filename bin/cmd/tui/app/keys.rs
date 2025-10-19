//! Keyboard input handling

use crossbeam_channel::Sender;
use crossterm::event::{self, KeyCode};

use super::App;
use crate::cmd::tui::app::AppEvent;

impl App {
    pub(crate) fn handle_key_event(&mut self, key_code: KeyCode) {
        if self.error_message.is_some() {
            self.error_message = None;
            return;
        }

        match key_code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.exit(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('h') | KeyCode::Left => self.load_previous_block(),
            KeyCode::Char('l') | KeyCode::Right => self.load_next_block(),
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
