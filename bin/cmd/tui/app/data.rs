//! Data management - fetching and updating transaction data

use super::App;
use crate::cmd::tui::data::{BlockId, DataRequest};

impl App {
    /// Returns the block number to use for navigation (loading_block if loading, otherwise current_block)
    fn effective_block(&self) -> Option<u64> {
        self.loading_block.or(self.current_block)
    }

    pub(crate) fn load_block(&mut self, block: u64) {
        self.is_loading = true;
        self.loading_block = Some(block);
        self.data_req_tx
            .send(DataRequest::Block(BlockId::Number(block)))
            .unwrap();
    }

    pub(crate) fn load_previous_block(&mut self) {
        if let Some(block) = self.effective_block()
            && block > 0
        {
            self.load_block(block - 1);
        }
    }

    pub(crate) fn load_next_block(&mut self) {
        if let Some(block) = self.effective_block() {
            self.load_block(block + 1);
        }
    }

    pub(crate) fn load_latest_block(&mut self) {
        self.is_loading = true;
        self.loading_block = None;
        self.data_req_tx
            .send(DataRequest::Block(BlockId::Latest))
            .unwrap();
    }
}
