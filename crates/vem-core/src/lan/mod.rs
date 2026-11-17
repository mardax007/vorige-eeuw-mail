//! Mail kept on the PC, also on the paired phone (decision 26, docs/pc-to-phone.md).
//!
//! The PC finds the phone with a sealed UDP broadcast and connects to it; it never listens itself,
//! so Windows never asks about its firewall. Only private addresses take part.

pub mod mirror;
pub mod wire;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time::timeout;

use crate::contacts_sync;
use crate::error::{Code, Result, VemError};
use crate::log;
use crate::model::now;
use crate::secrets::Secrets;
use crate::store::Store;
use crate::store::lan::OfferedMessage;
use mirror::Manifest;
use wire::{Channel, KIND_DONE, KIND_LIST, KIND_MAIL, KIND_RESULT, KIND_WANTS};

/// The phone listens for the PC's broadcast here.
pub const FIND_PORT: u16 = 47801;
/// The phone takes the PC's connection here (or any free port, which it then names).
pub const COPY_PORT: u16 = 47802;
/// A find packet older or newer than this is not answered.
const MAX_CLOCK_SKEW: i64 = 600;
const ANSWER_WAIT: Duration = Duration::from_secs(2);
const STEP_WAIT: Duration = Duration::from_secs(60);
/// The phone remembers the fingerprint of the last list it applied in full.
const DIGEST_KEY: &str = "lan_digest";
/// The PC remembers when the phone last had everything.
pub const LAST_KEY: &str = "lan_last";

#[derive(Debug, Serialize, Deserialize)]
struct Find {
    v: u32,
    at: i64,
    nonce: String,
    digest: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Here {
    v: u32,
    nonce: String,
    port: u16,
    wants: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copied {
    pub received: u32,
    pub removed: u32,
    pub skipped: u32,
}

/// When the PC last found the phone up to date, and how many messages it sent then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastCopy {
    pub at: i64,
    pub sent: u32,
}

/// What the phone tells the screens while the PC's mail comes in.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PhoneEvent {
    Progress { done: u32, total: u32 },
    Done { received: u32, removed: u32 },
    /// The copy broke off (Wi-Fi gone, PC asleep); it starts again with the next find.
    Stopped,
    Failed { error: VemError },
}

fn broken(what: &str) -> VemError {
    VemError::new(Code::LanCopy01).with_detail(what.to_string())
}

/// Addresses of a home network (and this device itself, for tests).
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback(),
        IpAddr::V6(_) => false,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Where the PC's find packet goes: the broadcast address of every local IPv4 network (Windows
/// sends 255.255.255.255 out of one network card only), and 255.255.255.255 itself.
pub fn broadcast_targets(port: u16) -> Vec<SocketAddr> {
    let mut out: Vec<SocketAddr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.addr {
            if_addrs::IfAddr::V4(v4) if v4.ip.is_private() => v4.broadcast.map(|b| SocketAddr::new(IpAddr::V4(b), port)),
            _ => None,
        })
        .collect();
    out.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port));
    out.dedup();
    out
}

// ---------------------------------------------------------------- PC

/// The PC side, run every so often: when there is local mail and a paired phone answers, bring
/// the phone up to date. `connect_to` replaces the address the answer came from (emulator tests).
/// Returns None when nothing happened (no key, nothing to offer, no phone answered).
pub async fn offer(
    store: &Store,
    secrets: &Secrets,
    targets: &[SocketAddr],
    connect_to: Option<IpAddr>,
) -> Result<Option<Copied>> {
    let Some(key) = contacts_sync::key(secrets)? else { return Ok(None) };
    let list = mirror::manifest(store)?;
    // A PC that never had local mail has nothing to say. One that had and now has none still
    // tells the phone, so the copies go there too.
    let ever = store.kv_get::<LastCopy>(LAST_KEY)?.is_some();
    if list.accounts.is_empty() && !ever {
        return Ok(None);
    }
    let digest = mirror::digest(&list);
    let Some((phone, here)) = find_phone(&key, &digest, targets).await? else { return Ok(None) };
    if !here.wants {
        store.kv_set(LAST_KEY, &LastCopy { at: now(), sent: 0 })?;
        return Ok(Some(Copied::default()));
    }
    let addr = SocketAddr::new(connect_to.unwrap_or(phone.ip()), here.port);
    let stream = timeout(Duration::from_secs(5), TcpStream::connect(addr))
        .await
        .map_err(|_| broken("connect timeout"))?
        .map_err(|_| broken("connect"))?;
    let copied = send_all(store, &key, stream, &list).await?;
    store.kv_set(LAST_KEY, &LastCopy { at: now(), sent: copied.received })?;
    log::event("lan.sent", &[("sent", &copied.received.to_string()), ("removed", &copied.removed.to_string())]);
    Ok(Some(copied))
}

