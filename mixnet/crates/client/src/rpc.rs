//! A destination service that speaks JSON-RPC to a chain node.
//!
//! This is the exit's view of a trade: a method name, a body, and no idea who
//! sent it. It is deliberately not a general proxy — it forwards a fixed set of
//! JSON-RPC methods to one upstream and refuses everything else, so an exit
//! operator cannot be asked to relay arbitrary traffic on a stranger's behalf,
//! and a client cannot use an exit as an open proxy.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tracing::debug;

use crate::sink::{Answer, Handler};

/// The methods an exit forwards by default: reads a wallet needs, and the one
/// write that matters. Everything else is refused.
pub const DEFAULT_METHODS: &[&str] = &[
    "eth_blockNumber",
    "eth_call",
    "eth_chainId",
    "eth_estimateGas",
    "eth_feeHistory",
    "eth_gasPrice",
    "eth_getBalance",
    "eth_getBlockByNumber",
    "eth_getCode",
    "eth_getLogs",
    "eth_getTransactionByHash",
    "eth_getTransactionCount",
    "eth_getTransactionReceipt",
    "eth_maxPriorityFeePerGas",
    "eth_sendRawTransaction",
    "net_version",
];

/// How long the exit will wait on its upstream, connection included. An
/// anonymous caller must not be able to pin a connection open indefinitely.
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(15);

/// The largest request the exit will screen. A request that arrives through the
/// mixnet is already bounded by the packet size; the exit's own listener is not.
pub const MAX_REQUEST: usize = 32 * 1024;

/// The most upstream response the exit will buffer. A reply has to fit in one
/// fixed-size packet anyway, so a larger answer could not be delivered — better
/// to stop reading it than to hold it in memory first.
pub const MAX_RESPONSE: usize = 28 * 1024;

/// The part of a JSON-RPC request the exit is allowed to look at, and the exact
/// bytes it will forward.
#[derive(Debug)]
struct Call {
    id: Value,
    method: String,
    /// The request re-serialised from what was screened. Forwarding the caller's
    /// original bytes would let an upstream parser read a different call than
    /// the one this exit agreed to carry.
    body: Vec<u8>,
}

/// A JSON object whose keys are unique.
///
/// `serde_json` keeps the last of a repeated key; other JSON-RPC servers keep
/// the first. A body carrying `method` twice would otherwise be screened as one
/// method and executed as another, which is the whole allowlist gone.
struct Unique(Map<String, Value>);

impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Keys;

        impl<'de> Visitor<'de> for Keys {
            type Value = Unique;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON-RPC request object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Unique, A::Error> {
                let mut object = Map::new();
                while let Some((key, value)) = entries.next_entry::<String, Value>()? {
                    if object.insert(key.clone(), value).is_some() {
                        return Err(de::Error::custom(format!("repeated key {key}")));
                    }
                }
                Ok(Unique(object))
            }
        }

        deserializer.deserialize_map(Keys)
    }
}

pub struct RpcService {
    upstream: String,
    methods: HashSet<String>,
    http: reqwest::Client,
}

impl RpcService {
    pub fn new(upstream: String, methods: impl IntoIterator<Item = String>) -> Self {
        Self {
            upstream,
            methods: methods.into_iter().collect(),
            http: reqwest::Client::builder()
                .timeout(UPSTREAM_TIMEOUT)
                .connect_timeout(UPSTREAM_TIMEOUT)
                .build()
                .expect("a default http client builds"),
        }
    }

    pub fn with_default_methods(upstream: String) -> Self {
        Self::new(upstream, DEFAULT_METHODS.iter().map(|m| (*m).to_string()))
    }

    pub fn handler(self) -> Handler {
        let service = Arc::new(self);
        Arc::new(move |body: Vec<u8>| -> Answer {
            let service = Arc::clone(&service);
            Box::pin(async move { Some(service.answer(&body).await) })
        })
    }

