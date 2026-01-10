use std::collections::HashMap;

use crossbeam_channel::{Receiver, Sender};
use mevlog::{
    ChainEntryJson,
    misc::{
        rpc_urls::{get_chain_id_from_rpc, get_chain_info_no_benchmark},
        shared_init::ConnOpts,
    },
};
use tokio::{runtime::Runtime, task::JoinHandle};
use tracing::{debug, error, info};

use crate::cmd::tui::{
    app::AppEvent,
    data::{
        BlockId, DataRequest, DataResponse,
        chains::fetch_chains,
        txs::{fetch_opcodes, fetch_traces, fetch_txs},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RequestKey {
    Block,
    Tx,
    Chains,
    ChainInfo,
    Opcodes,
    Traces,
}

impl DataRequest {
    fn key(&self) -> RequestKey {
        match self {
            DataRequest::Block(_) => RequestKey::Block,
            DataRequest::Tx(_) => RequestKey::Tx,
            DataRequest::Chains(_) => RequestKey::Chains,
            DataRequest::ChainInfo(_) => RequestKey::ChainInfo,
            DataRequest::Opcodes(_) => RequestKey::Opcodes,
            DataRequest::Traces(_) => RequestKey::Traces,
        }
    }
}

pub(crate) fn spawn_data_worker(
    data_req_rx: Receiver<DataRequest>,
    event_tx: Sender<AppEvent>,
    conn_opts: &ConnOpts,
) -> std::thread::JoinHandle<()> {
    let conn_opts = conn_opts.clone();
    std::thread::spawn(move || {
        let rt = Runtime::new().expect("tokio runtime");
        let mut active_tasks: HashMap<RequestKey, JoinHandle<()>> = HashMap::new();

        while let Ok(cmd) = data_req_rx.recv() {
            let key = cmd.key();
            if let Some(h) = active_tasks.remove(&key)
                && !h.is_finished()
            {
                info!("Aborting previous task: {:?}", key);
                h.abort();
            }

            let handle = match cmd {
                DataRequest::Block(BlockId::Latest) => {
                    info!("fetching latest block");
                    let tx = event_tx.clone();
                    let rpc_url = conn_opts.rpc_url.clone();
                    let chain_id = conn_opts.chain_id;
                    rt.spawn(async move {
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
                    })
                }

                DataRequest::Block(BlockId::Number(block)) => {
                    info!(block, "fetching block");
                    let tx = event_tx.clone();
                    let rpc_url = conn_opts.rpc_url.clone();
                    let chain_id = conn_opts.chain_id;
                    rt.spawn(async move {
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
                    })
                }

                DataRequest::Tx(_tx_hash) => rt.spawn(async move { todo!() }),

                DataRequest::Opcodes(tx_hash) => {
                    info!(%tx_hash, "fetching opcodes");
                    let tx = event_tx.clone();
                    let rpc_url = conn_opts.rpc_url.clone();
                    let chain_id = conn_opts.chain_id;
                    let hash = tx_hash.clone();
                    rt.spawn(async move {
                        match fetch_opcodes(&hash, rpc_url, chain_id).await {
                            Ok(opcodes) => {
                                debug!(tx_hash = %hash, count = opcodes.len(), "fetched opcodes");
                                let _ =
                                    tx.send(AppEvent::Data(DataResponse::Opcodes(hash, opcodes)));
                            }
                            Err(e) => {
                                error!(tx_hash = %hash, error = %e, "failed to fetch opcodes");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                        }
                    })
                }

                DataRequest::Traces(tx_hash) => {
                    info!(%tx_hash, "fetching traces");
                    let tx = event_tx.clone();
                    let rpc_url = conn_opts.rpc_url.clone();
                    let chain_id = conn_opts.chain_id;
                    let hash = tx_hash.clone();
                    rt.spawn(async move {
                        match fetch_traces(&hash, rpc_url, chain_id).await {
                            Ok(traces) => {
                                debug!(tx_hash = %hash, count = traces.len(), "fetched traces");
                                let _ = tx.send(AppEvent::Data(DataResponse::Traces(hash, traces)));
                            }
                            Err(e) => {
                                error!(tx_hash = %hash, error = %e, "failed to fetch traces");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                        }
                    })
                }

                DataRequest::Chains(filter) => {
                    info!(?filter, "fetching chains");
                    let tx = event_tx.clone();
                    rt.spawn(async move {
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
                    })
                }

                DataRequest::ChainInfo(rpc_url) => {
                    info!(%rpc_url, "fetching chain info from RPC");
                    let tx = event_tx.clone();
                    rt.spawn(async move {
                        match get_chain_id_from_rpc(&rpc_url).await {
                            Ok(chain_id) => {
                                debug!(chain_id, "got chain_id from RPC");
                                match get_chain_info_no_benchmark(chain_id).await {
                                    Ok(chain_info) => {
                                        let entry = ChainEntryJson {
                                            chain_id,
                                            name: chain_info.name,
                                            chain: chain_info.chain,
                                            explorer_url: chain_info.explorers.first().map(|e| e.url.clone()),
                                        };
                                        let _ =
                                            tx.send(AppEvent::Data(DataResponse::ChainInfo(entry)));
                                    }
                                    Err(e) => {
                                        debug!(chain_id, error = %e, "chain not in ChainList, using fallback");
                                        let entry = ChainEntryJson {
                                            chain_id,
                                            name: format!("Chain {chain_id}"),
                                            chain: "Unknown".to_string(),
                                            explorer_url: None,
                                        };
                                        let _ =
                                            tx.send(AppEvent::Data(DataResponse::ChainInfo(entry)));
                                    }
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "failed to get chain_id from RPC");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                        }
                    })
                }
            };

            active_tasks.insert(key, handle);
        }
    })
}
