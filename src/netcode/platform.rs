//! Platform abstraction: the seam where Steam plugs in.
//!
//! Everything online today runs over direct UDP + LAN discovery. The Steam build
//! (Fase 3) keeps the **entire simulation, GGRS layer, lobby state machine and
//! ban logic unchanged** and only swaps *how peers find each other and how their
//! packets travel*. That swap lives behind this trait, so no gameplay code has
//! to know which platform is active.
//!
//! ## What Steam changes, precisely
//!
//! | Concern | LAN backend (today) | Steam backend (Fase 3) |
//! |---|---|---|
//! | Identity | random `Identity::Local` | `Identity::Steam(SteamID64)`, authenticated |
//! | Room list | UDP broadcast (`lobby::Browser`) | `ISteamMatchmaking` lobby list |
//! | Invites / join | room code / `ip:port` | Steam friend invite + `GameLobbyJoinRequested` |
//! | Transport | our `OfSocket` (UDP) | Steam Datagram Relay via `ISteamNetworkingMessages` |
//! | Region/ping filter | none | lobby metadata + SDR ping location |
//!
//! GGRS is transport-agnostic: it drives an `impl NonBlockingSocket`. The Steam
//! backend provides a `SteamSocket` that implements the same trait on top of
//! `ISteamNetworkingMessages` (send/receive to a `SteamID`), so
//! `session::NetMatch` builds its `P2PSession` on that instead of `OfSocket`
//! with no other change. SDR then gives relay-assisted, DoS-protected,
//! IP-hiding connectivity **without any port forwarding** — which is the main
//! practical win of going through Steam.
//!
//! This module defines the trait and ships the LAN implementation. The Steam
//! implementation is added behind a `steam` cargo feature that pulls the
//! `steamworks` crate; see `docs/DESIGN.md` for the integration plan and
//! `docs/STEAM.md` for the Steamworks setup checklist.

use crate::identity::Identity;
use crate::netcode::lobby::Room;

/// How a room is reached once chosen from a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinTarget {
    /// A UDP socket address (LAN / direct IP).
    Address(std::net::SocketAddr),
    /// A Steam lobby id (Fase 3).
    SteamLobby(u64),
}

/// The services a networking platform must provide. The game depends on this
/// trait, never on a concrete backend.
pub trait Platform {
    /// Stable, platform-vouched identity of the local player.
    fn identity(&self) -> Identity;

    /// Human-readable name for handshakes and the HUD.
    fn display_name(&self) -> String;

    /// Refresh and return the current list of joinable rooms.
    fn poll_rooms(&mut self) -> Vec<Room>;

    /// Whether this platform hides the player's IP (Steam does via SDR).
    fn hides_ip(&self) -> bool;

    /// Short label for the UI ("LAN", "STEAM").
    fn label(&self) -> &'static str;
}

/// The default backend: LAN discovery + direct UDP, no external service.
pub struct LanPlatform {
    identity: Identity,
    name: String,
    browser: Option<crate::netcode::lobby::Browser>,
}

impl LanPlatform {
    pub fn new(identity: Identity, name: String) -> Self {
        LanPlatform {
            identity,
            name,
            // Browsing is optional: only one instance per machine can bind the
            // discovery port, and hosting doesn't need it.
            browser: crate::netcode::lobby::Browser::new().ok(),
        }
    }
}

impl Platform for LanPlatform {
    fn identity(&self) -> Identity {
        self.identity
    }
    fn display_name(&self) -> String {
        self.name.clone()
    }
    fn poll_rooms(&mut self) -> Vec<Room> {
        match self.browser.as_mut() {
            Some(b) => {
                b.poll();
                b.rooms()
            }
            None => Vec::new(),
        }
    }
    fn hides_ip(&self) -> bool {
        false
    }
    fn label(&self) -> &'static str {
        "LAN"
    }
}

// The Steam backend lands here behind `#[cfg(feature = "steam")]`:
//
//   pub struct SteamPlatform { client: steamworks::Client, ... }
//   impl Platform for SteamPlatform { /* ISteamMatchmaking + SDR */ }
//
// and `session::NetMatch` gains a constructor that takes a `SteamSocket`
// (an `impl NonBlockingSocket<SteamId>`), leaving the lobby/GGRS code intact.
