//! Authenticated datagram routing for VERA clients. This is an outer transport:
//! payloads remain the existing game datagrams, with no simulation authority.
//!
//! The HTTPS lobby signs a 74-byte ticket and separately returns its derived
//! packet key. A captured UDP ticket does not reveal that key. Domain-separated
//! HMAC-SHA256 uses RustCrypto's constant-time verification:
//! https://docs.rs/hmac/0.12.1/hmac/ . Neither key type prints secret bytes.

use std::collections::HashMap;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const MAX_PEERS: u8 = 30;
pub const BROADCAST: u8 = 255;
pub const MAX_PAYLOAD: usize = 640;
pub const TICKET_LEN: usize = 74;
pub const MAX_DATAGRAM: usize = 122 + MAX_PAYLOAD;
pub const MAX_TICKET_LIFETIME: u64 = 7200;
const TICKET_BODY_LEN: usize = 42;
const CLIENT_HEADER: usize = 90;
const SERVER_HEADER: usize = 48;
const MAC_LEN: usize = 32;
const COOKIE_LEN: usize = 40;
const CLIENT_MAGIC: &[u8; 4] = b"VR1C";
const SERVER_MAGIC: &[u8; 4] = b"VR1S";
const TICKET_DOMAIN: &[u8] = b"VERA20k relay ticket v1\0";
const KEY_DOMAIN: &[u8] = b"VERA20k relay packet key v1\0";
const CLIENT_DOMAIN: &[u8] = b"VERA20k relay client v1\0";
const SERVER_DOMAIN: &[u8] = b"VERA20k relay server v1\0";
const COOKIE_DOMAIN: &[u8] = b"VERA20k relay source v1\0";
const HELLO: u8 = 0;
const CONFIRM: u8 = 1;
const DATA: u8 = 2;
const PING: u8 = 3;
const CHALLENGE: u8 = 0;
const READY: u8 = 1;

/// Server signing material or a single client's packet key. HTTPS credentials
/// have their own owner; this value is only held in process memory.
#[derive(Clone)]
pub struct SecretKey([u8; 32]);

