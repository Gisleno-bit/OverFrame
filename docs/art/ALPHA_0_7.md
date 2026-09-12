# OVERFRAME Alpha 0.7 — active delivery direction

The product owner's 2026-09-11 request supersedes the previous v1.0.0-pilot delivery order. The canonical repository remains `Gisleno-bit/OverFrame`. Alpha 0.7 prioritizes game feel, character identity, animation, camera and presentation. Existing Trama work is preserved; it is not a prerequisite in this Alpha's requested Kestrel/Boulder/Viper sequence.

ChatGPT directs technical decisions, gameplay and art, diagnoses observed results and supplies concrete acceptance criteria. Claude implements and reports constraints; ChatGPT inspects the source, executes checks/captures and reviews the result. The product owner retains major irreversible decisions. Normal incremental corrections proceed under the user's authorization. Compilation and passing tests are minimum gates, never the artistic acceptance criterion.

## 2026-09-12 — Animation ownership update

The product owner will supply animations for the characters after Kestrel. Do not focus work on, specify new motion for, or author animations for Boulder, Viper, Trama or subsequent characters unless the product owner explicitly requests it. Preserve their existing work. Kestrel keeps the current animation direction, implementation and review plan, including Special N, charged release and throws.

Continue the other authorized technical, gameplay, character-presentation, camera, stage, HUD and verification work. The later fighter phases cover their other needs; their animation work waits for the owner's material or explicit direction. The older requests below for complete Boulder/Viper move direction are superseded by this update. Receiving material does not itself constitute implementation or visual approval.

## Current verified starting point

At the initial audit, `main` is `ea645ba1cbd0070aa602daa56352b034eb706299`, version 0.6.0. CI and visual evidence pass with 129 Rust tests. The published status still marks `animation_direction_kestrel` as `specified_not_implemented` and `art_review_kestrel` as `not_verified`.

Evidence: commit `0805cd0d6232347409f53f4ac3227d6b3fb8a0e1`, run `34546401262`, prefix `builds/ea645ba1cbd0070aa602daa56352b034eb706299/attempt-34546401262-1/`, renderer `game3d` under Mesa. The current back-air and neutral-air sheets still show the previously diagnosed rearward contact and late-active retraction defects. This is the unchanged integrated evaluator, not the new Opus implementation. File hashes of the downloaded review images were verified against its manifest.

The existing Claude task contains partial new directed-animation, parsing, contact/support and capture work. A pending implementation checkpoint has been preserved and its archive hash verified locally (`3f5ff23d19161275b7044fb2ccfe459e78879a991c5505998d6aa3b74435e5b0`). It has not been integrated or approved. Claude is completing the mixed-event capture wiring and checks; inspect and verify the final archive separately. The detailed failure measurements and contracts remain in [the Kestrel review](reviews/91f9c5a2834353bbb7207dcc4d866a4e513fd48f.md) and [animation format](procedural/anim/FORMAT.md).

## Required order and acceptance

| Phase | Direction | Evidence required before advancing |
| --- | --- | --- |
| 1. Kestrel | Fast-faller: technical, aggressive, precise, sharp and energetic. Each move has its own physical intention, line of action, anticipation when runtime allows, impact, follow-through and recovery. | Review every runtime action/variant individually, with both facings, actual contact geometry, first/late active ticks, hitlag/charge, support feet and representative motion. Strong contact and readable silhouettes are necessary; a generic shared aim pose is insufficient. |
| 2. Camera | Keep both fighters readable, anticipate separation, ease zoom and recentering, retain useful 3D depth without distortion or nervous motion. | Gameplay captures/video for close combat, separation, vertical play, ledges, recovery, KOs and extreme positions. Compare timing and framing while preserving the fixed diagnostic cameras used for art evidence. |
| 3. Boulder | Continue non-animation work for its stable heavyweight identity and Bulwark/armor readability. The owner supplies its animations. | Review the changed systems and actual presentation. Defer animation authorship and new move direction until the owner explicitly requests them. |
| 4. Viper | Continue non-animation work for its light, evasive, aerial identity and existing two air jumps/air control. The owner supplies its animations. | Review the changed systems and actual presentation. Defer animation authorship and new move direction until the owner explicitly requests them. |
| 5. Stages | Complete priority dressing, especially The Lattice: original composition, lighting, materials, background and depth. | Actual stage/combat images with readable fighters, clear collision surfaces and restrained background detail. |
| 6. HUD/identity | Clean competitive hierarchy: damage, stocks, fighter/palette identity, timer, damage/KO feedback and relevant online metrics. | Real menus and HUD at common resolutions, with readable type, spacing and contrast. |
| 7. Game feel | Review movement, jumps, landing/L-cancel, attacks, grabs/throws, ledge/tech/rolls, shields, hitlag/stun/knockback, DI/SDI and KOs as interactions. | Runtime sequences and actionable observations. Sound, rumble, shake, flashes, trails and particles reinforce the action without obscuring it. |
| 8. Windows/input | Windows 11 x64, keyboard, Xbox/XInput, hotplug/rebinding, window/fullscreen, resolutions, refresh rates, audio/rumble/settings, selection, versus/training and repeated sessions. | Identified release executable on the real PC, measured frame pacing, actual device input and close/reopen tests. Compilation, virtual-device enumeration or keyboard input do not prove physical-controller behavior. |
| 9. Online sanity | Preserve existing deterministic architecture and rollback. | Determinism/checksums plus two local instances, host/join and basic synchronization; cosmetic state cannot affect causality or introduce desync. |
| 10. Alpha delivery | Downloadable Windows build that needs no development tools, consistent version/label and an accurate changelog. | Formatter, warnings-denied clippy, all required tests, visual evidence and reviews, correct PROJECT_STATUS, release build and executable QA. Tag/publish only after the build meets these gates. |

