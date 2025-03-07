use std::sync::Arc;

use alloy::providers::{ProviderBuilder, WsConnect};
use eyre::Result;
use mevlog::misc::ens_utils::{ens_reverse_lookup_cached_sync, namehash, reverse_address};
use revm::primitives::address;

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_url = std::env::var("ETH_RPC_WS_URL").expect("ETH_RPC_WS_URL must be set");
    let ws = WsConnect::new(rpc_url);
    let provider = ProviderBuilder::new().on_ws(ws).await?;
    let provider = Arc::new(provider);

    // u know who
    let addr = address!("0xae2fc483527b8ef99eb5d9b44875f005ba1fae13");
    let name = reverse_address(&addr);
    let node = namehash(&name);
    dbg!(node);

    let name = ens_reverse_lookup_cached_sync(addr, &provider).await?;
    dbg!(name);
    Ok(())
}
