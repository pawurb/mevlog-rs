//! Data management - fetching and updating transaction data

use super::App;

impl App {
    pub(crate) fn load_block(&mut self, block: u64) {
        if let Ok(items) = self.fetcher.fetch_sync(block) {
            self.current_block = block;
            self.items = items;
            self.table_state
                .select(if self.items.is_empty() { None } else { Some(0) });
        }
    }

    pub(crate) fn load_previous_block(&mut self) {
        if self.current_block > 0 {
            self.load_block(self.current_block - 1);
        }
    }

    pub(crate) fn load_next_block(&mut self) {
        self.load_block(self.current_block + 1);
    }
}
