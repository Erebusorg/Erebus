//! Where a binary gets its node set from.
//!
//! Every Erebus binary that needs the node set — node, client, gateway — takes
//! the same two options, so a devnet reading a JSON file and a deployment
//! reading the registry contract differ by flags, not by code path.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Args;
use erebus_topology::Registry;

use crate::ChainRegistry;

#[derive(Args, Debug, Clone)]
pub struct RegistrySource {
    /// Registry file describing the node set. Use this on a local devnet, or
    /// with a set obtained somewhere a gateway cannot forge.
    #[arg(long)]
    pub registry: Option<PathBuf>,
    /// JSON-RPC endpoint of the chain holding the registry contract.
    #[arg(long, requires = "contract")]
    pub chain_rpc: Option<String>,
    /// Address of the NodeRegistry contract.
    #[arg(long, requires = "chain_rpc")]
    pub contract: Option<String>,
}

impl RegistrySource {
    pub async fn load(&self) -> Result<Registry> {
        match (&self.registry, &self.chain_rpc, &self.contract) {
            (Some(path), None, None) => {
                Registry::load(path).with_context(|| format!("reading {}", path.display()))
            }
            (None, Some(rpc), Some(contract)) => Ok(ChainRegistry::new(rpc.clone(), contract)?
                .fetch()
                .await
                .with_context(|| format!("reading the registry at {contract} through {rpc}"))?),
            (Some(_), Some(_), _) => {
                bail!("pass either --registry or --chain-rpc with --contract, not both")
            }
            _ => bail!("pass --registry, or --chain-rpc with --contract"),
        }
    }
}
