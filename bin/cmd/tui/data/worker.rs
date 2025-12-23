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
            match cmd {
                DataRequest::FetchBlock(block) => {
                    if let Some(h) = current.take() {
                        h.abort();
                    }

                    let tx = event_tx.clone();

                    current = Some(rt.spawn(async move {
                        let fetcher = DataFetcher::new(None, None);
                        let block_data = fetcher
                            .fetch(block.to_string().as_str())
                            .await
                            .expect("Fixme");
                        let _ = tx.send(AppEvent::Data(DataResponse::Block(block, block_data)));
                    }));
                }

                DataRequest::FetchTx(_tx_hash) => {
                    if let Some(h) = current.take() {
                        h.abort();
                    }

                    current = Some(rt.spawn(async move { todo!() }));
                }
            }
        }
    })
}