impl SecretKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn mac(&self, domain: &[u8], bytes: &[u8]) -> HmacSha256 {
        let mut mac =
            HmacSha256::new_from_slice(&self.0).expect("SHA-256 HMAC accepts any key size");
        mac.update(domain);
        mac.update(bytes);
        mac
    }

    /// For an HTTPS response to the admitted client, never logs or UDP.
    pub fn expose_for_https(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey([redacted])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket {
    pub room: [u8; 16],
    pub source: u8,
    pub expires: u64,
    pub nonce: [u8; 16],
}

impl Ticket {
    pub fn parse(bytes: &[u8]) -> Result<Self, RelayError> {
        if bytes.len() != TICKET_LEN || bytes[0] != 1 || bytes[17] >= MAX_PEERS {
            return Err(RelayError::Malformed);
        }
        Ok(Self {
            room: bytes[1..17].try_into().expect("checked ticket length"),
            source: bytes[17],
            expires: u64::from_le_bytes(bytes[18..26].try_into().expect("checked ticket length")),
            nonce: bytes[26..42].try_into().expect("checked ticket length"),
        })
    }

    /// Cloudflare and native test clients use this same fixed byte layout.
    /// The random nonce and room identifier must come from OS cryptography.
    pub fn sign(self, server_key: &SecretKey) -> Result<[u8; TICKET_LEN], RelayError> {
        if self.source >= MAX_PEERS {
            return Err(RelayError::Malformed);
        }
        let mut bytes = [0; TICKET_LEN];
        bytes[0] = 1;
        bytes[1..17].copy_from_slice(&self.room);
        bytes[17] = self.source;
        bytes[18..26].copy_from_slice(&self.expires.to_le_bytes());
        bytes[26..42].copy_from_slice(&self.nonce);
        let signature = server_key
            .mac(TICKET_DOMAIN, &bytes[..TICKET_BODY_LEN])
            .finalize()
            .into_bytes();
        bytes[TICKET_BODY_LEN..].copy_from_slice(&signature);
        Ok(bytes)
    }
}

pub fn derive_packet_key(server_key: &SecretKey, ticket: &[u8; TICKET_LEN]) -> SecretKey {
    SecretKey(
        server_key
            .mac(KEY_DOMAIN, ticket)
            .finalize()
            .into_bytes()
            .into(),
    )
}

/// Immutable endpoint labels let existing reliable/global owners keep their
/// endpoint identity rules while the physical UDP destination is the relay.
pub fn peer_endpoint(source: u8) -> Result<SocketAddrV4, RelayError> {
    if source >= MAX_PEERS {
        return Err(RelayError::Malformed);
    }
    Ok(SocketAddrV4::new(
        Ipv4Addr::new(127, 77, 0, source + 1),
        1234,
    ))
}

pub fn endpoint_peer(endpoint: SocketAddrV4) -> Option<u8> {
    let [a, b, c, d] = endpoint.ip().octets();
    (a == 127 && b == 77 && c == 0 && d > 0 && d <= MAX_PEERS).then_some(d.wrapping_sub(1))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RelayError {
    #[error("malformed relay datagram")]
    Malformed,
    #[error("relay authentication failed")]
    Authentication,
    #[error("relay ticket expired or exceeds its allowed lifetime")]
    Expired,
    #[error("replayed relay datagram")]
    Replay,
    #[error("relay address must complete its challenge")]
    Unregistered,
    #[error("relay session capacity reached")]
    Full,
    #[error("relay packet rate exceeded")]
    RateLimited,
    #[error("relay counter exhausted")]
    CounterExhausted,
}

/// A small sliding window accepts reordered packets once. State changes only
/// after the authentication tag and fixed framing have been checked.
#[derive(Debug, Default)]
struct ReplayWindow {
    maximum: u64,
    seen: u128,
}

impl ReplayWindow {
    fn accept(&mut self, sequence: u64) -> Result<(), RelayError> {
        if sequence == 0 {
            return Err(RelayError::Replay);
        }
        if sequence > self.maximum {
            let advance = sequence - self.maximum;
            self.seen = if advance >= 128 {
                1
            } else {
                (self.seen << advance) | 1
            };
            self.maximum = sequence;
            return Ok(());
        }
        let behind = self.maximum - sequence;
        if behind >= 128 || self.seen & (1u128 << behind) != 0 {
            return Err(RelayError::Replay);
        }
        self.seen |= 1u128 << behind;
        Ok(())
    }
}

pub struct ClientSession {
    ticket_bytes: [u8; TICKET_LEN],
    ticket: Ticket,
    key: SecretKey,
    sent: u64,
    received: ReplayWindow,
    probe: Option<u64>,
    ready: bool,
}

impl fmt::Debug for ClientSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientSession")
            .field("source", &self.ticket.source)
            .field("ready", &self.ready)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum ServerEvent<'a> {
    Challenge(&'a [u8]),
    Ready,
    Payload { source: u8, bytes: &'a [u8] },
}

impl ClientSession {
    pub fn new(ticket: &[u8], packet_key: [u8; 32]) -> Result<Self, RelayError> {
        Ok(Self {
            ticket: Ticket::parse(ticket)?,
            ticket_bytes: ticket.try_into().map_err(|_| RelayError::Malformed)?,
            key: SecretKey::new(packet_key),
            sent: 0,
            received: ReplayWindow::default(),
            probe: None,
            ready: false,
        })
    }

    pub fn source(&self) -> u8 {
        self.ticket.source
    }
    pub fn is_ready(&self) -> bool {
        self.ready
    }
    pub fn expires(&self) -> u64 {
        self.ticket.expires
    }
    pub fn same_lease(&self, ticket: &[u8]) -> bool {
        self.ticket_bytes.as_slice() == ticket
    }

    /// Prepare a fresh key without discarding the admitted route. Identity
    /// cannot change during a match, and delayed grants cannot roll it back.
    pub fn renewal(&self, ticket: &[u8], packet_key: [u8; 32]) -> Result<Self, RelayError> {
        let next = Self::new(ticket, packet_key)?;
        if next.ticket.room != self.ticket.room
            || next.ticket.source != self.ticket.source
            || next.ticket.nonce == self.ticket.nonce
            || next.ticket.expires <= self.ticket.expires
        {
            return Err(RelayError::Authentication);
        }
        Ok(next)
    }
    pub fn forget_readiness(&mut self) {
        self.ready = false;
    }

    pub fn probe(&mut self, out: &mut Vec<u8>) -> Result<(), RelayError> {
        self.encode(if self.ready { PING } else { HELLO }, BROADCAST, &[], out)?;
        self.probe = Some(self.sent);
        Ok(())
    }

    pub fn confirm(&mut self, cookie: &[u8], out: &mut Vec<u8>) -> Result<(), RelayError> {
        if cookie.len() != COOKIE_LEN {
            return Err(RelayError::Malformed);
        }
        self.encode(CONFIRM, BROADCAST, cookie, out)
    }

    pub fn data(
        &mut self,
        target: u8,
        payload: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), RelayError> {
        if !self.ready {
            return Err(RelayError::Unregistered);
        }
        self.encode(DATA, target, payload, out)
    }

    fn encode(
        &mut self,
        kind: u8,
        target: u8,
        payload: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), RelayError> {
        if payload.len() > MAX_PAYLOAD || (target >= MAX_PEERS && target != BROADCAST) {
            return Err(RelayError::Malformed);
        }
        self.sent = self
            .sent
            .checked_add(1)
            .ok_or(RelayError::CounterExhausted)?;
        out.clear();
        out.extend_from_slice(CLIENT_MAGIC);
        out.push(kind);
        out.extend_from_slice(&self.ticket_bytes);
        out.extend_from_slice(&self.sent.to_le_bytes());
        out.push(target);
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        out.extend_from_slice(payload);
        let tag = self.key.mac(CLIENT_DOMAIN, out).finalize().into_bytes();
        out.extend_from_slice(&tag);
        Ok(())
    }

    pub fn receive<'a>(&mut self, bytes: &'a [u8]) -> Result<ServerEvent<'a>, RelayError> {
        if bytes.len() < SERVER_HEADER + MAC_LEN || &bytes[..4] != SERVER_MAGIC {
            return Err(RelayError::Malformed);
        }
        let length = u16::from_le_bytes([bytes[46], bytes[47]]) as usize;
        if length > MAX_PAYLOAD || bytes.len() != SERVER_HEADER + length + MAC_LEN {
            return Err(RelayError::Malformed);
        }
        self.key
            .mac(SERVER_DOMAIN, &bytes[..bytes.len() - MAC_LEN])
            .verify_slice(&bytes[bytes.len() - MAC_LEN..])
            .map_err(|_| RelayError::Authentication)?;
        if bytes[5..21] != self.ticket.room
            || bytes[21..37] != self.ticket.nonce
            || bytes[45] >= MAX_PEERS
        {
            return Err(RelayError::Authentication);
        }
        let sequence = u64::from_le_bytes(bytes[37..45].try_into().expect("checked server header"));
        let payload = &bytes[SERVER_HEADER..SERVER_HEADER + length];
        match bytes[4] {
            CHALLENGE if length == COOKIE_LEN && self.probe == Some(sequence) => {
                self.probe = None;
                self.ready = false;
                Ok(ServerEvent::Challenge(payload))
            }
            READY if payload.is_empty() && bytes[45] == self.ticket.source => {
                self.received.accept(sequence)?;
                self.ready = true;
                Ok(ServerEvent::Ready)
            }
            DATA if !payload.is_empty() && self.ready => {
                self.received.accept(sequence)?;
                Ok(ServerEvent::Payload {
                    source: bytes[45],
                    bytes: payload,
                })
            }
            _ => Err(RelayError::Malformed),
        }
    }
}

struct Incoming<'a> {
    ticket: Ticket,
    ticket_bytes: &'a [u8; TICKET_LEN],
    key: SecretKey,
    kind: u8,
    sequence: u64,
    target: u8,
    payload: &'a [u8],
}