    async fn answer(&self, body: &[u8]) -> Vec<u8> {
        let call = match screen(body, &self.methods) {
            Ok(call) => call,
            Err(refusal) => return refusal,
        };
        debug!(method = %call.method, "forwarding a call upstream");

        match self.forward(call.body).await {
            Ok(answer) => answer,
            Err(err) => error(call.id, -32603, &format!("upstream: {err}")),
        }
    }

    async fn forward(&self, body: Vec<u8>) -> Result<Vec<u8>> {
        let mut response = self
            .http
            .post(&self.upstream)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        let mut answer = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if answer.len() + chunk.len() > MAX_RESPONSE {
                bail!("the answer is larger than a reply can carry");
            }
            answer.extend_from_slice(&chunk);
        }
        Ok(answer)
    }
}

/// Parses a call and decides whether this exit will carry it.
fn screen(body: &[u8], methods: &HashSet<String>) -> Result<Call, Vec<u8>> {
    if body.len() > MAX_REQUEST {
        return Err(error(Value::Null, -32600, "request too large"));
    }

    let Unique(object) = serde_json::from_slice(body)
        .map_err(|err| error(Value::Null, -32700, &format!("parse error: {err}")))?;

    let id = object.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Err(error(id, -32700, "parse error: no method"));
    };

    if !methods.contains(method) {
        return Err(error(
            id,
            -32601,
            &format!("this exit does not forward {method}"),
        ));
    }

    let method = method.to_string();
    let body = serde_json::to_vec(&Value::Object(object))
        .map_err(|err| error(id.clone(), -32603, &format!("internal error: {err}")))?;
    Ok(Call { id, method, body })
}

fn error(id: Value, code: i32, message: &str) -> Vec<u8> {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        .to_string()
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn methods() -> HashSet<String> {
        DEFAULT_METHODS.iter().map(|m| (*m).to_string()).collect()
    }

    fn refusal(body: &str) -> Value {
        let refused = screen(body.as_bytes(), &methods()).expect_err("refused");
        serde_json::from_slice(&refused).expect("json")
    }

    #[test]
    fn a_forwarded_method_is_accepted() {
        let call = screen(
            br#"{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}"#,
            &methods(),
        )
        .expect("accepted");
        assert_eq!(call.method, "eth_blockNumber");
    }

    #[test]
    fn a_method_outside_the_list_is_refused_with_the_callers_id() {
        let refused = refusal(r#"{"jsonrpc":"2.0","id":7,"method":"admin_peers"}"#);
        assert_eq!(refused["id"], 7);
        assert_eq!(refused["error"]["code"], -32601);
    }

    #[test]
    fn a_body_that_is_not_json_rpc_is_refused_rather_than_forwarded() {
        assert_eq!(refusal("not json")["error"]["code"], -32700);
        assert_eq!(refusal(r#"{"id":1}"#)["error"]["code"], -32700);
        assert_eq!(
            refusal(r#"[{"method":"eth_chainId"}]"#)["error"]["code"],
            -32700
        );
    }

    #[test]
    fn a_repeated_method_is_refused_rather_than_screened_as_the_last_one() {
        let refused = refusal(r#"{"id":1,"method":"admin_peers","method":"eth_chainId"}"#);
        assert_eq!(refused["error"]["code"], -32700);
        assert!(refused["error"]["message"]
            .as_str()
            .expect("a message")
            .contains("repeated key method"));
    }

    #[test]
    fn what_is_forwarded_is_what_was_screened() {
        let call = screen(
            br#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"to":"0x1"}]}"#,
            &methods(),
        )
        .expect("accepted");
        let forwarded: Value = serde_json::from_slice(&call.body).expect("json");
        assert_eq!(forwarded["method"], "eth_call");
        assert_eq!(forwarded["params"][0]["to"], "0x1");
    }

    #[test]
    fn a_request_larger_than_a_packet_is_refused_before_it_is_parsed() {
        let body = format!(
            r#"{{"id":1,"method":"eth_call","params":["{}"]}}"#,
            "a".repeat(MAX_REQUEST)
        );
        assert_eq!(refusal(&body)["error"]["code"], -32600);
    }
}
