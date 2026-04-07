use std::sync::Arc;

use eyre::Result;
use revm::primitives::Address;

use crate::{
    GenericProvider,
    misc::ens_utils::{ENSLookup, ens_lookup_async, ens_lookup_only_cached, ens_lookup_sync},
};

#[derive(Debug, Clone, PartialEq)]
pub struct MEVAddress {
    address: Address,
    ens_name: Option<String>,
}

#[hotpath::measure_all(future = true)]
impl MEVAddress {
    pub async fn new(
        address: Address,
        ens_lookup: &ENSLookup,
        provider: &Arc<GenericProvider>,
    ) -> Result<Self> {
        let ens_name = match ens_lookup {
            ENSLookup::Async(lookup_worker) => ens_lookup_async(address, lookup_worker).await?,
            ENSLookup::Sync => ens_lookup_sync(address, provider).await?,
            ENSLookup::OnlyCached => ens_lookup_only_cached(address).await?,
            ENSLookup::Disabled => None,
        };
        Ok(Self { address, ens_name })
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn ens_name(&self) -> Option<&str> {
        self.ens_name.as_deref()
    }
}