fn incoming<'a>(key: &SecretKey, bytes: &'a [u8], now: u64) -> Result<Incoming<'a>, RelayError> {
    if bytes.len() < CLIENT_HEADER + MAC_LEN || &bytes[..4] != CLIENT_MAGIC {
        return Err(RelayError::Malformed);
    }
    let length = u16::from_le_bytes([bytes[88], bytes[89]]) as usize;
    if length > MAX_PAYLOAD || bytes.len() != CLIENT_HEADER + length + MAC_LEN {
        return Err(RelayError::Malformed);
    }
    let ticket_bytes: &[u8; TICKET_LEN] = bytes[5..79].try_into().expect("checked client header");
    let ticket = Ticket::parse(ticket_bytes)?;
    key.mac(TICKET_DOMAIN, &ticket_bytes[..TICKET_BODY_LEN])
        .verify_slice(&ticket_bytes[TICKET_BODY_LEN..])
        .map_err(|_| RelayError::Authentication)?;
    if ticket.expires <= now || ticket.expires > now.saturating_add(MAX_TICKET_LIFETIME) {
        return Err(RelayError::Expired);
    }
    let packet_key = derive_packet_key(key, ticket_bytes);
    packet_key
        .mac(CLIENT_DOMAIN, &bytes[..bytes.len() - MAC_LEN])
        .verify_slice(&bytes[bytes.len() - MAC_LEN..])
        .map_err(|_| RelayError::Authentication)?;
    let target = bytes[87];
    let kind = bytes[4];
    if (target >= MAX_PEERS && target != BROADCAST)
        || !matches!(kind, HELLO | CONFIRM | DATA | PING)
        || (kind == CONFIRM && length != COOKIE_LEN)
        || (kind == DATA && length == 0)
        || (matches!(kind, HELLO | PING) && length != 0)
    {
        return Err(RelayError::Malformed);
    }
    let sequence = u64::from_le_bytes(bytes[79..87].try_into().expect("checked client header"));
    if sequence == 0 {
        return Err(RelayError::Replay);
    }
    Ok(Incoming {
        ticket,
        ticket_bytes,
        key: packet_key,
        kind,
        sequence,
        target,
        payload: &bytes[CLIENT_HEADER..CLIENT_HEADER + length],
    })
}

