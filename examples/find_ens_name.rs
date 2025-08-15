use std::sync::Arc;

use alloy::{providers::ProviderBuilder, rpc::client::RpcClient};
use eyre::Result;
use mevlog::misc::ens_utils::{ens_lookup_sync, namehash, reverse_address};
use revm::primitives::address;

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_url = std::env::var("ETH_URL").expect("ETH_URL must be set");
    let client = RpcClient::builder().http(rpc_url.parse()?);
    let provider = ProviderBuilder::new().connect_client(client);
    let provider = Arc::new(provider);

    // u know who
    let addr = address!("0xae2fc483527b8ef99eb5d9b44875f005ba1fae13");
    let name = reverse_address(&addr);
    let node = namehash(&name);
    println!("node: {node}");

    let name = ens_lookup_sync(addr, &provider).await?;
    println!("name: {name:?}");
    Ok(())
}
