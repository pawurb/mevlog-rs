use std::{collections::HashMap, time::Duration};

use crossbeam_channel::{Receiver, Sender};
use mevlog::{
    ChainEntryJson,
    misc::{
        config::Config,
        rpc_urls::{get_chain_id_from_rpc, get_chain_info, get_chain_info_no_benchmark},
    },
};
use rand::seq::IndexedRandom;
use tokio::{runtime::Runtime, task::JoinHandle, time::timeout};
use tracing::{debug, error, info};

use crate::cmd::tui::{
    app::AppEvent,
    data::{
        BlockId, DataRequest, DataResponse,
        chains::fetch_chains,
        txs::{detect_trace_mode, fetch_opcodes, fetch_traces, fetch_tx_with_trace, fetch_txs},
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
    TxTrace,
    DetectTraceMode,
    RefreshRpc,
}

impl DataRequest {
    fn key(&self) -> RequestKey {
        match self {
            DataRequest::Block(..) => RequestKey::Block,
            DataRequest::Tx(..) => RequestKey::Tx,
            DataRequest::Chains(_) => RequestKey::Chains,
            DataRequest::ChainInfo(_) => RequestKey::ChainInfo,
            DataRequest::Opcodes(..) => RequestKey::Opcodes,
            DataRequest::Traces(..) => RequestKey::Traces,
            DataRequest::TxTrace(..) => RequestKey::TxTrace,
            DataRequest::DetectTraceMode(_) => RequestKey::DetectTraceMode,
            DataRequest::RefreshRpc(..) => RequestKey::RefreshRpc,
        }
    }
}

pub(crate) fn spawn_data_worker(
    data_req_rx: Receiver<DataRequest>,
    event_tx: Sender<AppEvent>,
) -> std::thread::JoinHandle<()> {
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
                DataRequest::Block(BlockId::Latest, opts) => {
                    info!("fetching latest block");
                    let tx = event_tx.clone();
                    let timeout_duration = Duration::from_millis(opts.block_timeout_ms);
                    rt.spawn(async move {
                        match timeout(
                            timeout_duration,
                            fetch_txs("latest", Some(opts.rpc_url), Some(opts.chain_id)),
                        )
                        .await
                        {
                            Ok(Ok(block_data)) => {
                                let block_num =
                                    block_data.first().map(|tx| tx.block_number).unwrap_or(0);
                                debug!(block_num, txs = block_data.len(), "fetched latest block");
                                let _ = tx.send(AppEvent::Data(DataResponse::Block(
                                    block_num, block_data,
                                )));
                            }
                            Ok(Err(e)) => {
                                error!(error = %e, "failed to fetch latest block");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                            Err(_) => {
                                error!("block fetch timed out");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(
                                    "block fetch timeout, press r to use different RPC endpoint"
                                        .to_string(),
                                )));
                            }
                        }
                    })
                }

                DataRequest::Block(BlockId::Number(block), opts) => {
                    info!(block, "fetching block");
                    let tx = event_tx.clone();
                    let timeout_duration = Duration::from_millis(opts.block_timeout_ms);
                    rt.spawn(async move {
                        match timeout(
                            timeout_duration,
                            fetch_txs(
                                block.to_string().as_str(),
                                Some(opts.rpc_url),
                                Some(opts.chain_id),
                            ),
                        )
                        .await
                        {
                            Ok(Ok(block_data)) => {
                                debug!(block, txs = block_data.len(), "fetched block");
                                let _ =
                                    tx.send(AppEvent::Data(DataResponse::Block(block, block_data)));
                            }
                            Ok(Err(e)) => {
                                error!(block, error = %e, "failed to fetch block");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                            Err(_) => {
                                error!(block, "block fetch timed out");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(
                                    "block fetch timeout, press r to use different RPC endpoint"
                                        .to_string(),
                                )));
                            }
                        }
                    })
                }

                DataRequest::Tx(_tx_hash, _opts) => rt.spawn(async move { todo!() }),

                DataRequest::Opcodes(tx_hash, trace_mode, opts) => {
                    info!(%tx_hash, ?trace_mode, "fetching opcodes");
                    let tx = event_tx.clone();
                    let hash = tx_hash.clone();
                    rt.spawn(async move {
                        match fetch_opcodes(
                            &hash,
                            Some(opts.rpc_url),
                            Some(opts.chain_id),
                            trace_mode,
                        )
                        .await
                        {
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

                DataRequest::Traces(tx_hash, trace_mode, opts) => {
                    info!(%tx_hash, ?trace_mode, "fetching traces");
                    let tx = event_tx.clone();
                    let hash = tx_hash.clone();
                    rt.spawn(async move {
                        match fetch_traces(
                            &hash,
                            Some(opts.rpc_url),
                            Some(opts.chain_id),
                            trace_mode,
                        )
                        .await
                        {
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

                DataRequest::TxTrace(tx_hash, trace_mode, opts) => {
                    info!(%tx_hash, ?trace_mode, "fetching tx with trace");
                    let tx = event_tx.clone();
                    let hash = tx_hash.clone();
                    rt.spawn(async move {
                        match fetch_tx_with_trace(
                            &hash,
                            Some(opts.rpc_url),
                            Some(opts.chain_id),
                            trace_mode,
                        )
                        .await
                        {
                            Ok(traced_tx) => {
                                debug!(tx_hash = %hash, "fetched tx with trace");
                                let _ = tx
                                    .send(AppEvent::Data(DataResponse::TxTraced(hash, traced_tx)));
                            }
                            Err(e) => {
                                error!(tx_hash = %hash, error = %e, "failed to fetch tx trace");
                                let _ = tx.send(AppEvent::Data(DataResponse::Error(e.to_string())));
                            }
                        }
                    })
                }

                DataRequest::DetectTraceMode(rpc_url) => {
                    info!(%rpc_url, "detecting trace mode");
                    let tx = event_tx.clone();
                    rt.spawn(async move {
                        let trace_mode = detect_trace_mode(&rpc_url).await;
                        debug!(?trace_mode, "detected trace mode");
                        let _ = tx.send(AppEvent::Data(DataResponse::TraceMode(trace_mode)));
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
                                            explorer_url: chain_info
                                                .explorers
                                                .first()
                                                .map(|e| e.url.clone()),
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

                DataRequest::RefreshRpc(chain_id, timeout_ms) => {
                    info!(chain_id, "refreshing RPC URL");
                    let tx = event_tx.clone();
                    rt.spawn(async move {
                        if let Ok(config) = Config::load()
                            && let Some(chain_cfg) = config.get_chain(chain_id)
                        {
                            info!(chain_id, rpc_url = %chain_cfg.rpc_url, "using RPC URL from config");
                            let _ = tx.send(AppEvent::Data(DataResponse::RpcRefreshed(
                                chain_cfg.rpc_url.clone(),
                            )));
                            return;
                        }

                        match get_chain_info(chain_id, timeout_ms, 3).await {
                            Ok(chain_info) => {
                                let top_rpcs: Vec<_> =
                                    chain_info.benchmarked_rpc_urls.iter().take(3).collect();
                                if let Some(&(rpc_url, _latency)) =
                                    top_rpcs.choose(&mut rand::rng())
                                {
                                    info!(chain_id, %rpc_url, "selected RPC URL from top 3");
                                    let _ = tx.send(AppEvent::Data(DataResponse::RpcRefreshed(
                                        rpc_url.clone(),
                                    )));
                                } else {
                                    error!(chain_id, "no working RPC URLs found");
                                    let _ = tx.send(AppEvent::Data(DataResponse::Error(
                                        "No working RPC URLs found".to_string(),
                                    )));
                                }
                            }
                            Err(e) => {
                                error!(chain_id, error = %e, "failed to refresh RPC");
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
