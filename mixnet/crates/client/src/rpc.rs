//! A destination service that speaks JSON-RPC to a chain node.
//!
//! This is the exit's view of a trade: a method name, a body, and no idea who
//! sent it. It is deliberately not a general proxy — it forwards a fixed set of
//! JSON-RPC methods to one upstream and refuses everything else, so an exit
//! operator cannot be asked to relay arbitrary traffic on a stranger's behalf,
//! and a client cannot use an exit as an open proxy.

use std::collections::HashSet;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};

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

/// The part of a JSON-RPC request the exit is allowed to look at.
#[derive(Debug, Deserialize)]
struct Call {
    #[serde(default)]
    id: Value,
    method: String,
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
            http: reqwest::Client::new(),
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

        match self.forward(body).await {
            Ok(answer) => answer,
            Err(err) => error(call.id, -32603, &format!("upstream: {err}")),
        }
    }

    async fn forward(&self, body: &[u8]) -> Result<Vec<u8>, reqwest::Error> {
        let response = self
            .http
            .post(&self.upstream)
            .header("content-type", "application/json")
            .body(body.to_vec())
            .send()
            .await?
            .error_for_status()?;
        Ok(response.bytes().await?.to_vec())
    }
}

/// Parses a call and decides whether this exit will carry it.
fn screen(body: &[u8], methods: &HashSet<String>) -> Result<Call, Vec<u8>> {
    let call: Call = serde_json::from_slice(body)
        .map_err(|err| error(Value::Null, -32700, &format!("parse error: {err}")))?;

    if !methods.contains(&call.method) {
        return Err(error(
            call.id.clone(),
            -32601,
            &format!("this exit does not forward {}", call.method),
        ));
    }
    Ok(call)
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
    }
}
