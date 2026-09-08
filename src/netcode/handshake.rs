//! The pre-match protocol: connection handshake, LAN discovery and the lobby.
//!
//! Everything here travels as small datagrams with an 8-byte magic prefix so
//! the shared socket can tell them apart from GGRS traffic. The format is a
//! hand-rolled, versioned binary layout — no serialisation dependency, trivially
//! stable, fully covered by tests.
//!
//! ```text
//! guest ── Hello{identity, version, name, pass_hash} ──► host
//! guest ◄─ Welcome{identity, version, name, seed} ──────── host   (or Reject)
//! both  ◄── Lobby(Pick / Rules / Chat / Start / Leave) ──► both   (state-based, resent)
//! host  ── Announce{name, port, players, locked} ──► LAN broadcast (discovery)
//! ```

use crate::identity::Identity;

/// Magic prefix on every datagram of this protocol.
pub const MAGIC: &[u8; 8] = b"OVERFRHS";
/// Protocol version; bump when the layout or gameplay-affecting rules change so
/// mismatched builds refuse to play (they would desync).
pub const PROTOCOL_VERSION: u16 = 3;
/// UDP port LAN room announcements are broadcast to.
pub const DISCOVERY_PORT: u16 = 7778;

const TAG_HELLO: u8 = 1;
const TAG_WELCOME: u8 = 2;
const TAG_REJECT: u8 = 3;
const TAG_LOBBY: u8 = 4;
const TAG_ANNOUNCE: u8 = 5;

const LOBBY_PICK: u8 = 1;
const LOBBY_RULES: u8 = 2;
const LOBBY_CHAT: u8 = 3;
const LOBBY_START: u8 = 4;
const LOBBY_LEAVE: u8 = 5;

/// Match rules as they travel over the wire (host → guest).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RulesWire {
    pub stage: u8,
    pub stocks: u8,
    pub time_secs: u16,
}

/// The final, agreed configuration (host → guest) that both sides build their
/// [`crate::GameState`] from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StartWire {
    pub rules: RulesWire,
    pub seed: u32,
    /// `[host character, guest character]`.
    pub chars: [u8; 2],
    pub palettes: [u8; 2],
}

/// Messages exchanged while both players sit in the lobby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LobbyMsg {
    /// A player's own selection (sent by both; resent periodically).
    Pick {
        character: u8,
        palette: u8,
        ready: bool,
    },
    /// The host's rules (resent periodically).
    Rules(RulesWire),
    /// A chat line; `seq` lets the receiver drop duplicates of resent lines.
    Chat { seq: u32, text: String },
    /// Host: everything is agreed, build the match.
    Start(StartWire),
    /// Polite goodbye.
    Leave,
}

/// LAN room advertisement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announce {
    pub version: u16,
    pub name: String,
    pub port: u16,
    pub players: u8,
    pub max_players: u8,
    pub locked: bool,
}

/// All datagram kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Handshake {
    Hello {
        identity: Identity,
        version: u16,
        name: String,
        /// FNV-1a of the room password (0 = none).
        pass_hash: u64,
    },
    Welcome {
        identity: Identity,
        version: u16,
        name: String,
        seed: u32,
    },
    Reject {
        reason: String,
    },
    Lobby(LobbyMsg),
    Announce(Announce),
}

/// FNV-1a over a password string (0 for an empty password).
pub fn password_hash(p: &str) -> u64 {
    let p = p.trim();
    if p.is_empty() {
        return 0;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in p.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    if h == 0 {
        1
    } else {
        h
    }
}

// ------------------------------------------------------------ encoding

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str, max: usize) {
        let bytes = s.as_bytes();
        let n = bytes.len().min(max).min(255);
        // Never cut a UTF-8 sequence in half.
        let mut n = n;
        while n > 0 && !s.is_char_boundary(n) {
            n -= 1;
        }
        self.u8(n as u8);
        self.0.extend_from_slice(&bytes[..n]);
    }
    fn identity(&mut self, id: Identity) {
        let (kind, v) = id.to_wire();
        self.u8(kind);
        self.u64(v);
    }
}

struct R<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> R<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.at)?;
        self.at += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        let s = self.b.get(self.at..self.at + 2)?;
        self.at += 2;
        Some(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        let s = self.b.get(self.at..self.at + 4)?;
        self.at += 4;
        Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self) -> Option<u64> {
        let s = self.b.get(self.at..self.at + 8)?;
        self.at += 8;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Some(u64::from_le_bytes(a))
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(String::from_utf8_lossy(s).into_owned())
    }
    fn identity(&mut self) -> Option<Identity> {
        let kind = self.u8()?;
        let v = self.u64()?;
        Identity::from_wire(kind, v)
    }
}

