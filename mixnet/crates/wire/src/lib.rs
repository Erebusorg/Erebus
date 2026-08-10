//! Framing.
//!
//! Mix links carry bare [`PACKET_SIZE`] frames: no length prefix, because every
//! packet is the same size and a length field would be a place to hide a mark.
//! Delivery to a destination service is length-prefixed, since that traffic has
//! left the mixnet and is no longer size-constrained.

use anyhow::{bail, Result};
use erebus_sphinx::{Packet, PACKET_SIZE};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn read_packet<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Option<Packet>> {
    let mut buf = vec![0u8; PACKET_SIZE];
    match reader.read_exact(&mut buf).await {
        Ok(_) => Ok(Some(Packet::from_bytes(&buf)?)),
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub async fn send_packet(address: &str, packet: &Packet) -> Result<()> {
    let mut stream = TcpStream::connect(address).await?;
    stream.write_all(&packet.to_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

pub async fn send_message(address: &str, message: &[u8]) -> Result<()> {
    let mut stream = TcpStream::connect(address).await?;
    stream
        .write_all(&(message.len() as u32).to_le_bytes())
        .await?;
    stream.write_all(message).await?;
    stream.flush().await?;
    Ok(())
}

pub async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len).await {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }

    let len = u32::from_le_bytes(len) as usize;
    if len > PACKET_SIZE {
        bail!("message of {len} bytes is larger than a packet");
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

/// Delivery tags are a 32-byte field holding a zero-padded `host:port`.
pub fn tag_from_address(address: &str) -> Result<[u8; 32]> {
    let bytes = address.as_bytes();
    if bytes.len() > 32 {
        bail!("address `{address}` does not fit in a 32 byte delivery tag");
    }
    let mut tag = [0u8; 32];
    tag[..bytes.len()].copy_from_slice(bytes);
    Ok(tag)
}

pub fn address_from_tag(tag: &[u8; 32]) -> Result<String> {
    let end = tag.iter().position(|b| *b == 0).unwrap_or(tag.len());
    Ok(String::from_utf8(tag[..end].to_vec())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_tags_round_trip() {
        let tag = tag_from_address("127.0.0.1:9100").unwrap();
        assert_eq!(address_from_tag(&tag).unwrap(), "127.0.0.1:9100");
    }

    #[test]
    fn an_oversized_address_is_rejected() {
        assert!(tag_from_address(&"a".repeat(33)).is_err());
    }
}
