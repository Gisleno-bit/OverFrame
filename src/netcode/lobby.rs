//! LAN room discovery.
//!
//! A hosting game broadcasts an [`Announce`] once a second to
//! `255.255.255.255:DISCOVERY_PORT` from its own game socket; a browsing game
//! binds the discovery port and collects what it hears into a room list with
//! the sender's address. Rooms that stop announcing drop off after a few
//! seconds. That is the whole "server browser" for LAN play — the Steam build
//! replaces this module with Steam's lobby list behind the same shape.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use super::handshake::{Announce, Handshake, DISCOVERY_PORT, PROTOCOL_VERSION};
use super::socket::SharedSocket;

/// A room seen on the LAN.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    pub name: String,
    /// Where to send `Hello`.
    pub addr: SocketAddr,
    pub players: u8,
    pub max_players: u8,
    pub locked: bool,
    pub version_ok: bool,
}

const ROOM_TTL: Duration = Duration::from_secs(4);
const ANNOUNCE_EVERY: Duration = Duration::from_millis(1000);

/// Host side: periodically shouts the room on the LAN.
pub struct Announcer {
    socket: SharedSocket,
    last: Instant,
    pub name: String,
    pub locked: bool,
    pub players: u8,
}

impl Announcer {
    pub fn new(socket: SharedSocket, name: String, locked: bool) -> Announcer {
        let _ = socket.set_broadcast(true);
        Announcer {
            socket,
            last: Instant::now() - ANNOUNCE_EVERY,
            name,
            locked,
            players: 1,
        }
    }

    /// Send an announcement if it's time. Cheap to call every frame.
    pub fn tick(&mut self) {
        if self.last.elapsed() < ANNOUNCE_EVERY {
            return;
        }
        self.last = Instant::now();
        let port = self.socket.local_addr().map(|a| a.port()).unwrap_or(0);
        let msg = Handshake::Announce(Announce {
            version: PROTOCOL_VERSION,
            name: self.name.clone(),
            port,
            players: self.players,
            max_players: 2,
            locked: self.locked,
        });
        let to = SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT));
        let _ = self.socket.send_handshake(&msg, to);
        // Also loopback, so two instances on one machine find each other even
        // when the OS doesn't loop broadcasts back.
        let lo = SocketAddr::from((Ipv4Addr::LOCALHOST, DISCOVERY_PORT));
        let _ = self.socket.send_handshake(&msg, lo);
    }
}

/// Client side: listens for announcements and keeps a fresh room list.
pub struct Browser {
    socket: UdpSocket,
    seen: Vec<(Room, Instant)>,
    buf: [u8; 512],
}

impl Browser {
    /// Bind the discovery port. Fails if another browser on this machine holds
    /// it (only one instance can browse at a time; hosting is unaffected).
    pub fn new() -> std::io::Result<Browser> {
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT)))?;
        socket.set_nonblocking(true)?;
        socket.set_broadcast(true)?;
        Ok(Browser {
            socket,
            seen: Vec::new(),
            buf: [0; 512],
        })
    }

    /// Drain announcements and expire stale rooms. Call every frame.
    pub fn poll(&mut self) {
        while let Ok((n, from)) = self.socket.recv_from(&mut self.buf) {
            if let Some(Handshake::Announce(a)) = Handshake::decode(&self.buf[..n]) {
                let addr = SocketAddr::new(from.ip(), a.port);
                let room = Room {
                    name: a.name,
                    addr,
                    players: a.players,
                    max_players: a.max_players,
                    locked: a.locked,
                    version_ok: a.version == PROTOCOL_VERSION,
                };
                match self.seen.iter_mut().find(|(r, _)| r.addr == addr) {
                    Some(slot) => *slot = (room, Instant::now()),
                    None => self.seen.push((room, Instant::now())),
                }
            }
        }
        self.seen.retain(|(_, t)| t.elapsed() < ROOM_TTL);
    }

    /// Current rooms, most recently heard first.
    pub fn rooms(&self) -> Vec<Room> {
        let mut v: Vec<(Room, Instant)> = self.seen.clone();
        v.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
        v.into_iter().map(|(r, _)| r).collect()
    }
}
