//! UI state management - navigation and selection

use super::App;

impl App {
    pub(crate) fn select_next(&mut self) {
        let count = self.items.len();
        if count == 0 {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => (i + 1).min(count - 1),
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub(crate) fn select_previous(&mut self) {
        let count = self.items.len();
        if count == 0 {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.table_state.select(Some(i));
    }
}
