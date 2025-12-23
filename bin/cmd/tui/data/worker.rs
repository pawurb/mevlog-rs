use crossbeam_channel::{Receiver, Sender};
use tokio::{runtime::Runtime, task::JoinHandle};

use crate::cmd::tui::{
    app::AppEvent,
    data::{DataRequest, DataResponse, fetcher::DataFetcher},
};

pub(crate) fn spawn_data_worker(
    data_req_rx: Receiver<DataRequest>,
    event_tx: Sender<AppEvent>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let rt = Runtime::new().expect("tokio runtime");

        // Keep only the currently running task so we can cancel it.
        let mut current: Option<JoinHandle<()>> = None;

        while let Ok(cmd) = data_req_rx.recv() {
            if let Some(h) = current.take() {
                h.abort();
            }

            match cmd {
                DataRequest::FetchLatest => {
                    let tx = event_tx.clone();
                    current = Some(rt.spawn(async move {
                        let fetcher = DataFetcher::new(None, None);
                        if let Ok(block_data) = fetcher.fetch("latest").await {
                            let block_num =
                                block_data.first().map(|tx| tx.block_number).unwrap_or(0);
                            let _ =
                                tx.send(AppEvent::Data(DataResponse::Block(block_num, block_data)));
                        }
                    }));
                }

                DataRequest::FetchBlock(block) => {
                    let tx = event_tx.clone();
                    current = Some(rt.spawn(async move {
                        let fetcher = DataFetcher::new(None, None);
                        if let Ok(block_data) = fetcher.fetch(block.to_string().as_str()).await {
                            let _ = tx.send(AppEvent::Data(DataResponse::Block(block, block_data)));
                        }
                    }));
                }

                DataRequest::FetchTx(_tx_hash) => {
                    current = Some(rt.spawn(async move { todo!() }));
                }
            }
        }
    })
}
