use crossbeam_channel::{Receiver, Sender};
use mevlog::misc::shared_init::ConnOpts;
use tokio::{runtime::Runtime, task::JoinHandle};
use tracing::{debug, error, info};

use crate::cmd::tui::{
    app::AppEvent,
    data::{BlockId, DataRequest, DataResponse, chains::fetch_chains, txs::fetch_txs},
};

pub(crate) fn spawn_data_worker(
    data_req_rx: Receiver<DataRequest>,
    event_tx: Sender<AppEvent>,
    conn_opts: &ConnOpts,
) -> std::thread::JoinHandle<()> {
    let conn_opts = conn_opts.clone();
    std::thread::spawn(move || {
        let rt = Runtime::new().expect("tokio runtime");
        let mut current: Option<JoinHandle<()>> = None;

        while let Ok(cmd) = data_req_rx.recv() {
            if let Some(h) = current.take() {
                h.abort();
            }

            match cmd {
                DataRequest::Block(BlockId::Latest) => {
                    info!("fetching latest block");
                    let tx = event_tx.clone();
                    let rpc_url = conn_opts.rpc_url.clone();
                    let chain_id = conn_opts.chain_id;
                    current = Some(rt.spawn(async move {
                        match fetch_txs("latest", rpc_url, chain_id).await {
                            Ok(block_data) => {
                                let block_num =
                                    block_data.first().map(|tx| tx.block_number).unwrap_or(0);
                                debug!(block_num, txs = block_data.len(), "fetched latest block");
                                let _ = tx.send(AppEvent::Data(DataResponse::Block(
                                    block_num, block_data,
                                )));
                            }
                            Err(e) => {
                                error!(error = %e, "failed to fetch latest block");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                        }
                    }));
                }

                DataRequest::Block(BlockId::Number(block)) => {
                    info!(block, "fetching block");
                    let tx = event_tx.clone();
                    let rpc_url = conn_opts.rpc_url.clone();
                    let chain_id = conn_opts.chain_id;
                    current = Some(rt.spawn(async move {
                        match fetch_txs(block.to_string().as_str(), rpc_url, chain_id).await {
                            Ok(block_data) => {
                                debug!(block, txs = block_data.len(), "fetched block");
                                let _ =
                                    tx.send(AppEvent::Data(DataResponse::Block(block, block_data)));
                            }
                            Err(e) => {
                                error!(block, error = %e, "failed to fetch block");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                        }
                    }));
                }

                DataRequest::Tx(_tx_hash) => {
                    current = Some(rt.spawn(async move { todo!() }));
                }

                DataRequest::Chains(filter) => {
                    info!(?filter, "fetching chains");
                    let tx = event_tx.clone();
                    current = Some(rt.spawn(async move {
                        match fetch_chains(filter).await {
                            Ok(chains) => {
                                debug!(count = chains.len(), "fetched chains");
                                let _ = tx.send(AppEvent::Data(DataResponse::Chains(chains)));
                            }
                            Err(e) => {
                                error!(error = %e, "failed to fetch chains");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                        }
                    }));
                }
            }
        }
    })
}
