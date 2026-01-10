//! UI state management - navigation and selection

use ratatui::widgets::TableState;

use mevlog::ChainEntryJson;

use crate::cmd::tui::app::{App, AppMode, DEFAULT_CHAINS, PrimaryTab, TxPopupTab};
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

    pub(crate) fn request_opcodes_if_needed(&mut self) {
        if let Some(idx) = self.table_state.selected()
            && let Some(tx) = self.items.get(idx)
        {
            let tx_hash = tx.tx_hash.to_string();

            if self.opcodes_tx_hash.as_ref() == Some(&tx_hash) {
                return;
            }

            self.opcodes = None;
            self.opcodes_loading = true;
            self.opcodes_tx_hash = Some(tx_hash.clone());

            let _ = self.data_req_tx.send(DataRequest::Opcodes(tx_hash));
        }
    }

    pub(crate) fn clear_opcodes(&mut self) {
        self.opcodes = None;
        self.opcodes_loading = false;
        self.opcodes_tx_hash = None;
    }

    pub(crate) fn request_traces_if_needed(&mut self) {
        if let Some(idx) = self.table_state.selected()
            && let Some(tx) = self.items.get(idx)
        {
            let tx_hash = tx.tx_hash.to_string();

            if self.traces_tx_hash.as_ref() == Some(&tx_hash) {
                return;
            }

            self.traces = None;
            self.traces_loading = true;
            self.traces_tx_hash = Some(tx_hash.clone());

            let _ = self.data_req_tx.send(DataRequest::Traces(tx_hash));
        }
    }

    pub(crate) fn clear_traces(&mut self) {
        self.traces = None;
        self.traces_loading = false;
        self.traces_tx_hash = None;
    }

    pub(crate) fn return_to_network_selection(&mut self) {
        self.items.clear();
        self.table_state = TableState::default();
        self.current_block = None;
        self.loading_block = None;
        self.is_loading = false;
        self.tx_popup_open = false;
        self.tx_popup_scroll = 0;
        self.tx_popup_tab = TxPopupTab::default();
        self.selected_chain = None;
        self.active_tab = PrimaryTab::Explore;
        self.error_message = None;
        self.clear_opcodes();
        self.clear_traces();
        self.conn_opts.chain_id = None;

        self.available_chains = DEFAULT_CHAINS
            .iter()
            .map(|(id, name, chain, explorer)| ChainEntryJson {
                chain_id: *id,
                name: name.to_string(),
                chain: chain.to_string(),
                explorer_url: Some(explorer.to_string()),
            })
            .collect();

        self.network_table_state = TableState::default().with_selected(Some(0));
        self.search_query.clear();
        self.search_popup_open = false;

        self.mode = AppMode::SelectNetwork;
    }
}
