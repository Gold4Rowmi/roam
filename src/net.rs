//! Encrypted, replay-protected, LAN-only UDP link between the two machines.
use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};
use sha2::{Digest, Sha256};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PORT: u16 = 47321;
pub const HOST: u8 = 0;
pub const CLIENT: u8 = 1;
/// Bump when the wire format changes. Sent in nonce byte 1, so it is authenticated. Builds before 0.2 sent 0.
pub const PROTOCOL: u8 = 1;
/// shortcut: clipboard capped at one UDP datagram, chunk it if you need bigger pastes
pub const MAX_CLIP: usize = 60_000;

#[derive(Debug, PartialEq)]
pub enum Msg {
    Enter(u8, f32),    // host -> client: cursor crossed edge (0 right,1 left,2 top,3 bottom), position along it as 0..1
    Leave(f32),        // client -> host: cursor came back, position along the edge as 0..1
    Move(i16, i16),    // raw mouse delta
    Button(u16, i16),  // raw-input button flags + wheel data
    Key { vk: u8, scan: u8, up: bool, ext: bool },
    Clip(String),
    Ping,
    Bye,               // this side is stopping; don't wait for the silence timeout
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        match self {
            Msg::Enter(e, f) => { b.extend([0, *e]); b.extend(f.to_le_bytes()) }
            Msg::Leave(f) => { b.push(1); b.extend(f.to_le_bytes()) }
            Msg::Move(x, y) => { b.push(2); b.extend(x.to_le_bytes()); b.extend(y.to_le_bytes()) }
            Msg::Button(f, d) => { b.push(3); b.extend(f.to_le_bytes()); b.extend(d.to_le_bytes()) }
            Msg::Key { vk, scan, up, ext } => b.extend([4, *vk, *scan, *up as u8 | (*ext as u8) << 1]),
            Msg::Clip(s) => { b.push(5); b.extend(s.as_bytes()) }
            Msg::Ping => b.push(6),
            Msg::Bye => b.push(7),
        }
        b
    }

    pub fn decode(b: &[u8]) -> Option<Msg> {
        let (t, p) = b.split_first()?;
        let i16at = |i: usize| p.get(i..i + 2).map(|s| i16::from_le_bytes(s.try_into().unwrap()));
        Some(match t {
            0 => Msg::Enter(*p.first()?, f32::from_le_bytes(p.get(1..)?.try_into().ok()?)),
            1 => Msg::Leave(f32::from_le_bytes(p.try_into().ok()?)),
            2 => Msg::Move(i16at(0)?, i16at(2)?),
            3 => Msg::Button(i16at(0)? as u16, i16at(2)?),
            4 => { let [vk, scan, f] = *<&[u8; 3]>::try_from(p).ok()?; Msg::Key { vk, scan, up: f & 1 != 0, ext: f & 2 != 0 } }
            5 => Msg::Clip(String::from_utf8(p.to_vec()).ok()?),
            6 => Msg::Ping,
            7 => Msg::Bye,
            _ => return None,
        })
    }
}

pub struct Link {
    sock: UdpSocket,
    cipher: ChaCha20Poly1305,
    role: u8,
    seq: AtomicU64,
    last_seq: AtomicU64,
    peer: Mutex<Option<SocketAddr>>,
    mismatch: AtomicBool,
}

impl Link {
    pub fn new(secret: &str, sock: UdpSocket, role: u8, peer: Option<SocketAddr>) -> Link {
        sock.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
        // seq starts at wall-clock nanos so a restart never reuses a nonce
        let seq = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
        Link {
            sock,
            cipher: ChaCha20Poly1305::new(&Sha256::digest(secret.as_bytes())),
            role,
            seq: AtomicU64::new(seq),
            last_seq: AtomicU64::new(0),
            peer: Mutex::new(peer),
            mismatch: AtomicBool::new(false),
        }
    }

    /// True if a packet with the right pairing code but another protocol version arrived since the last call.
    pub fn mismatch(&self) -> bool {
        self.mismatch.swap(false, Relaxed)
    }

    pub fn set_peer(&self, p: SocketAddr) {
        *self.peer.lock().unwrap() = Some(p);
    }

