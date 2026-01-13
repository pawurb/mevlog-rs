//! Data management - fetching and updating transaction data

use crate::cmd::tui::{
    app::App,
    data::{BlockId, DataRequest},
};

#[hotpath::measure_all]
impl App {
    fn effective_block(&self) -> Option<u64> {
        self.loading_block.or(self.current_block)
    }

    pub(crate) fn load_block(&mut self, block: u64) {
        let Some(opts) = self.rpc_opts() else {
            return;
        };
        self.is_loading = true;
        self.loading_block = Some(block);
        self.data_req_tx
            .send(DataRequest::Block(BlockId::Number(block), opts))
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
        let Some(opts) = self.rpc_opts() else {
            return;
        };
        self.is_loading = true;
        self.loading_block = None;
        self.data_req_tx
            .send(DataRequest::Block(BlockId::Latest, opts))
            .unwrap();
    }
}
