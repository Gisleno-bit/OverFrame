//! Persistent local settings: player identity, network port, input delay and
//! gamepad bindings.
//!
//! Stored as a tiny `key = value` text file so it is human-editable and needs no
//! serialisation dependency. Location (created on first save):
//!
//! * Windows: `%APPDATA%\overframe\overframe.cfg`
//! * macOS:   `~/Library/Application Support/overframe/overframe.cfg`
//! * Linux:   `$XDG_CONFIG_HOME/overframe/overframe.cfg` (or `~/.config/...`)
//!
//! A `bans.txt` next to it, if present, is the local ban list (see
//! `netcode::banlist`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::gamepad::PadBindings;
use crate::identity::Identity;
use crate::sim::roster::CharacterId;

/// Everything the game remembers between runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Random 64-bit identity generated on first run. Exchanged in the online
    /// handshake so a future ban list can refer to a stable id. It contains no
    /// hardware or personal information.
    pub player_id: u64,
    /// UDP port used when hosting.
    pub host_port: u16,
    /// GGRS input delay in frames (1–4 is typical; 2 is a good default).
    pub input_delay: u8,
    /// Display name shown to the peer (ASCII, short).
    pub name: String,
    /// Gamepad bindings for player slots 1 and 2.
    pub pad: [PadBindings; 2],
    /// Character last picked in the character-select screen (u8 id).
    pub last_character: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            player_id: generate_player_id(),
            host_port: 7777,
            input_delay: 2,
            name: "PLAYER".to_owned(),
            pad: [PadBindings::default(), PadBindings::default()],
            last_character: 0,
        }
    }
}

/// Generate a random-looking 64-bit id without a randomness dependency:
/// SplitMix64 over wall-clock nanos, the pid and two address values. This is
/// an *identifier*, not a secret — collisions are astronomically unlikely and
/// carry no security meaning.
pub fn generate_player_id() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let pid = std::process::id() as u64;
    let stack_probe = 0u8;
    let stack_addr = &stack_probe as *const u8 as u64;
    let heap_addr = Box::into_raw(Box::new(0u8)) as u64;
    // Reclaim the box so we don't leak.
    unsafe { drop(Box::from_raw(heap_addr as *mut u8)) };

    let mut x = nanos ^ (pid << 32) ^ stack_addr.rotate_left(17) ^ heap_addr.rotate_left(41);
    // SplitMix64 finaliser, applied twice.
    for _ in 0..2 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x = z ^ (z >> 31);
    }
    // Never hand out 0 — it's the "unknown" id.
    if x == 0 {
        1
    } else {
        x
    }
}

impl Settings {
    /// Serialise to the `key = value` text format.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str("# OVERFRAME settings — edit freely; unknown keys are ignored.\n");
        s.push_str(&format!("player_id = {:016x}\n", self.player_id));
        s.push_str(&format!("host_port = {}\n", self.host_port));
        s.push_str(&format!("input_delay = {}\n", self.input_delay));
        s.push_str(&format!("name = {}\n", sanitize_name(&self.name)));
        s.push_str(&format!("last_character = {}\n", self.last_character));
        for (i, b) in self.pad.iter().enumerate() {
            s.push_str(&format!("pad{}.jump = {}\n", i + 1, b.jump));
            s.push_str(&format!("pad{}.attack = {}\n", i + 1, b.attack));
            s.push_str(&format!("pad{}.special = {}\n", i + 1, b.special));
            s.push_str(&format!("pad{}.shield = {}\n", i + 1, b.shield));
            s.push_str(&format!("pad{}.grab = {}\n", i + 1, b.grab));
            s.push_str(&format!("pad{}.deadzone = {}\n", i + 1, b.deadzone));
        }
        s
    }

    /// Parse the text format. Missing keys keep their defaults; a missing
    /// `player_id` is generated fresh.
    pub fn from_text(text: &str) -> Settings {
        let mut kv: BTreeMap<String, String> = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                kv.insert(k.trim().to_owned(), v.trim().to_owned());
            }
        }
        let mut s = Settings::default();
        if let Some(v) = kv.get("player_id") {
            if let Ok(id) = u64::from_str_radix(v, 16) {
                if id != 0 {
                    s.player_id = id;
                }
            }
        }
        if let Some(v) = kv.get("host_port").and_then(|v| v.parse().ok()) {
            s.host_port = v;
        }
        if let Some(v) = kv.get("input_delay").and_then(|v| v.parse::<u8>().ok()) {
            s.input_delay = v.clamp(0, 8);
        }
        if let Some(v) = kv.get("name") {
            s.name = sanitize_name(v);
        }
        if let Some(v) = kv.get("last_character").and_then(|v| v.parse::<u8>().ok()) {
            s.last_character = v;
        }
        for i in 0..2 {
            let p = format!("pad{}.", i + 1);
            let get = |k: &str| {
                kv.get(&format!("{p}{k}"))
                    .and_then(|v| v.parse::<u32>().ok())
            };
            if let Some(v) = get("jump") {
                s.pad[i].jump = v;
            }
            if let Some(v) = get("attack") {
                s.pad[i].attack = v;
            }
            if let Some(v) = get("special") {
                s.pad[i].special = v;
            }
            if let Some(v) = get("shield") {
                s.pad[i].shield = v;
            }
            if let Some(v) = get("grab") {
                s.pad[i].grab = v;
            }
            if let Some(v) = kv
                .get(&format!("{p}deadzone"))
                .and_then(|v| v.parse::<f32>().ok())
            {
                s.pad[i].deadzone = v.clamp(0.0, 0.9);
            }
        }
        s
    }

    /// This install's identity (local id today; a Steam id in the Steam build).
    pub fn identity(&self) -> Identity {
        Identity::Local(self.player_id)
    }

    /// The last character the player selected (defaults to Kestrel).
    pub fn last_character(&self) -> CharacterId {
        CharacterId::from_u8(self.last_character).unwrap_or(CharacterId::Kestrel)
    }

    /// Path of the settings file for this platform.
    pub fn path() -> PathBuf {
        config_dir().join("overframe.cfg")
    }

    /// Load from disk, or return (and persist) fresh defaults.
    pub fn load_or_create() -> Settings {
        let path = Settings::path();
        match std::fs::read_to_string(&path) {
            Ok(text) => Settings::from_text(&text),
            Err(_) => {
                let s = Settings::default();
                let _ = s.save();
                s
            }
        }
    }

    /// Write to disk (creating the directory).
    pub fn save(&self) -> std::io::Result<()> {
        let path = Settings::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }
}

/// Keep display names short and printable-ASCII (they travel in the handshake
/// and are drawn with the bitmap font).
pub fn sanitize_name(n: &str) -> String {
    let cleaned: String = n
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(12)
        .collect::<String>()
        .to_ascii_uppercase();
    if cleaned.is_empty() {
        "PLAYER".to_owned()
    } else {
        cleaned
    }
}

/// Keep chat printable-ASCII and bounded for the bitmap font.
pub fn sanitize_chat(t: &str) -> String {
    t.chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .take(80)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Per-platform config directory (not created here).
pub fn config_dir() -> PathBuf {
    if let Ok(over) = std::env::var("OVERFRAME_CONFIG_DIR") {
        return PathBuf::from(over);
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("overframe");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("overframe");
        }
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("overframe");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("overframe");
    }
    PathBuf::from(".").join("overframe-config")
}
