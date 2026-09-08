//! Room codes: a short, shareable encoding of `IPv4:port`.
//!
//! Direct-IP play means the guest needs the host's address. Typing
//! `192.168.1.37:7777` is error-prone, so we pack the 6 bytes (4 of address,
//! 2 of port) into 10 Crockford base-32 symbols and show them as `XXXXX-XXXXX`.
//! Decoding is forgiving (case, dashes/spaces, `O`→`0`, `I`/`L`→`1`), and a raw
//! `ip:port` or bare `ip` (default port) is accepted too.
//!
//! Codes are not secrets — they are just an address. Over the internet the host
//! must share a code built from their **public** IP and forward the UDP port.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Default UDP port for hosting.
pub const DEFAULT_PORT: u16 = 7777;

/// Encode an address as a room code, e.g. `"C0G81-5N2E1"`.
pub fn encode(ip: Ipv4Addr, port: u16) -> String {
    let o = ip.octets();
    // 48 bits: ip(32) | port(16), packed big-endian into a u64.
    let v: u64 = ((o[0] as u64) << 40)
        | ((o[1] as u64) << 32)
        | ((o[2] as u64) << 24)
        | ((o[3] as u64) << 16)
        | (port as u64);
    // 10 symbols × 5 bits = 50 bits; left-align the 48 bits.
    let v = v << 2;
    let mut out = String::with_capacity(11);
    for i in 0..10 {
        let shift = 45 - i * 5;
        let sym = ((v >> shift) & 0x1F) as usize;
        out.push(ALPHABET[sym] as char);
        if i == 4 {
            out.push('-');
        }
    }
    out
}

fn symbol_value(c: char) -> Option<u64> {
    let c = c.to_ascii_uppercase();
    let c = match c {
        'O' => '0',
        'I' | 'L' => '1',
        other => other,
    };
    ALPHABET
        .iter()
        .position(|&a| a as char == c)
        .map(|p| p as u64)
}

/// Decode a room code back to an address. Also accepts `ip:port` / `ip`.
pub fn decode(text: &str) -> Option<SocketAddr> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    // Raw socket address?
    if let Ok(sa) = t.parse::<SocketAddr>() {
        return Some(sa);
    }
    if let Ok(ip) = t.parse::<IpAddr>() {
        return Some(SocketAddr::new(ip, DEFAULT_PORT));
    }
    // Room code.
    let syms: Vec<char> = t
        .chars()
        .filter(|c| !matches!(c, '-' | ' ' | '_' | '.'))
        .collect();
    if syms.len() != 10 {
        return None;
    }
    let mut v: u64 = 0;
    for c in syms {
        v = (v << 5) | symbol_value(c)?;
    }
    let v = v >> 2;
    let ip = Ipv4Addr::new(
        ((v >> 40) & 0xFF) as u8,
        ((v >> 32) & 0xFF) as u8,
        ((v >> 24) & 0xFF) as u8,
        ((v >> 16) & 0xFF) as u8,
    );
    let port = (v & 0xFFFF) as u16;
    if port == 0 {
        return None;
    }
    Some(SocketAddr::new(IpAddr::V4(ip), port))
}

/// Best-effort LAN IPv4 of this machine (the interface that would route to the
/// internet). Uses the classic "connect a UDP socket" trick, which sends no
/// packets. Falls back to loopback.
pub fn local_ipv4() -> Ipv4Addr {
    let probe = || -> Option<Ipv4Addr> {
        let s = UdpSocket::bind("0.0.0.0:0").ok()?;
        s.connect("192.0.2.1:9").ok()?; // TEST-NET, never actually sent to
        match s.local_addr().ok()?.ip() {
            IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
            _ => None,
        }
    };
    probe().unwrap_or(Ipv4Addr::LOCALHOST)
}
