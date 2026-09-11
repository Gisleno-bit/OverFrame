# Kestrel gameplay corrections after the Windows baseline

Technical/gameplay direction, 2026-09-11. Implementation follows the separate low-sweep/upward-smash visual correction; do not mix the two deliveries. These are deliberate gameplay decisions based on observed interactions, not accommodations for artwork.

## Baseline and its limits

Simulation source: `019c2b141b0ddce864c35fcd03ec7163922e85cd`, unchanged on documentation-only main `8a5bee927f30fb2633faca84508d7658a1f430f3`. Claude diagnostic ZIP SHA-256: `1da37015658c681e4a211ba2503400c35712806e1627d107bc0ff00eec6b9588`; `tests/gameplay_diagnostics.rs` SHA-256: `ffd8a8e0c509cd45b6d558b00c829b7ea0b81921a6b7920a3c674e29eb9fcd09`. The exact file was run on Windows in an isolated copy: 10 diagnostic tests passed. These assert fixture setup, not desired game behavior, and are not a gameplay approval.

Observed in the actual game-state stepping:

| Interaction | Windows result | Decision |
| --- | --- | --- |
| SpecialN from idle / from the shield transition / after full shield drop | Projectile peak count 1 / 0 / 1; the action starts in each case. | Every supported entry into this projectile action emits once. Preserve the existing entry rules. |
| Later SpecialN fighter hitbox with the projectile isolated | At tick 8 in both facings: zero damage, 3 frames of hitlag on both fighters, 4 frames of hitstun, zero launch speed. | Kestrel's projectile action should not also inflict an invisible, zero-damage melee interruption. Remove this causal placeholder deliberately. |
| That zero-damage hitbox versus Dtilt | Rebound occurs at tick 8; no damage. | The removed placeholder must not clank either. |
| That hit with a controlled stale queue | Inserts SpecialN and changes the following Jab multiplier from 0.56 to 0.64. | Removing the placeholder must remove this unintended queue manipulation. |
| Kestrel live grab → ThrowF/ThrowB, both facings | Forward throw X velocity ±2.910; back throw X velocity ±3.065, both matching the attacker's original facing. | Back throw must reverse field position and launch behind the thrower. |
| Frontal projectile during freshly raised shield, both facings | Adds 5 percent, 4 victim hitlag, no shield/powershield effect; shot is consumed. Isolated control confirms the shield input raises Shield. | Projectiles must follow the established frontal shield/powershield defense rules. |

Two limitations must be corrected in the regression work. The diagnostic calls one shield case `established`, but uses separation 40, firing on tick 0 and collision on tick 1; raising shield on tick 0 is still inside the powershield window. Warm up that shield for at least 10 ticks before firing and assert its actual age at impact. The all-character ThrowB/Bair matrix directly calls and repeats the formula chain; only Kestrel's throws were also exercised end to end. Add actual combat cases for Boulder/Viper and actual clean/late Bair connections before treating that full matrix as reproduced. The Fsmash comparison's final-state output alone also does not prove the exact instant of clank suppression; inspect the collision tick if retaining that diagnostic claim.

## Required implementation

1. **Shield interaction.** Use existing frontal shield/powershield semantics for projectile collision. A normal block consumes the shot exactly once, causes the existing shield health/stun/push response and shield feedback, and adds no body damage or body launch. Powershield avoids additional shield damage/stun; distinguish ordinary holding drain in the assertion. Keep backside and unshielded control cases. Derive approach from the projectile, not the owner's later position/facing. No reflection or new defensive mechanic is requested.
2. **Emission on supported entry.** Fix the shield entry path to request one projectile just as idle entry does. Test a released-to-pressed SPECIAL edge, held input afterward, both facings, and a full shield-drop control. Do not add new cancel options or automatic repeated shots.
3. **Owner hitlag preservation.** The current projectile routine applies a normal hit and then assigns the owner's hitlag to zero. Add a focused baseline/regression case where the owner already has unrelated hitlag; a distant connection must neither add projectile-owner freeze nor erase that existing freeze. Preserve other owner state, including facing. This concern is source-derived until that additional case runs.
4. **Remove Kestrel's zero-damage melee placeholder.** Represent the absence of melee collision explicitly for Kestrel SpecialN. Preserve current projectile spawn, flight, damage and the action's total commitment in this bug-fix pass. Do not globally treat zero damage as noncolliding, or change other characters' move roles without equivalent evidence. The isolated old tick-8 scenario must produce no melee hitlag, hitstun, clank, stale entry or hit effect. Update data/export semantics, animation event selection, fixtures and evidence requirements together so they reflect the new actual event structure.
5. **Rearward launch.** Give ThrowB and Bair deliberate rearward launch semantics, after exercising their real current collisions. Preserve knockback magnitude, vertical behavior, Sakurai-angle resolution and DI. Use explicit move direction or deliberate move parameters; do not infer every move's launch direction from the sign of a hitbox offset. Cover Kestrel, Boulder and Viper, both facings; Bair clean and late; neutral DI plus forward/up/down control moves. Preserve unaffected moves.

## Presentation and verification

After the placeholder removal, SpecialN's animation/evidence contract should require the actual projectile release and its compact recoil. The palm relates to the real spawn origin, not the projectile's later flight. The old tick-8 fighter-hitbox supplements remain historical pre-change evidence; stop demanding a nonexistent contact only when the simulation change and regression establish that it no longer exists. Do not waive an event while it is still causal.

Retain the [original discharge direction](kestrel-gameplay-direction.md): palm-born cyan-white angular energy, braced off hand, short recoil, clear interruption/approach-pressure role. Range/recovery rebalance is a later measured choice, not part of these bug fixes. Ground/air hit, block and whiff sequences should report return to dash/jump/guard and defender recovery before balancing.

Deliver exact source changes and regression tests with before/after output. Run the new behavior assertions against the preserved baseline to demonstrate the expected failures, then against the fix. Check formatter, warnings-denied Clippy, feature coverage, deterministic replay and rollback checks; regenerate actual event/contact exports and matching production-renderer images. Keep the low-sweep visual patch separate, preserve current main work and report unrun Windows/controller or full gameplay QA honestly.