async fn find_phone(key: &[u8; 32], digest: &str, targets: &[SocketAddr]) -> Result<Option<(SocketAddr, Here)>> {
    let nonce = hex(&rand::random::<[u8; 16]>());
    let packet = wire::seal_packet(key, &Find { v: 1, at: now(), nonce: nonce.clone(), digest: digest.to_string() })?;
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.map_err(|_| broken("udp"))?;
    socket.set_broadcast(true).map_err(|_| broken("udp"))?;
    for t in targets {
        // One network card may refuse a broadcast; the others still go.
        let _ = socket.send_to(&packet, t).await;
    }
    let mut buf = [0u8; 2048];
    let deadline = tokio::time::Instant::now() + ANSWER_WAIT;
    loop {
        let Ok(got) = tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await else { return Ok(None) };
        let Ok((n, from)) = got else { continue };
        if !is_private(from.ip()) {
            continue;
        }
        if let Some(here) = wire::open_packet::<Here>(key, &buf[..n])
            && here.v == 1
            && here.nonce == nonce
        {
            return Ok(Some((from, here)));
        }
    }
}

async fn send_all(store: &Store, key: &[u8; 32], stream: TcpStream, list: &Manifest) -> Result<Copied> {
    let mut ch = Channel::open(stream, key, true).await?;
    ch.send_json(KIND_LIST, list).await?;
    let wants: Vec<i64> = timeout(STEP_WAIT, ch.recv_json(KIND_WANTS)).await.map_err(|_| broken("wants timeout"))??;
    let by_id: HashMap<i64, &OfferedMessage> = list.messages.iter().map(|m| (m.id, m)).collect();
    for id in wants {
        let Some(meta) = by_id.get(&id) else { continue };
        let Some(raw) = store.read_eml(id)? else { continue };
        if raw.len() > wire::MAX_MESSAGE {
            continue;
        }
        ch.send(KIND_MAIL, &wire::mail_frame(meta, &raw)?).await?;
    }
    ch.send(KIND_DONE, b"").await?;
    timeout(STEP_WAIT, ch.recv_json::<Copied>(KIND_RESULT)).await.map_err(|_| broken("result timeout"))?
}

// ---------------------------------------------------------------- phone

/// Where the phone listens: all its networks in the app, only 127.0.0.1 in tests.
#[derive(Debug, Clone, Copy)]
pub struct Listen {
    pub ip: Ipv4Addr,
    pub find_port: u16,
    pub copy_port: u16,
}

impl Default for Listen {
    fn default() -> Self {
        Listen { ip: Ipv4Addr::UNSPECIFIED, find_port: FIND_PORT, copy_port: COPY_PORT }
    }
}

