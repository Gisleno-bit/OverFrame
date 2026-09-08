//! Player identity, abstracted over the platform that vouches for it.
//!
//! Today every identity is [`Identity::Local`] (the random id from the settings
//! file). With the Steam build, [`Identity::Steam`] carries the SteamID64 that
//! the Steamworks client authenticates. The ban list, the handshake and the
//! lobby all work on `Identity`, so the Steam integration doesn't touch them.

use std::fmt;

/// Who a player is, and who says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Identity {
    /// Self-generated random id (see `config::Settings::player_id`).
    Local(u64),
    /// SteamID64, authenticated by the Steam client (Fase 3 Steam build).
    Steam(u64),
}

impl Identity {
    /// `(kind, value)` for the wire.
    pub fn to_wire(self) -> (u8, u64) {
        match self {
            Identity::Local(v) => (0, v),
            Identity::Steam(v) => (1, v),
        }
    }

    pub fn from_wire(kind: u8, v: u64) -> Option<Identity> {
        match kind {
            0 => Some(Identity::Local(v)),
            1 => Some(Identity::Steam(v)),
            _ => None,
        }
    }

    /// The raw 64-bit value (without the kind).
    pub fn raw(self) -> u64 {
        match self {
            Identity::Local(v) | Identity::Steam(v) => v,
        }
    }

    /// Parse the textual form used in ban lists: `local:<hex16>`,
    /// `steam:<decimal SteamID64>`, or a bare `<hex16>` (legacy = local).
    pub fn parse(s: &str) -> Option<Identity> {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix("steam:") {
            return rest.parse::<u64>().ok().map(Identity::Steam);
        }
        let rest = s.strip_prefix("local:").unwrap_or(s);
        u64::from_str_radix(rest, 16).ok().map(Identity::Local)
    }

    /// Short form for HUDs (`L:3F0A91C2`, `S:7656…`).
    pub fn short(self) -> String {
        match self {
            Identity::Local(v) => format!("L:{:08X}", (v >> 32) as u32),
            Identity::Steam(v) => format!("S:{v}"),
        }
    }
}

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Identity::Local(v) => write!(f, "local:{v:016x}"),
            Identity::Steam(v) => write!(f, "steam:{v}"),
        }
    }
}
