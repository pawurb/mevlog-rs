//! UI state management - navigation and selection

use ratatui::widgets::TableState;

use mevlog::ChainEntryJson;

use crate::cmd::tui::{
    app::{App, AppMode, DEFAULT_CHAINS, PrimaryTab, TxPopupTab},
    data::{BlockId, DataRequest, TraceMode},
};

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
            self.chain_id = Some(chain.chain_id);
            self.rpc_url = None;

            self.mode = AppMode::Main;
            self.is_loading = true;
            self.rpc_refreshing = true;

            let _ = self
                .data_req_tx
                .send(DataRequest::RefreshRpc(chain.chain_id, self.rpc_timeout_ms));

            self.available_chains.clear();
            self.search_query.clear();
        }
    }

    pub(crate) fn open_network_selection(&mut self) {
        self.mode = AppMode::SelectNetwork;
        self.tx_popup_open = false;
        self.info_popup_open = false;
        self.available_chains = DEFAULT_CHAINS
            .iter()
            .map(|(id, name, chain, explorer)| ChainEntryJson {
                chain_id: *id,
                name: name.to_string(),
                chain: chain.to_string(),
                explorer_url: Some(explorer.to_string()),
            })
            .collect();
        self.network_table_state.select(Some(0));
    }

    pub(crate) fn can_return_to_main(&self) -> bool {
        self.selected_chain.is_some() || self.rpc_url.is_some()
    }

    pub(crate) fn return_to_main(&mut self) {
        if self.can_return_to_main() {
            self.mode = AppMode::Main;
            self.available_chains.clear();
            self.search_query.clear();
        }
    }

    pub(crate) fn request_opcodes_if_needed(&mut self) {
        let Some(opts) = self.rpc_opts() else {
            return;
        };
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

            let trace_mode = self.trace_mode.clone().unwrap_or(TraceMode::Revm);
            let _ = self
                .data_req_tx
                .send(DataRequest::Opcodes(tx_hash, trace_mode, opts));
        }
    }

    pub(crate) fn clear_opcodes(&mut self) {
        self.opcodes = None;
        self.opcodes_loading = false;
        self.opcodes_tx_hash = None;
    }

    pub(crate) fn request_traces_if_needed(&mut self) {
        let Some(opts) = self.rpc_opts() else {
            return;
        };
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

            let trace_mode = self.trace_mode.clone().unwrap_or(TraceMode::Revm);
            let _ = self
                .data_req_tx
                .send(DataRequest::Traces(tx_hash, trace_mode, opts));
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
        self.chain_id = None;
        self.rpc_url = None;
        self.trace_mode = None;
        self.info_popup_open = false;
        self.rpc_refreshing = false;

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

    pub(crate) fn request_rpc_refresh(&mut self) {
        if self.rpc_refreshing {
            return;
        }

        let cid = self
            .selected_chain
            .as_ref()
            .map(|c| c.chain_id)
            .or(self.chain_id);

        if let Some(cid) = cid {
            self.rpc_refreshing = true;
            let _ = self
                .data_req_tx
                .send(DataRequest::RefreshRpc(cid, self.rpc_timeout_ms));
        }
    }

    pub(crate) fn handle_rpc_refreshed(&mut self, new_rpc_url: String) {
        self.rpc_refreshing = false;
        self.rpc_url = Some(new_rpc_url.clone());

        if let Some(opts) = self.rpc_opts() {
            self.is_loading = true;
            let _ = self
                .data_req_tx
                .send(DataRequest::Block(BlockId::Latest, opts));
        }
        let _ = self
            .data_req_tx
            .send(DataRequest::DetectTraceMode(new_rpc_url));
    }
}
