//! UI state management - navigation and selection

use mevlog::ChainEntryJson;

use super::{App, AppMode, DEFAULT_CHAINS};
use crate::cmd::tui::data::{BlockId, DataRequest, worker::spawn_data_worker};

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

    pub(crate) fn select_next_network(&mut self) {
        let count = self.available_chains.len();
        if count == 0 {
            return;
        }
        let i = match self.network_table_state.selected() {
            Some(i) => (i + 1).min(count - 1),
            None => 0,
        };
        self.network_table_state.select(Some(i));
    }

    pub(crate) fn select_previous_network(&mut self) {
        let count = self.available_chains.len();
        if count == 0 {
            return;
        }
        let i = match self.network_table_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.network_table_state.select(Some(i));
    }

    pub(crate) fn request_filtered_chains(&mut self) {
        if self.search_query.is_empty() {
            self.available_chains = DEFAULT_CHAINS
                .iter()
                .map(|(id, name, chain, explorer)| ChainEntryJson {
                    chain_id: *id,
                    name: name.to_string(),
                    chain: chain.to_string(),
                    explorer_url: Some(explorer.to_string()),
                })
                .collect();
            self.is_loading = false;
        } else {
            self.is_loading = true;
            let _ = self
                .data_req_tx
                .send(DataRequest::Chains(Some(self.search_query.clone())));
        }
    }

    pub(crate) fn confirm_network_selection(&mut self) {
        if let Some(selected_idx) = self.network_table_state.selected()
            && let Some(chain) = self.available_chains.get(selected_idx)
        {
            self.selected_chain = Some(chain.clone());
            self.conn_opts.chain_id = Some(chain.chain_id);

            let (data_req_tx, data_req_rx) = crossbeam_channel::unbounded();

            spawn_data_worker(data_req_rx, self.state_tx.clone(), &self.conn_opts);

            self.data_req_tx = data_req_tx;

            self.mode = AppMode::Main;
            self.is_loading = true;

            let _ = self.data_req_tx.send(DataRequest::Block(BlockId::Latest));

            self.available_chains.clear();
            self.search_query.clear();
        }
    }
}
