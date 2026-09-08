//! Connection handshake that runs *before* GGRS takes over the socket.
//!
//! GGRS needs to know the remote address when the session is built, but a
//! host doesn't know who will join. So the guest first sends a [`Handshake::Hello`]
//! to the host's port; the host learns the guest's address from that datagram,
//! checks the ban list, answers [`Handshake::Welcome`] (carrying the match seed
//! so both sides build an identical [`crate::GameState`]), and *then* both sides
//! start their GGRS P2P session on the very same UDP socket.
//!
//! Wire format is a tiny hand-rolled binary layout with an 8-byte magic so the
//! socket can tell handshake datagrams apart from GGRS traffic. No third-party
//! serialisation is involved, which keeps it trivially stable and testable.

/// Magic prefix on every handshake datagram.
pub const MAGIC: &[u8; 8] = b"OVERFRHS";
/// Protocol version; bump when the layout or gameplay-affecting rules change so
/// mismatched builds refuse to play (they would desync).
pub const PROTOCOL_VERSION: u16 = 2;

const TAG_HELLO: u8 = 1;
const TAG_WELCOME: u8 = 2;
const TAG_REJECT: u8 = 3;

/// The three handshake messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Handshake {
    /// Guest → host: "I want to play".
    Hello {
        player_id: u64,
        version: u16,
        name: String,
    },
    /// Host → guest: "OK, here's the match seed".
    Welcome {
        player_id: u64,
        version: u16,
        seed: u32,
        input_delay: u8,
        name: String,
    },
    /// Host → guest: "No" (version mismatch, banned, room full…).
    Reject { reason: String },
}

fn push_str(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let n = bytes.len().min(32);
    buf.push(n as u8);
    buf.extend_from_slice(&bytes[..n]);
}

fn read_str(b: &[u8], at: &mut usize) -> Option<String> {
    let n = *b.get(*at)? as usize;
    *at += 1;
    let s = b.get(*at..*at + n)?;
    *at += n;
    Some(String::from_utf8_lossy(s).into_owned())
}

fn read_u16(b: &[u8], at: &mut usize) -> Option<u16> {
    let s = b.get(*at..*at + 2)?;
    *at += 2;
    Some(u16::from_le_bytes([s[0], s[1]]))
}
fn read_u32(b: &[u8], at: &mut usize) -> Option<u32> {
    let s = b.get(*at..*at + 4)?;
    *at += 4;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn read_u64(b: &[u8], at: &mut usize) -> Option<u64> {
    let s = b.get(*at..*at + 8)?;
    *at += 8;
    let mut a = [0u8; 8];
    a.copy_from_slice(s);
    Some(u64::from_le_bytes(a))
}

impl Handshake {
    /// Serialise to a datagram.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(MAGIC);
        match self {
            Handshake::Hello {
                player_id,
                version,
                name,
            } => {
                buf.push(TAG_HELLO);
                buf.extend_from_slice(&player_id.to_le_bytes());
                buf.extend_from_slice(&version.to_le_bytes());
                push_str(&mut buf, name);
            }
            Handshake::Welcome {
                player_id,
                version,
                seed,
                input_delay,
                name,
            } => {
                buf.push(TAG_WELCOME);
                buf.extend_from_slice(&player_id.to_le_bytes());
                buf.extend_from_slice(&version.to_le_bytes());
                buf.extend_from_slice(&seed.to_le_bytes());
                buf.push(*input_delay);
                push_str(&mut buf, name);
            }
            Handshake::Reject { reason } => {
                buf.push(TAG_REJECT);
                push_str(&mut buf, reason);
            }
        }
        buf
    }

    /// Is this datagram a handshake message at all?
    #[inline]
    pub fn is_handshake(bytes: &[u8]) -> bool {
        bytes.len() > MAGIC.len() && &bytes[..MAGIC.len()] == MAGIC
    }

    /// Parse a datagram. `None` if it isn't a well-formed handshake.
    pub fn decode(bytes: &[u8]) -> Option<Handshake> {
        if !Self::is_handshake(bytes) {
            return None;
        }
        let mut at = MAGIC.len();
        let tag = *bytes.get(at)?;
        at += 1;
        match tag {
            TAG_HELLO => {
                let player_id = read_u64(bytes, &mut at)?;
                let version = read_u16(bytes, &mut at)?;
                let name = read_str(bytes, &mut at)?;
                Some(Handshake::Hello {
                    player_id,
                    version,
                    name,
                })
            }
            TAG_WELCOME => {
                let player_id = read_u64(bytes, &mut at)?;
                let version = read_u16(bytes, &mut at)?;
                let seed = read_u32(bytes, &mut at)?;
                let input_delay = *bytes.get(at)?;
                at += 1;
                let name = read_str(bytes, &mut at)?;
                Some(Handshake::Welcome {
                    player_id,
                    version,
                    seed,
                    input_delay,
                    name,
                })
            }
            TAG_REJECT => {
                let reason = read_str(bytes, &mut at)?;
                Some(Handshake::Reject { reason })
            }
            _ => None,
        }
    }
}