impl Handshake {
    /// Serialise to a datagram.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W(Vec::with_capacity(96));
        w.0.extend_from_slice(MAGIC);
        match self {
            Handshake::Hello {
                identity,
                version,
                name,
                pass_hash,
            } => {
                w.u8(TAG_HELLO);
                w.identity(*identity);
                w.u16(*version);
                w.str(name, 32);
                w.u64(*pass_hash);
            }
            Handshake::Welcome {
                identity,
                version,
                name,
                seed,
            } => {
                w.u8(TAG_WELCOME);
                w.identity(*identity);
                w.u16(*version);
                w.str(name, 32);
                w.u32(*seed);
            }
            Handshake::Reject { reason } => {
                w.u8(TAG_REJECT);
                w.str(reason, 120);
            }
            Handshake::Lobby(m) => {
                w.u8(TAG_LOBBY);
                match m {
                    LobbyMsg::Pick {
                        character,
                        palette,
                        ready,
                    } => {
                        w.u8(LOBBY_PICK);
                        w.u8(*character);
                        w.u8(*palette);
                        w.u8(*ready as u8);
                    }
                    LobbyMsg::Rules(r) => {
                        w.u8(LOBBY_RULES);
                        w.u8(r.stage);
                        w.u8(r.stocks);
                        w.u16(r.time_secs);
                    }
                    LobbyMsg::Chat { seq, text } => {
                        w.u8(LOBBY_CHAT);
                        w.u32(*seq);
                        w.str(text, 120);
                    }
                    LobbyMsg::Start(s) => {
                        w.u8(LOBBY_START);
                        w.u8(s.rules.stage);
                        w.u8(s.rules.stocks);
                        w.u16(s.rules.time_secs);
                        w.u32(s.seed);
                        w.u8(s.chars[0]);
                        w.u8(s.chars[1]);
                        w.u8(s.palettes[0]);
                        w.u8(s.palettes[1]);
                    }
                    LobbyMsg::Leave => w.u8(LOBBY_LEAVE),
                }
            }
            Handshake::Announce(a) => {
                w.u8(TAG_ANNOUNCE);
                w.u16(a.version);
                w.str(&a.name, 32);
                w.u16(a.port);
                w.u8(a.players);
                w.u8(a.max_players);
                w.u8(a.locked as u8);
            }
        }
        w.0
    }

    /// Is this datagram part of this protocol at all?
    #[inline]
    pub fn is_handshake(bytes: &[u8]) -> bool {
        bytes.len() > MAGIC.len() && &bytes[..MAGIC.len()] == MAGIC
    }

    /// Parse a datagram. `None` if it isn't well-formed.
    pub fn decode(bytes: &[u8]) -> Option<Handshake> {
        if !Self::is_handshake(bytes) {
            return None;
        }
        let mut r = R {
            b: bytes,
            at: MAGIC.len(),
        };
        match r.u8()? {
            TAG_HELLO => {
                let identity = r.identity()?;
                let version = r.u16()?;
                let name = r.str()?;
                let pass_hash = r.u64()?;
                Some(Handshake::Hello {
                    identity,
                    version,
                    name,
                    pass_hash,
                })
            }
            TAG_WELCOME => {
                let identity = r.identity()?;
                let version = r.u16()?;
                let name = r.str()?;
                let seed = r.u32()?;
                Some(Handshake::Welcome {
                    identity,
                    version,
                    name,
                    seed,
                })
            }
            TAG_REJECT => Some(Handshake::Reject { reason: r.str()? }),
            TAG_LOBBY => {
                let m = match r.u8()? {
                    LOBBY_PICK => LobbyMsg::Pick {
                        character: r.u8()?,
                        palette: r.u8()?,
                        ready: r.u8()? != 0,
                    },
                    LOBBY_RULES => LobbyMsg::Rules(RulesWire {
                        stage: r.u8()?,
                        stocks: r.u8()?,
                        time_secs: r.u16()?,
                    }),
                    LOBBY_CHAT => LobbyMsg::Chat {
                        seq: r.u32()?,
                        text: r.str()?,
                    },
                    LOBBY_START => LobbyMsg::Start(StartWire {
                        rules: RulesWire {
                            stage: r.u8()?,
                            stocks: r.u8()?,
                            time_secs: r.u16()?,
                        },
                        seed: r.u32()?,
                        chars: [r.u8()?, r.u8()?],
                        palettes: [r.u8()?, r.u8()?],
                    }),
                    LOBBY_LEAVE => LobbyMsg::Leave,
                    _ => return None,
                };
                Some(Handshake::Lobby(m))
            }
            TAG_ANNOUNCE => Some(Handshake::Announce(Announce {
                version: r.u16()?,
                name: r.str()?,
                port: r.u16()?,
                players: r.u8()?,
                max_players: r.u8()?,
                locked: r.u8()? != 0,
            })),
            _ => None,
        }
    }
}
