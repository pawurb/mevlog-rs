//! Data management - fetching and updating transaction data

use super::App;
use crate::cmd::tui::data::DataRequest;

impl App {
    pub(crate) fn load_block(&mut self, block: u64) {
        self.data_req_tx
            .send(DataRequest::FetchBlock(block))
            .unwrap();
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