    pub fn send(&self, m: &Msg) {
        let Some(peer) = *self.peer.lock().unwrap() else { return };
        let mut nonce = [0u8; 12];
        nonce[0] = self.role;
        nonce[1] = PROTOCOL;
        nonce[4..].copy_from_slice(&self.seq.fetch_add(1, Relaxed).to_le_bytes());
        let ct = self.cipher.encrypt(Nonce::from_slice(&nonce), m.encode().as_slice()).unwrap();
        let _ = self.sock.send_to(&[&nonce[..], &ct].concat(), peer);
    }

    /// Returns None on timeout (500ms) or on any packet that is not from the LAN,
    /// not from the other role, replayed/out of order, or fails authentication.
    pub fn recv(&self) -> Option<Msg> {
        let mut buf = [0u8; 65535];
        let (n, from) = self.sock.recv_from(&mut buf).ok()?;
        let SocketAddr::V4(a) = from else { return None };
        let ip = a.ip();
        if !(ip.is_private() || ip.is_link_local() || ip.is_loopback()) {
            return None;
        }
        let (nonce, ct) = buf[..n].split_at_checked(12)?;
        if nonce[0] == self.role {
            return None;
        }
        let seq = u64::from_le_bytes(nonce[4..].try_into().unwrap());
        if seq <= self.last_seq.load(Relaxed) {
            return None;
        }
        let pt = self.cipher.decrypt(Nonce::from_slice(nonce), ct).ok()?;
        self.last_seq.store(seq, Relaxed);
        *self.peer.lock().unwrap() = Some(from);
        if nonce[1] != PROTOCOL {
            self.mismatch.store(true, Relaxed);
            return None;
        }
        Msg::decode(&pt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_messages() {
        let msgs = [
            Msg::Enter(3, 0.25), Msg::Leave(0.9), Msg::Move(-3, 7), Msg::Button(0x401, -120),
            Msg::Key { vk: 65, scan: 30, up: true, ext: false }, Msg::Clip("héllo".into()), Msg::Ping, Msg::Bye,
        ];
        for m in msgs {
            assert_eq!(Msg::decode(&m.encode()).unwrap(), m);
        }
        assert!(Msg::decode(&[8]).is_none());
        assert!(Msg::decode(&[2, 1]).is_none());
    }

    #[test]
    fn link_encrypts_and_rejects_replay_and_wrong_key() {
        let a = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b_addr = b.local_addr().unwrap();
        let host = Link::new("s3cret", a, HOST, Some(b_addr));
        let client = Link::new("s3cret", b, CLIENT, None);

        host.send(&Msg::Move(1, 2));
        assert_eq!(client.recv(), Some(Msg::Move(1, 2)));
        client.send(&Msg::Ping); // client learned host addr from the first packet
        assert_eq!(host.recv(), Some(Msg::Ping));

        // replay: resend an old captured datagram
        let raw = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&(host.last_seq.load(Relaxed) - 1).to_le_bytes()); // older seq
        nonce[0] = CLIENT;
        let ct = client.cipher.encrypt(Nonce::from_slice(&nonce), &Msg::Ping.encode()[..]).unwrap();
        raw.send_to(&[&nonce[..], &ct].concat(), host.sock.local_addr().unwrap()).unwrap();
        assert_eq!(host.recv(), None);

        // wrong secret
        let bad = Link::new("wrong", UdpSocket::bind("127.0.0.1:0").unwrap(), CLIENT, Some(host.sock.local_addr().unwrap()));
        bad.send(&Msg::Enter(0, 0.5));
        assert_eq!(host.recv(), None);
        assert!(!host.mismatch()); // a wrong code proves nothing about the version

        // right code, other protocol: dropped, and reported once
        let mut nonce = [0u8; 12];
        nonce[0] = CLIENT;
        nonce[4..].copy_from_slice(&u64::MAX.to_le_bytes());
        let ct = client.cipher.encrypt(Nonce::from_slice(&nonce), &Msg::Ping.encode()[..]).unwrap();
        raw.send_to(&[&nonce[..], &ct].concat(), host.sock.local_addr().unwrap()).unwrap();
        assert_eq!(host.recv(), None);
        assert!(host.mismatch());
        assert!(!host.mismatch());
    }
}