struct Peer {
    ticket: Ticket,
    key: SecretKey,
    endpoint: SocketAddrV4,
    received: ReplayWindow,
    sent: u64,
    last_seen: u64,
    rate_second: u64,
    rate_count: u32,
}

pub struct ForwardDatagram {
    pub endpoint: SocketAddrV4,
    pub bytes: Vec<u8>,
}

/// Bound storage and packet rate; untrusted data never creates a room until
/// both ticket authentication and UDP source reachability are established.
pub struct RelayServer {
    key: SecretKey,
    rooms: HashMap<[u8; 16], Vec<Peer>>,
    max_rooms: usize,
    last_pruned: u64,
}

impl RelayServer {
    pub fn new(key: SecretKey, max_rooms: usize) -> Self {
        Self {
            key,
            rooms: HashMap::new(),
            max_rooms,
            last_pruned: 0,
        }
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    pub fn process(
        &mut self,
        source: SocketAddrV4,
        bytes: &[u8],
        now: u64,
    ) -> Result<Vec<ForwardDatagram>, RelayError> {
        let message = incoming(&self.key, bytes, now)?;
        if self.last_pruned != now {
            self.rooms.retain(|_, peers| {
                // Retain counters until the grant expires. An idle NAT route
                // can be challenged again without reopening captured packets.
                peers.retain(|peer| peer.ticket.expires > now);
                !peers.is_empty()
            });
            self.last_pruned = now;
        }
        if message.kind == HELLO {
            return Ok(vec![self.challenge(source, &message, now)]);
        }
        if message.kind == CONFIRM {
            self.verify_cookie(source, &message, now)?;
            if !self.rooms.contains_key(&message.ticket.room) && self.rooms.len() >= self.max_rooms
            {
                return Err(RelayError::Full);
            }
            let peers = self.rooms.entry(message.ticket.room).or_default();
            if let Some(peer) = peers
                .iter_mut()
                .find(|peer| peer.ticket.source == message.ticket.source)
            {
                if peer.ticket != message.ticket {
                    // Only a newer server-issued grant may replace an active
                    // key. Delayed old grants cannot roll identity backwards.
                    if message.ticket.expires <= peer.ticket.expires {
                        return Err(RelayError::Authentication);
                    }
                    *peer = new_peer(&message, source, now);
                }
                if message.sequence <= peer.received.maximum && peer.endpoint != source {
                    return Err(RelayError::Replay);
                }
                peer.received.accept(message.sequence)?;
                peer.endpoint = source;
                peer.last_seen = now;
                return Ok(vec![server_packet(
                    peer,
                    READY,
                    message.ticket.source,
                    &[],
                )?]);
            }
            let mut peer = new_peer(&message, source, now);
            peer.received.accept(message.sequence)?;
            let reply = server_packet(&mut peer, READY, message.ticket.source, &[])?;
            peers.push(peer);
            return Ok(vec![reply]);
        }
        let Some(peers) = self.rooms.get_mut(&message.ticket.room) else {
            return Err(RelayError::Unregistered);
        };
        let Some(index) = peers.iter().position(|peer| peer.ticket == message.ticket) else {
            return Err(RelayError::Unregistered);
        };
        if peers[index].endpoint != source {
            return if message.kind == PING {
                Ok(vec![self.challenge(source, &message, now)])
            } else {
                Err(RelayError::Unregistered)
            };
        }
        let peer = &mut peers[index];
        peer.received.accept(message.sequence)?;
        if peer.rate_second != now {
            peer.rate_second = now;
            peer.rate_count = 0;
        }
        peer.rate_count += 1;
        if peer.rate_count > 2048 {
            return Err(RelayError::RateLimited);
        }
        peer.last_seen = now;
        if message.kind == PING {
            return Ok(vec![server_packet(
                peer,
                READY,
                message.ticket.source,
                &[],
            )?]);
        }
        let mut output = Vec::new();
        for peer in peers {
            if peer.ticket.source != message.ticket.source
                && now.saturating_sub(peer.last_seen) <= 30
                && (message.target == BROADCAST || message.target == peer.ticket.source)
            {
                output.push(server_packet(
                    peer,
                    DATA,
                    message.ticket.source,
                    message.payload,
                )?);
            }
        }
        Ok(output)
    }

    fn challenge(&self, source: SocketAddrV4, message: &Incoming<'_>, now: u64) -> ForwardDatagram {
        let expires = now.saturating_add(10);
        let mut cookie = expires.to_le_bytes().to_vec();
        let tag = self
            .cookie_mac(source, message.ticket_bytes, expires)
            .finalize()
            .into_bytes();
        cookie.extend_from_slice(&tag);
        ForwardDatagram {
            endpoint: source,
            bytes: encode_server(
                &message.ticket,
                &message.key,
                CHALLENGE,
                message.sequence,
                message.ticket.source,
                &cookie,
            ),
        }
    }

    fn cookie_mac(
        &self,
        source: SocketAddrV4,
        ticket: &[u8; TICKET_LEN],
        expires: u64,
    ) -> HmacSha256 {
        let mut mac = self.key.mac(COOKIE_DOMAIN, ticket);
        mac.update(&source.ip().octets());
        mac.update(&source.port().to_le_bytes());
        mac.update(&expires.to_le_bytes());
        mac
    }

    fn verify_cookie(
        &self,
        source: SocketAddrV4,
        message: &Incoming<'_>,
        now: u64,
    ) -> Result<(), RelayError> {
        let expires = u64::from_le_bytes(
            message.payload[..8]
                .try_into()
                .expect("checked confirm payload"),
        );
        if expires < now || expires > now.saturating_add(10) {
            return Err(RelayError::Expired);
        }
        self.cookie_mac(source, message.ticket_bytes, expires)
            .verify_slice(&message.payload[8..])
            .map_err(|_| RelayError::Authentication)
    }
}

fn new_peer(message: &Incoming<'_>, endpoint: SocketAddrV4, now: u64) -> Peer {
    Peer {
        ticket: message.ticket,
        key: message.key.clone(),
        endpoint,
        received: ReplayWindow::default(),
        sent: 0,
        last_seen: now,
        rate_second: now,
        rate_count: 0,
    }
}

fn server_packet(
    peer: &mut Peer,
    kind: u8,
    source: u8,
    payload: &[u8],
) -> Result<ForwardDatagram, RelayError> {
    peer.sent = peer
        .sent
        .checked_add(1)
        .ok_or(RelayError::CounterExhausted)?;
    Ok(ForwardDatagram {
        endpoint: peer.endpoint,
        bytes: encode_server(&peer.ticket, &peer.key, kind, peer.sent, source, payload),
    })
}

fn encode_server(
    ticket: &Ticket,
    key: &SecretKey,
    kind: u8,
    sequence: u64,
    source: u8,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(SERVER_HEADER + payload.len() + MAC_LEN);
    bytes.extend_from_slice(SERVER_MAGIC);
    bytes.push(kind);
    bytes.extend_from_slice(&ticket.room);
    bytes.extend_from_slice(&ticket.nonce);
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes.push(source);
    bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    bytes.extend_from_slice(payload);
    let tag = key.mac(SERVER_DOMAIN, &bytes).finalize().into_bytes();
    bytes.extend_from_slice(&tag);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(key: &SecretKey, room: u8, source: u8) -> ClientSession {
        let ticket = Ticket {
            room: [room; 16],
            source,
            expires: 2000,
            nonce: [source + 1; 16],
        }
        .sign(key)
        .unwrap();
        let packet_key = derive_packet_key(key, &ticket);
        ClientSession::new(&ticket, *packet_key.expose_for_https()).unwrap()
    }

    fn connect(server: &mut RelayServer, client: &mut ClientSession, endpoint: SocketAddrV4) {
        let mut bytes = Vec::new();
        client.probe(&mut bytes).unwrap();
        let challenge = server.process(endpoint, &bytes, 1000).unwrap().remove(0);
        let ServerEvent::Challenge(cookie) = client.receive(&challenge.bytes).unwrap() else {
            panic!("challenge required");
        };
        client.confirm(cookie, &mut bytes).unwrap();
        let ready = server.process(endpoint, &bytes, 1000).unwrap().remove(0);
        assert!(matches!(
            client.receive(&ready.bytes),
            Ok(ServerEvent::Ready)
        ));
        assert!(client.is_ready());
    }

    #[test]
    fn authenticated_round_trip_preserves_payload_and_rejects_replay_forgery_and_other_room() {
        let key = SecretKey::new([73; 32]);
        let mut server = RelayServer::new(key.clone(), 2);
        let mut first = client(&key, 1, 0);
        let mut second = client(&key, 1, 29);
        let mut outsider = client(&key, 2, 1);
        let a = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 30001);
        let b = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 30002);
        connect(&mut server, &mut first, a);
        connect(&mut server, &mut second, b);
        connect(
            &mut server,
            &mut outsider,
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 30003),
        );
        let mut bytes = Vec::new();
        first
            .data(BROADCAST, b"unchanged game datagram", &mut bytes)
            .unwrap();
        let forwarded = server.process(a, &bytes, 1000).unwrap();
        assert_eq!(
            forwarded.len(),
            1,
            "broadcast is scoped to the authenticated match"
        );
        assert_eq!(forwarded[0].endpoint, b);
        assert!(matches!(
            second.receive(&forwarded[0].bytes),
            Ok(ServerEvent::Payload {
                source: 0,
                bytes: b"unchanged game datagram"
            })
        ));
        assert!(matches!(
            second.receive(&forwarded[0].bytes),
            Err(RelayError::Replay)
        ));
        assert!(matches!(
            server.process(a, &bytes, 1000),
            Err(RelayError::Replay)
        ));
        bytes[CLIENT_HEADER] ^= 1;
        assert!(matches!(
            server.process(a, &bytes, 1000),
            Err(RelayError::Authentication)
        ));
        assert!(matches!(
            server.process(a, &bytes, 2000),
            Err(RelayError::Expired)
        ));
    }

    #[test]
    fn source_rebinding_requires_return_challenge_and_reordered_packets_cannot_move_it_back() {
        let key = SecretKey::new([91; 32]);
        let mut server = RelayServer::new(key.clone(), 1);
        let mut first = client(&key, 1, 0);
        let old = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 31001);
        let new = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 31002);
        connect(&mut server, &mut first, old);
        let mut bytes = Vec::new();
        first.probe(&mut bytes).unwrap();
        let challenge = server.process(new, &bytes, 1000).unwrap().remove(0);
        let ServerEvent::Challenge(cookie) = first.receive(&challenge.bytes).unwrap() else {
            panic!("challenge required");
        };
        first.confirm(cookie, &mut bytes).unwrap();
        assert!(matches!(
            server.process(old, &bytes, 1000),
            Err(RelayError::Authentication)
        ));
        let old_confirm = bytes.clone();
        let ready = server.process(new, &bytes, 1000).unwrap().remove(0);
        assert!(matches!(
            first.receive(&ready.bytes),
            Ok(ServerEvent::Ready)
        ));
        assert!(matches!(
            server.process(old, &old_confirm, 1000),
            Err(RelayError::Authentication)
        ));
        first.data(29, b"peer not registered", &mut bytes).unwrap();
        assert!(matches!(
            server.process(old, &bytes, 1000),
            Err(RelayError::Unregistered)
        ));
        assert!(server.process(new, &bytes, 1000).unwrap().is_empty());
        assert!(!format!("{first:?} {key:?}").contains("91, 91"));
    }
}
