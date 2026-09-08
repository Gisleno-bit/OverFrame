# Shipping OVERFRAME on Steam (Fase 3 checklist)

This is the concrete, ordered checklist for taking the current build to a Steam
Early-Access page. The code seam for all of it is
`src/netcode/platform.rs` — see `docs/DESIGN.md` → "Steam integration".

> Status: **not started.** The game is fully playable online today over direct
> UDP + LAN discovery; Steam replaces *matchmaking and transport*, not gameplay.

## 1. Steamworks account & app

- [ ] Join the [Steamworks partner program](https://partner.steamgames.com/)
      (one-time fee per app, refundable after revenue threshold).
- [ ] Create the app, get its **App ID**. Put it in `steam_appid.txt` (dev only)
      and in CI secrets for release builds.
- [ ] Reserve the store page name, upload capsule art, screenshots, trailer.
      (Capsule/marketing art must be original — see `docs/LEGAL.md`.)
- [ ] Set the app to "Coming soon / Early Access" and fill the EA questionnaire.

## 2. SDK & Rust binding

- [ ] Download the Steamworks SDK (the C++ redistributable DLL/.so/.dylib).
- [ ] Add a `steam` cargo feature: `steamworks = "0.11"` (Rust binding), behind
      `#[cfg(feature = "steam")]`. Do **not** make it default — the LAN build
      must keep working without Steam.
- [ ] Ship the Steamworks redistributable next to the executable (Steam does
      this automatically for depots).

## 3. Auth & identity

- [ ] `steamworks::Client::init_app(APP_ID)` at startup; if it fails (no Steam
      running), fall back to `LanPlatform` and show "Steam offline — LAN only".
- [ ] Implement `SteamPlatform: netcode::platform::Platform`; `identity()`
      returns `Identity::Steam(steam_id.raw())`. The ban list already keys on
      Steam ids (`steam:<id>`), so tournament bans work immediately.

## 4. Matchmaking & lobbies

- [ ] Create/join lobbies with `ISteamMatchmaking` (`CreateLobby`,
      `RequestLobbyList`, filters for region + a `ping` metadata key).
- [ ] Store the room's rules/password as lobby metadata (host writes, guests
      read) — reuse the `RulesWire` fields.
- [ ] Friend invites: `InviteUserToLobby`, and handle
      `GameLobbyJoinRequested` (accepting an invite from the friends list or a
      launch argument `+connect_lobby`).
- [ ] In-lobby chat: `SendLobbyChatMsg` / `LobbyChatMsg` carrying our `Chat`
      payload (or keep our own `LobbyMsg::Chat` over the transport below).

## 5. Transport (the important one)

- [ ] Implement `SteamSocket: ggrs::NonBlockingSocket<SteamId>` on top of
      `ISteamNetworkingMessages::SendMessageToUser` /
      `ReceiveMessagesOnChannel`. Enable **Steam Datagram Relay** so packets go
      through Valve's relays: no port forwarding, IP hidden, DoS-protected.
- [ ] Add `NetMatch::host_steam(...)` / `join_steam(...)` that build the GGRS
      `P2PSession` on `SteamSocket` instead of `OfSocket`. Everything else
      (lobby state machine, `advance`, stats, desync detection) is unchanged.
- [ ] Keep the direct-UDP path compiled too, so LAN and non-Steam builds work.

## 6. Cloud & config

- [ ] Steam Cloud sync for `overframe.cfg` and `bans.txt` (auto-cloud by path,
      or the `ISteamRemoteStorage` API).
- [ ] Optionally publish/subscribe tournament ban lists via Steam Workshop.

## 7. Build, depots, release

- [ ] Windows is the primary depot; add Linux (and macOS if notarised) depots.
- [ ] `cargo build --release --features "gui,steam"`; bundle the Steamworks
      redistributable; upload with `steamcmd` (`app_build_*.vdf`).
- [ ] Run Valve's automated content check; pass the review (no malware, correct
      age rating, EA disclaimer).
- [ ] Set price / free, launch date, and the EA "what's left" description.

## 8. Compliance

- [ ] Trailer, screenshots, capsule and all in-game assets are original
      (`docs/LEGAL.md`). No trademarks, no lookalike characters.
- [ ] EA page states clearly it is a community platform fighter, not affiliated
      with any other game or company.

## Dependency note (answering "do GGRS and Steamworks conflict?")

No. They sit in different layers: GGRS is a pure-Rust rollback library that only
needs an `impl NonBlockingSocket`; `steamworks` is an FFI binding to a C++ DLL
providing matchmaking + a socket. They never call each other — the `SteamSocket`
adapter is the only glue, ~150 lines. The only build consideration is that the
Steamworks redistributable must be present at link/run time, which the `steam`
feature and the depot handle.
