//! The bytes between PC and phone (docs/pc-to-phone.md): sealed UDP packets to find the phone, and
//! a TCP session whose frames are sealed with a key made fresh for that session.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Code, Result, VemError};

/// Every packet and every session starts with this, so stray traffic is dropped at once.
const MAGIC: &[u8; 4] = b"VEM1";
const NONCE: usize = 24;
/// The largest message that is copied; a bigger one is skipped and counted.
pub const MAX_MESSAGE: usize = 128 << 20;
/// A frame holds one message plus its description.
const MAX_FRAME: usize = MAX_MESSAGE + (64 << 10);

pub const KIND_LIST: u8 = 1;
pub const KIND_WANTS: u8 = 2;
pub const KIND_MAIL: u8 = 3;
pub const KIND_DONE: u8 = 4;
pub const KIND_RESULT: u8 = 5;

fn broken(what: &str) -> VemError {
    VemError::new(Code::LanCopy01).with_detail(what.to_string())
}

/// A UDP packet: the magic, a random nonce and the sealed JSON.
pub fn seal_packet<T: Serialize>(key: &[u8; 32], value: &T) -> Result<Vec<u8>> {
    let plain = serde_json::to_vec(value).map_err(|_| broken("json"))?;
    let nonce: [u8; NONCE] = rand::random();
    let sealed = XChaCha20Poly1305::new(key.into())
        .encrypt(&XNonce::from(nonce), plain.as_ref())
        .map_err(|_| broken("seal"))?;
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Opens a packet sealed with the same key; anything else gives None.
pub fn open_packet<T: DeserializeOwned>(key: &[u8; 32], packet: &[u8]) -> Option<T> {
    let rest = packet.strip_prefix(MAGIC)?;
    if rest.len() < NONCE {
        return None;
    }
    let (nonce, sealed) = rest.split_at(NONCE);
    let mut n = [0u8; NONCE];
    n.copy_from_slice(nonce);
    let plain = XChaCha20Poly1305::new(key.into()).decrypt(&XNonce::from(n), sealed).ok()?;
    serde_json::from_slice(&plain).ok()
}

/// One TCP session. Each side sends 24 random bytes first; the session key is made from the
/// pairing key and both, so a recorded session cannot be played again. Frames carry a counter per
/// direction in their nonce, so none can be dropped, repeated or reordered unnoticed.
pub struct Channel<S> {
    stream: S,
    cipher: XChaCha20Poly1305,
    is_pc: bool,
    sent: u64,
    received: u64,
}

fn frame_nonce(from_pc: bool, counter: u64) -> XNonce {
    let mut n = [0u8; NONCE];
    n[0] = if from_pc { 1 } else { 2 };
    n[NONCE - 8..].copy_from_slice(&counter.to_be_bytes());
    XNonce::from(n)
}

impl<S: AsyncRead + AsyncWrite + Unpin> Channel<S> {
    pub async fn open(mut stream: S, pair_key: &[u8; 32], is_pc: bool) -> Result<Self> {
        let mine: [u8; NONCE] = rand::random();
        let mut hello = MAGIC.to_vec();
        hello.extend_from_slice(&mine);
        stream.write_all(&hello).await.map_err(|_| broken("hello"))?;
        let mut theirs = [0u8; 4 + NONCE];
        stream.read_exact(&mut theirs).await.map_err(|_| broken("hello"))?;
        if &theirs[..4] != MAGIC {
            return Err(broken("magic"));
        }
        let (phone, pc) = if is_pc { (&theirs[4..], &mine[..]) } else { (&mine[..], &theirs[4..]) };
        let mut h = Sha256::new();
        h.update(b"vem-lan-session-1");
        h.update(pair_key);
        h.update(phone);
        h.update(pc);
        let session: [u8; 32] = h.finalize().into();
        Ok(Channel { stream, cipher: XChaCha20Poly1305::new((&session).into()), is_pc, sent: 0, received: 0 })
    }

    pub async fn send(&mut self, kind: u8, body: &[u8]) -> Result<()> {
        let mut plain = Vec::with_capacity(body.len() + 1);
        plain.push(kind);
        plain.extend_from_slice(body);
        let sealed = self
            .cipher
            .encrypt(&frame_nonce(self.is_pc, self.sent), plain.as_ref())
            .map_err(|_| broken("seal"))?;
        self.sent += 1;
        let len = u32::try_from(sealed.len()).map_err(|_| broken("too big"))?;
        self.stream.write_all(&len.to_be_bytes()).await.map_err(|_| broken("write"))?;
        self.stream.write_all(&sealed).await.map_err(|_| broken("write"))?;
        self.stream.flush().await.map_err(|_| broken("write"))?;
        Ok(())
    }

    pub async fn send_json<T: Serialize>(&mut self, kind: u8, value: &T) -> Result<()> {
        let body = serde_json::to_vec(value).map_err(|_| broken("json"))?;
        self.send(kind, &body).await
    }

    pub async fn recv(&mut self) -> Result<(u8, Vec<u8>)> {
        let mut len = [0u8; 4];
        self.stream.read_exact(&mut len).await.map_err(|_| broken("read"))?;
        let len = u32::from_be_bytes(len) as usize;
        if len > MAX_FRAME + 64 {
            return Err(broken("frame too big"));
        }
        let mut sealed = vec![0u8; len];
        self.stream.read_exact(&mut sealed).await.map_err(|_| broken("read"))?;
        let plain = self
            .cipher
            .decrypt(&frame_nonce(!self.is_pc, self.received), sealed.as_ref())
            .map_err(|_| broken("not ours"))?;
        self.received += 1;
        let (&kind, body) = plain.split_first().ok_or_else(|| broken("empty"))?;
        Ok((kind, body.to_vec()))
    }

    /// The next frame, which must be of this kind, as JSON.
    pub async fn recv_json<T: DeserializeOwned>(&mut self, kind: u8) -> Result<T> {
        let (k, body) = self.recv().await?;
        if k != kind {
            return Err(broken("unexpected frame"));
        }
        serde_json::from_slice(&body).map_err(|_| broken("json"))
    }
}

/// A message frame: the length of its description, the description, then the message itself.
pub fn mail_frame<T: Serialize>(meta: &T, raw: &[u8]) -> Result<Vec<u8>> {
    let meta = serde_json::to_vec(meta).map_err(|_| broken("json"))?;
    let len = u32::try_from(meta.len()).map_err(|_| broken("too big"))?;
    let mut out = Vec::with_capacity(4 + meta.len() + raw.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&meta);
    out.extend_from_slice(raw);
    Ok(out)
}

pub fn split_mail_frame<T: DeserializeOwned>(body: &[u8]) -> Result<(T, &[u8])> {
    let bad = || broken("mail frame");
    if body.len() < 4 {
        return Err(bad());
    }
    let len = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
    let rest = &body[4..];
    if rest.len() < len {
        return Err(bad());
    }
    let meta = serde_json::from_slice(&rest[..len]).map_err(|_| bad())?;
    Ok((meta, &rest[len..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_opens_only_with_the_same_key() {
        let key = [7u8; 32];
        let p = seal_packet(&key, &("hallo", 3)).unwrap();
        assert_eq!(open_packet::<(String, i32)>(&key, &p), Some(("hallo".into(), 3)));
        assert_eq!(open_packet::<(String, i32)>(&[8u8; 32], &p), None);
        let mut tampered = p.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(open_packet::<(String, i32)>(&key, &tampered), None);
        assert_eq!(open_packet::<(String, i32)>(&key, b"VEM1short"), None);
    }

    #[tokio::test]
    async fn session_frames_go_both_ways_and_refuse_a_wrong_key() {
        let (a, b) = tokio::io::duplex(1 << 16);
        let key = [1u8; 32];
        let pc = tokio::spawn(async move {
            let mut c = Channel::open(a, &key, true).await.unwrap();
            c.send(KIND_LIST, b"lijst").await.unwrap();
            c.recv().await.unwrap()
        });
        let mut phone = Channel::open(b, &key, false).await.unwrap();
        assert_eq!(phone.recv().await.unwrap(), (KIND_LIST, b"lijst".to_vec()));
        phone.send(KIND_WANTS, b"[1]").await.unwrap();
        assert_eq!(pc.await.unwrap(), (KIND_WANTS, b"[1]".to_vec()));

        let (a, b) = tokio::io::duplex(1 << 16);
        let pc = tokio::spawn(async move {
            let mut c = Channel::open(a, &[2u8; 32], true).await.unwrap();
            c.send(KIND_LIST, b"lijst").await.unwrap();
        });
        let mut phone = Channel::open(b, &key, false).await.unwrap();
        assert!(phone.recv().await.is_err(), "a frame sealed with another key is refused");
        pc.await.unwrap();
    }

    #[test]
    fn mail_frame_round_trip() {
        let f = mail_frame(&("a", 1), b"From: x\r\n\r\nhoi").unwrap();
        let ((s, n), raw): ((String, i32), &[u8]) = split_mail_frame(&f).unwrap();
        assert_eq!((s.as_str(), n, raw), ("a", 1, &b"From: x\r\n\r\nhoi"[..]));
        assert!(split_mail_frame::<(String, i32)>(&f[..3]).is_err());
    }
}
