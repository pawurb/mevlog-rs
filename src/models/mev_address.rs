use colored::Colorize;
use eyre::Result;
use revm::primitives::Address;
use std::{fmt, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    misc::ens_utils::{ens_reverse_lookup_cached_async, ens_reverse_lookup_cached_sync},
    GenericProvider,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MEVAddress {
    address: Address,
    ens_name: Option<String>,
}

impl MEVAddress {
    pub async fn new(
        address: Address,
        ens_lookup: Option<&UnboundedSender<Address>>,
        provider: &Arc<GenericProvider>,
    ) -> Result<Self> {
        let ens_name = match ens_lookup {
            Some(sender) => ens_reverse_lookup_cached_async(address, sender).await?,
            None => ens_reverse_lookup_cached_sync(address, provider).await?,
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

impl fmt::Display for MEVAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.ens_name {
            Some(name) => write!(f, "{}", format!("<{}>", name).yellow()),
            None => write!(f, "{}", self.address.to_string().yellow()),
        }
    }
}
