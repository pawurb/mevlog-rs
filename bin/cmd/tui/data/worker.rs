use std::sync::mpsc::{Receiver, Sender};

use tokio::{runtime::Runtime, task::JoinHandle};

use crate::cmd::tui::data::{DataRequest, DataResponse, fetcher::DataFetcher};

pub(crate) fn spawn_data_worker(
    data_req_rx: Receiver<DataRequest>,
    data_resp_tx: Sender<DataResponse>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let rt = Runtime::new().expect("tokio runtime");
        let client = reqwest::Client::new();

        // Keep only the currently running task so we can cancel it.
        let mut current: Option<JoinHandle<()>> = None;

        while let Ok(cmd) = data_req_rx.recv() {
            match cmd {
                DataRequest::FetchBlock(block) => {
                    if let Some(h) = current.take() {
                        h.abort();
                    }

                    let resp_tx = data_resp_tx.clone();

                    current = Some(rt.spawn(async move {
                        let fetcher = DataFetcher::new(None, None);
                        let block_data = fetcher
                            .fetch(block.to_string().as_str())
                            .await
                            .expect("Fixme");
                        let _ = resp_tx.send(DataResponse::Block(block, block_data));
                    }));
                }

                DataRequest::FetchTx(tx_hash) => {
                    if let Some(h) = current.take() {
                        h.abort();
                    }

                    current = Some(rt.spawn(async move { todo!() }));
                }
            }
        }
    })
}
