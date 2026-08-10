//! Cover traffic.
//!
//! Mixing hides which packet is which, but not that a client sent one. A client
//! that only transmits when it has something to say leaks its trading activity
//! in the timing of its packets alone. So the client transmits at a rate that
//! does not depend on what it is doing: real packets take the place of cover
//! packets in the stream rather than adding to it.

use std::sync::Arc;

use erebus_topology::exponential_delay;
use rand::rngs::OsRng;
use tokio::time::{sleep, Duration};
use tracing::warn;

use crate::Client;

pub struct CoverConfig {
    /// Mean gap between packets. The stream is Poisson, so an observer sees a
    /// constant average rate with no periodicity to lock onto.
    pub mean_interval_ms: f64,
    /// Where cover requests are addressed. Cover is marked inside the encrypted
    /// envelope, so only the destination service can tell it apart from a real
    /// request — no hop can.
    pub destination: String,
    /// Fraction of cover packets sent as loop probes back to the client instead,
    /// which double as liveness checks on the path.
    pub probe_ratio: f64,
}

/// Emits cover traffic until the future is dropped.
pub async fn run(client: Arc<Client>, config: CoverConfig) {
    loop {
        let gap = exponential_delay(&mut OsRng, config.mean_interval_ms);
        sleep(Duration::from_millis(gap as u64)).await;

        let probe = rand::Rng::gen_bool(&mut OsRng, config.probe_ratio.clamp(0.0, 1.0));
        let sent = if probe {
            client
                .loop_probe(Duration::from_secs(10))
                .await
                .map(|elapsed| {
                    tracing::debug!(?elapsed, "loop probe returned");
                })
        } else {
            client.send_cover(&config.destination).await
        };

        if let Err(err) = sent {
            warn!(%err, "cover traffic failed");
        }
    }
}