/// The phone side, for as long as the app runs: answer the PC's find packets and take its mail.
/// Returns only when a port cannot be opened (another process of the app has it already).
pub async fn serve(
    store: Arc<Store>,
    secrets: Arc<Secrets>,
    listen: Listen,
    on: Arc<dyn Fn(PhoneEvent) + Send + Sync>,
) -> Result<()> {
    let udp = UdpSocket::bind((listen.ip, listen.find_port)).await.map_err(|_| broken("udp bind"))?;
    let tcp = match TcpListener::bind((listen.ip, listen.copy_port)).await {
        Ok(l) => l,
        Err(_) => TcpListener::bind((listen.ip, 0)).await.map_err(|_| broken("tcp bind"))?,
    };
    let port = tcp.local_addr().map_err(|_| broken("tcp bind"))?.port();
    let mut buf = vec![0u8; 2048];
    loop {
        tokio::select! {
            got = udp.recv_from(&mut buf) => {
                let Ok((n, from)) = got else { continue };
                if let Some(answer) = answer_find(&store, &secrets, &buf[..n], from, port)? {
                    let _ = udp.send_to(&answer, from).await;
                }
            }
            got = tcp.accept() => {
                let Ok((stream, from)) = got else { continue };
                if !is_private(from.ip()) {
                    continue;
                }
                let Some(key) = contacts_sync::key(&secrets)? else { continue };
                match take_all(&store, &key, stream, on.as_ref()).await {
                    Ok(c) => on(PhoneEvent::Done { received: c.received, removed: c.removed }),
                    Err(error) => {
                        log::event("lan.failed", &[("code", &error.code)]);
                        // A broken connection is tried again with the next find; only a full
                        // phone needs the user.
                        if error.code() == Some(Code::LanSpace01) {
                            on(PhoneEvent::Failed { error });
                        } else {
                            on(PhoneEvent::Stopped);
                        }
                    }
                }
            }
        }
    }
}

fn answer_find(store: &Store, secrets: &Secrets, packet: &[u8], from: SocketAddr, port: u16) -> Result<Option<Vec<u8>>> {
    if !is_private(from.ip()) {
        return Ok(None);
    }
    let Some(key) = contacts_sync::key(secrets)? else { return Ok(None) };
    let Some(find) = wire::open_packet::<Find>(&key, packet) else { return Ok(None) };
    if find.v != 1 || (find.at - now()).abs() > MAX_CLOCK_SKEW {
        return Ok(None);
    }
    let have: Option<String> = store.kv_get(DIGEST_KEY)?;
    let here = Here { v: 1, nonce: find.nonce, port, wants: have.as_deref() != Some(find.digest.as_str()) };
    Ok(Some(wire::seal_packet(&key, &here)?))
}

fn is_disk_full(e: &VemError) -> bool {
    // StoreWrite01 carries the io error kind; a full disk is ErrorKind::StorageFull ("no storage
    // space") on Android and Windows alike.
    e.code() == Some(Code::StoreWrite01)
        && e.detail.as_deref().is_some_and(|d| d.contains(&std::io::ErrorKind::StorageFull.to_string()))
}

async fn take_all(store: &Store, key: &[u8; 32], stream: TcpStream, on: &(dyn Fn(PhoneEvent) + Send + Sync)) -> Result<Copied> {
    let mut ch = Channel::open(stream, key, false).await?;
    let list: Manifest = timeout(STEP_WAIT, ch.recv_json(KIND_LIST)).await.map_err(|_| broken("list timeout"))??;
    let applied = mirror::apply(store, &list)?;
    ch.send_json(KIND_WANTS, &applied.wants).await?;
    let total = applied.wants.len() as u32;
    let mut copied = Copied { removed: applied.removed, ..Copied::default() };
    if total > 0 {
        on(PhoneEvent::Progress { done: 0, total });
    }
    loop {
        let (kind, body) = timeout(STEP_WAIT, ch.recv()).await.map_err(|_| broken("mail timeout"))??;
        match kind {
            KIND_MAIL => {
                let (meta, raw): (OfferedMessage, &[u8]) = wire::split_mail_frame(&body)?;
                if let Err(e) = mirror::receive(store, &meta, raw) {
                    return Err(if is_disk_full(&e) { VemError::new(Code::LanSpace01) } else { e });
                }
                copied.received += 1;
                if copied.received.is_multiple_of(25) || copied.received == total {
                    on(PhoneEvent::Progress { done: copied.received, total });
                }
            }
            KIND_DONE => break,
            _ => return Err(broken("unexpected frame")),
        }
    }
    copied.skipped = total.saturating_sub(copied.received);
    if copied.skipped == 0 {
        store.kv_set(DIGEST_KEY, &mirror::digest(&list))?;
    }
    ch.send_json(KIND_RESULT, &copied).await?;
    Ok(copied)
}
