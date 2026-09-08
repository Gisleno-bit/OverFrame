//! Ban list: data structure and loader, prepared for the Fase 3 ban system.
//!
//! Design (see `docs/DESIGN.md`, "Ban system"): there is **no central server**.
//! A tournament organiser publishes a plain-text list of banned player ids,
//! signed with their key; players (or a TO-run host) drop it in the config
//! directory as `bans.txt`, and the host refuses handshakes from listed ids.
//! This module implements the format, lookup and expiry now; signature
//! *verification* is a documented stub until Fase 3 adds an ed25519 dependency.
//!
//! Format (one entry per line, `#` comments):
//!
//! ```text
//! # OVERFRAME ban list
//! issuer Madrid Weekly TO
//! ban local:3fa9c1d2e4b5a678 until 1767225600 reason repeated no-shows
//! ban steam:76561198000000000 reason cheating
//! ban 00ddeeff00112233 reason legacy bare hex = local
//! sig <base64 signature over all lines above, Fase 3>
//! ```

use std::path::PathBuf;

use crate::identity::Identity;

/// One banned identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BanEntry {
    pub identity: Identity,
    /// Unix seconds after which the ban no longer applies (`None` = permanent).
    pub until: Option<u64>,
    pub reason: String,
}

/// Result of checking a list's signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    /// The list carries no signature line.
    Unsigned,
    /// A signature is present but this build cannot verify it yet (Fase 3).
    Unverified,
}

/// A parsed ban list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BanList {
    pub issuer: String,
    pub entries: Vec<BanEntry>,
    pub signature: Option<String>,
}

impl BanList {
    /// Parse the text format. Malformed lines are skipped, never fatal.
    pub fn parse(text: &str) -> BanList {
        let mut list = BanList::default();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut words = line.split_whitespace();
            match words.next() {
                Some("issuer") => {
                    list.issuer = words.collect::<Vec<_>>().join(" ");
                }
                Some("sig") => {
                    list.signature = words.next().map(|s| s.to_owned());
                }
                Some("ban") => {
                    let identity = match words.next().and_then(Identity::parse) {
                        Some(id) => id,
                        None => continue,
                    };
                    let mut until = None;
                    let mut reason = Vec::new();
                    let mut rest = words.peekable();
                    while let Some(w) = rest.next() {
                        match w {
                            "until" => {
                                until = rest.next().and_then(|u| u.parse::<u64>().ok());
                            }
                            "reason" => {
                                reason = rest.by_ref().map(|s| s.to_owned()).collect();
                            }
                            _ => {}
                        }
                    }
                    list.entries.push(BanEntry {
                        identity,
                        until,
                        reason: reason.join(" "),
                    });
                }
                _ => {}
            }
        }
        list
    }

    /// Serialise back to the text format.
    pub fn to_text(&self) -> String {
        let mut s = String::from("# OVERFRAME ban list\n");
        if !self.issuer.is_empty() {
            s.push_str(&format!("issuer {}\n", self.issuer));
        }
        for e in &self.entries {
            s.push_str(&format!("ban {}", e.identity));
            if let Some(u) = e.until {
                s.push_str(&format!(" until {u}"));
            }
            if !e.reason.is_empty() {
                s.push_str(&format!(" reason {}", e.reason));
            }
            s.push('\n');
        }
        if let Some(sig) = &self.signature {
            s.push_str(&format!("sig {sig}\n"));
        }
        s
    }

    /// Is `identity` banned at time `now` (unix seconds)?
    pub fn is_banned(&self, identity: Identity, now: u64) -> Option<&BanEntry> {
        self.entries
            .iter()
            .find(|e| e.identity == identity && e.until.map_or(true, |u| now < u))
    }

    /// Add a permanent ban (no-op if already listed) and persist to the local
    /// file. Used by the host's in-lobby "kick & ban".
    pub fn add_and_save(&mut self, identity: Identity, reason: &str) -> std::io::Result<()> {
        if !self.entries.iter().any(|e| e.identity == identity) {
            self.entries.push(BanEntry {
                identity,
                until: None,
                reason: reason.to_owned(),
            });
        }
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }

    /// Signature status. Verification itself lands in Fase 3.
    pub fn verification(&self) -> Verification {
        if self.signature.is_some() {
            Verification::Unverified
        } else {
            Verification::Unsigned
        }
    }

    /// Path of the local ban list file.
    pub fn path() -> PathBuf {
        crate::config::config_dir().join("bans.txt")
    }

    /// Load the local ban list, or an empty one if absent.
    pub fn load_local() -> BanList {
        std::fs::read_to_string(Self::path())
            .map(|t| BanList::parse(&t))
            .unwrap_or_default()
    }
}

/// Current unix time in seconds (0 if the clock is unavailable).
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
