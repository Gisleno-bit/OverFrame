//! A non-blocking UDP socket that carries both the handshake and GGRS traffic.
//!
//! GGRS's own `UdpNonBlockingSocket` owns its socket, which would force the
//! handshake onto a second port. This implementation speaks the same wire format
//! (bincode-serialised [`ggrs::Message`]s) so it is drop-in compatible, but it
//! also recognises [`Handshake`] datagrams by their magic prefix and hands them
//! to the session layer. One port, one NAT mapping, no surprises.

use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};

use ggrs::{Message, NonBlockingSocket};

use super::handshake::Handshake;

const RECV_BUFFER_SIZE: usize = 4096;

/// The shared UDP socket.
#[derive(Debug)]
pub struct OfSocket {
    socket: UdpSocket,
    buffer: [u8; RECV_BUFFER_SIZE],
    /// GGRS messages that arrived while we were only polling for handshakes.
    pending: Vec<(SocketAddr, Message)>,
    /// Handshake datagrams that arrived while GGRS was draining messages.
    pending_hs: Vec<(SocketAddr, Handshake)>,
}

impl OfSocket {
    /// Bind to `0.0.0.0:port` (all interfaces) in non-blocking mode.
    pub fn bind(port: u16) -> std::io::Result<Self> {
        Self::bind_addr(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)))
    }

    /// Bind to an ephemeral port (for guests).
    pub fn bind_any() -> std::io::Result<Self> {
        Self::bind(0)
    }

    /// Bind to a specific address (tests use loopback).
    pub fn bind_addr(addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(OfSocket {
            socket,
            buffer: [0; RECV_BUFFER_SIZE],
            pending: Vec::new(),
            pending_hs: Vec::new(),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Allow sending to the LAN broadcast address (room announcements).
    pub fn set_broadcast(&self, on: bool) -> std::io::Result<()> {
        self.socket.set_broadcast(on)
    }

    /// Send a handshake datagram.
    pub fn send_handshake(&self, hs: &Handshake, to: SocketAddr) -> std::io::Result<()> {
        self.socket.send_to(&hs.encode(), to).map(|_| ())
    }

    /// Drain the socket, sorting datagrams into handshakes (returned) and GGRS
    /// messages (queued for the next [`NonBlockingSocket::receive_all_messages`]).
    pub fn poll_handshakes(&mut self) -> Vec<(SocketAddr, Handshake)> {
        self.drain();
        std::mem::take(&mut self.pending_hs)
    }

    fn drain(&mut self) {
        loop {
            match self.socket.recv_from(&mut self.buffer) {
                Ok((n, from)) => {
                    let bytes = &self.buffer[..n];
                    if Handshake::is_handshake(bytes) {
                        if let Some(hs) = Handshake::decode(bytes) {
                            self.pending_hs.push((from, hs));
                        }
                    } else if let Ok(msg) = bincode::deserialize::<Message>(bytes) {
                        self.pending.push((from, msg));
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => return,
                // Windows reports ICMP "port unreachable" as ConnectionReset on
                // the next recv — harmless for UDP, keep draining.
                Err(ref e) if e.kind() == ErrorKind::ConnectionReset => continue,
                Err(_) => return,
            }
        }
    }
}

/// A clonable handle to one [`OfSocket`]. GGRS takes ownership of the socket it
/// is given; sharing it behind a mutex lets the lobby layer keep sending
/// handshake/lobby datagrams (e.g. resending `Start`) after the session exists.
#[derive(Clone, Debug)]
pub struct SharedSocket(pub Arc<Mutex<OfSocket>>);

impl SharedSocket {
    pub fn new(s: OfSocket) -> Self {
        SharedSocket(Arc::new(Mutex::new(s)))
    }
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.0.lock().expect("socket mutex").local_addr()
    }
    pub fn send_handshake(&self, hs: &Handshake, to: SocketAddr) -> std::io::Result<()> {
        self.0.lock().expect("socket mutex").send_handshake(hs, to)
    }
    pub fn poll_handshakes(&self) -> Vec<(SocketAddr, Handshake)> {
        self.0.lock().expect("socket mutex").poll_handshakes()
    }
    pub fn set_broadcast(&self, on: bool) -> std::io::Result<()> {
        self.0.lock().expect("socket mutex").set_broadcast(on)
    }
}

impl NonBlockingSocket<SocketAddr> for SharedSocket {
    fn send_to(&mut self, msg: &Message, addr: &SocketAddr) {
        self.0.lock().expect("socket mutex").send_to(msg, addr);
    }
    fn receive_all_messages(&mut self) -> Vec<(SocketAddr, Message)> {
        self.0.lock().expect("socket mutex").receive_all_messages()
    }
}

impl NonBlockingSocket<SocketAddr> for OfSocket {
    fn send_to(&mut self, msg: &Message, addr: &SocketAddr) {
        if let Ok(buf) = bincode::serialize(msg) {
            // Send failures (e.g. transient unreachable) are not fatal: GGRS
            // retransmits what matters.
            let _ = self.socket.send_to(&buf, addr);
        }
    }

    fn receive_all_messages(&mut self) -> Vec<(SocketAddr, Message)> {
        self.drain();
        std::mem::take(&mut self.pending)
    }
}