## Kestrel's first implementation gate

Keep the current 22-action JSON and 23 runtime variants as the implementation baseline. Complete exact contact-piece selection, strict parsing, all clean/late activity, charged hold, jab chaining and hitlag. Correct back-air direction, premature neutral-air recovery, grounded support and overflowing captions from the measured evidence. Include every genuine neutral-special event. The subsequent [gameplay correction](reviews/4ddb10315b936f8b70e5957a5adcd986e8039e9c.md) deliberately removes only Kestrel's later zero-damage fighter hitbox; its projectile emission remains required. Other fighters retain their actual mixed events and supplemental evidence. Follow this with the [compact original Kestrel discharge](procedural/anim/kestrel-special-n-presentation.md).

After the implementation is transferred, test it and review every action before accepting its visual direction. Numerical intersection is not enough: inspect the torso, counterbalancing hand, noncontact leg, head/crest, scarf and support transition as a coherent action. Immediate runtime attacks must still respond immediately; do not invent startup just to add anticipation.

## Original neutral-special exploration

Study the gameplay role of interruption, lane control, approach pressure and commitments rather than reproducing another game's assets or move tables. The initial design question is whether an original compact energy discharge gives Kestrel a useful short, sharp ranged action consistent with its fast-faller identity. First verify the current actual emission, flight, later fighter contact and recovery. Then evaluate any proposed behavior change against spacing, pressure, airborne use, punishability and determinism, with explicit before/after evidence.

Use FightCore, Dragdown/Rivals 2 and SSBWiki as references for move function and readability. If the supplied melee-master reference is consulted, record the exact local source and observation, and use it only for permitted mechanical/technical study. Do not copy game code, tables, assets, designs or animation curves into the shipped game. Existing reference work is retained; repeat research only where the new gameplay question needs it.

## Iteration rules

Inspect → diagnose → specify → Claude implements → execute → observe → correct → test → commit. Each instruction names files, action, observed failure, desired pose/behavior, relation to runtime data, checks and required captures. Inspect implementation personally; a Claude success claim does not close an observation.

Preserve original assets and deterministic simulation. Do not rewrite a solid system without a concrete defect. Gameplay changes require an explicit design decision, baseline and regression evidence; they must not be disguised as a fix for artwork that fails to reach an existing hitbox. Preserve fixed diagnostic cameras while evaluating the separate gameplay camera. Keep unrun or unavailable QA pending, and preserve dated checkpoints and exact-source handoffs through [WORKFLOW.md](WORKFLOW.md).

## Windows iteration checkpoint

The [checkpoint-2 Windows review](reviews/69917fb3a501bf9ad8d3fd465e71c2235709b271.md) records actual images, preserved simulation data, rejected test exceptions and the next concrete pose corrections. Its implementation SHA is an isolated unpublished WIP, not main or an approved delivery. The [gameplay review direction](procedural/anim/kestrel-gameplay-direction.md) records original move roles and follow-up reproduction cases.


## Integrated technical iteration

The [019c2b1 integration review](reviews/019c2b141b0ddce864c35fcd03ec7163922e85cd.md) records the transferred directed-animation source, passing Windows build, matching local/CI evidence and its remaining visual gate. Kestrel is still the active phase; the [low-sweep/upward-release direction](procedural/anim/kestrel-physical-iteration.md) is the next visual correction.


## 2026-09-11 — Low sweep and upward-smash correction

The [b9e650b Windows candidate review](reviews/b9e650b93a94b8b56061cdf30789d7fc68ad7cd2.md) accepts the corrected low-sweep chamber and upward extension as an incremental improvement. The actual Windows capture has no grounded foot penetration, preserves active contact and leaves simulation fixtures unchanged. Full Kestrel approval remains pending. Next: implement the [explicit gameplay corrections](procedural/anim/kestrel-gameplay-fixes.md), then finish the compact original SpecialN presentation.
