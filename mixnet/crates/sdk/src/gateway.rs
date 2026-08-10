//! The wire protocol between a browser and its gateway.
//!
//! A browser cannot listen on a socket, so it cannot be the address a reply is
//! delivered to and it cannot dial an entry node itself. The gateway does both
//! on its behalf. It is deliberately dumb: it sees the client's address and the
//! first hop, which is exactly what the client's own network link would see
//! anyway, and it cannot read the packet, learn the path beyond the first hop,
//! or open a reply.

use wasm_bindgen::prelude::*;

use crate::SdkError;

/// Client to gateway: dial `first_hop` and write this packet to it.
pub const SEND: u8 = 0x01;
/// Client to gateway: route a delivery carrying this id back to me.
pub const EXPECT: u8 = 0x02;
/// Gateway to client: the address replies should be addressed to, and the
/// registry the gateway is serving.
pub const HELLO: u8 = 0x81;
/// Gateway to client: a frame the mixnet delivered.
pub const DELIVER: u8 = 0x82;

/// What the gateway told a client when it connected.
#[wasm_bindgen]
pub struct Hello {
    tag: String,
    registry: String,
}

#[wasm_bindgen]
impl Hello {
    /// The `host:port` at which the gateway receives mixnet deliveries.
    #[wasm_bindgen(getter)]
    pub fn tag(&self) -> String {
        self.tag.clone()
    }

    /// The registry, as JSON. A client is free to ignore it and use one it
    /// obtained elsewhere; nothing the gateway says about the topology is
    /// trusted beyond the first hop it is asked to dial.
    #[wasm_bindgen(getter)]
    pub fn registry(&self) -> String {
        self.registry.clone()
    }
}

#[wasm_bindgen]
pub fn encode_send(first_hop: &[u8], packet: &[u8]) -> Result<Vec<u8>, SdkError> {
    if first_hop.len() != 32 {
        return Err(SdkError::Frame("a node id is 32 bytes".into()));
    }
    let mut out = Vec::with_capacity(1 + 32 + packet.len());
    out.push(SEND);
    out.extend_from_slice(first_hop);
    out.extend_from_slice(packet);
    Ok(out)
}

#[wasm_bindgen]
pub fn encode_expect(id: &[u8]) -> Result<Vec<u8>, SdkError> {
    if id.len() != 32 {
        return Err(SdkError::Frame("an id is 32 bytes".into()));
    }
    let mut out = Vec::with_capacity(33);
    out.push(EXPECT);
    out.extend_from_slice(id);
    Ok(out)
}

pub fn encode_hello(tag: &str, registry_json: &str) -> Result<Vec<u8>, SdkError> {
    let tag = tag.as_bytes();
    if tag.len() > 32 {
        return Err(SdkError::GatewayTag(String::from_utf8_lossy(tag).into()));
    }
    let mut out = Vec::with_capacity(2 + tag.len() + registry_json.len());
    out.push(HELLO);
    out.push(tag.len() as u8);
    out.extend_from_slice(tag);
    out.extend_from_slice(registry_json.as_bytes());
    Ok(out)
}

pub fn encode_deliver(frame: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + frame.len());
    out.push(DELIVER);
    out.extend_from_slice(frame);
    out
}

/// Reads a gateway greeting. Returns nothing for any other message, so a caller
/// can hand it every message it receives.
#[wasm_bindgen]
pub fn decode_hello(message: &[u8]) -> Result<Option<Hello>, SdkError> {
    if message.first() != Some(&HELLO) {
        return Ok(None);
    }
    let len = *message
        .get(1)
        .ok_or_else(|| SdkError::Frame("truncated greeting".into()))? as usize;
    let end = 2 + len;
    if end > message.len() {
        return Err(SdkError::Frame("truncated greeting".into()));
    }
    Ok(Some(Hello {
        tag: String::from_utf8_lossy(&message[2..end]).into_owned(),
        registry: String::from_utf8_lossy(&message[end..]).into_owned(),
    }))
}

/// Reads a delivered frame. Returns nothing for any other message.
#[wasm_bindgen]
pub fn decode_deliver(message: &[u8]) -> Option<Vec<u8>> {
    match message.first() {
        Some(&DELIVER) => Some(message[1..].to_vec()),
        _ => None,
    }
}

/// Reads a client's request to send, as the gateway sees it.
pub fn decode_send(message: &[u8]) -> Result<([u8; 32], &[u8]), SdkError> {
    if message.first() != Some(&SEND) || message.len() < 33 {
        return Err(SdkError::Frame("not a send".into()));
    }
    let mut first_hop = [0u8; 32];
    first_hop.copy_from_slice(&message[1..33]);
    Ok((first_hop, &message[33..]))
}

/// Reads a client's registration of an id it is waiting for.
pub fn decode_expect(message: &[u8]) -> Result<[u8; 32], SdkError> {
    if message.first() != Some(&EXPECT) || message.len() != 33 {
        return Err(SdkError::Frame("not an expectation".into()));
    }
    let mut id = [0u8; 32];
    id.copy_from_slice(&message[1..33]);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_send_round_trips() {
        let encoded = encode_send(&[3u8; 32], &[9u8; 64]).unwrap();
        let (first_hop, packet) = decode_send(&encoded).unwrap();
        assert_eq!(first_hop, [3u8; 32]);
        assert_eq!(packet, &[9u8; 64]);
        assert!(encode_send(&[3u8; 8], &[]).is_err());
    }

    #[test]
    fn an_expectation_round_trips() {
        let encoded = encode_expect(&[5u8; 32]).unwrap();
        assert_eq!(decode_expect(&encoded).unwrap(), [5u8; 32]);
        assert!(decode_expect(&encoded[..10]).is_err());
    }

    #[test]
    fn a_greeting_round_trips() {
        let encoded = encode_hello("127.0.0.1:9200", r#"{"epoch_seed":"e","nodes":[]}"#).unwrap();
        let hello = decode_hello(&encoded).unwrap().unwrap();
        assert_eq!(hello.tag(), "127.0.0.1:9200");
        assert_eq!(hello.registry(), r#"{"epoch_seed":"e","nodes":[]}"#);
        assert!(encode_hello(&"a".repeat(33), "{}").is_err());
    }

    #[test]
    fn a_delivery_round_trips_and_is_told_apart_from_a_greeting() {
        let encoded = encode_deliver(b"frame");
        assert_eq!(decode_deliver(&encoded).unwrap(), b"frame");
        assert!(decode_hello(&encoded).unwrap().is_none());
        assert!(decode_deliver(&encode_hello("a", "{}").unwrap()).is_none());
    }
}
