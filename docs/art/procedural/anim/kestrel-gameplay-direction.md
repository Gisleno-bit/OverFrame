# Kestrel Alpha 0.7 — gameplay review direction

2026-09-11. Baseline source: `91f9c5a2834353bbb7207dcc4d866a4e513fd48f`, unchanged through the art-only `4c46c7b514dcfc5a537bd981c83c0f91bff7ed39`. These are design intentions and source observations, not playtest approval. Complete the current animation handoff and inspect actual images before implementing the follow-up below.

## Function and original expression

The roles below are inferred from OVERFRAME's own attack windows, offsets, launch angles and recovery in `src/sim/attacks.rs`. They are hypotheses to test against movement, DI, shielding and stage position; a listed combo/finisher role is not a claim of a guaranteed combo or kill percentage.

| Action | Intended use and commitment | Physical expression to inspect |
| --- | --- | --- |
| jab | Immediate close-range interruption; short reach prevents it replacing spacing tools. | Brief right-hand snap from a tight guard; no added anticipation frame. |
| jab2 | Confirmed close-range follow-up and small upward conversion. | Left-hand cross with the first hand withdrawing; clearly a continuation. |
| ftilt | Deliberate horizontal spacing poke with visible recovery. | Knee chambers, sole extends, knee folds before the leg lowers. |
| utilt | Anti-air and vertical combo-start hypothesis. | Compact waist-to-overhead uppercut; upright body, protected off hand. |
| dtilt | Low horizontal poke, checking grounded approaches. | Support knee folds while the other sole skims forward. |
| dash_attack | Commit movement to catching approach/retreat and lifting the opponent. | Bent leading forearm and running diagonal, driven by actual displacement. |
| fsmash | Committed horizontal finisher/read. | Shoulder coil into a palm drive, braced support and forceful recoil. |
| usmash | Vertical finisher/anti-air read; late hit has weaker output. | Low compression into an upward palm, held until real activity ends. |
| dsmash | Low forward finisher/roll read; current geometry has no rear hit. | Low forward heel sweep with a distinct planted support leg. |
| nair | Lingering airborne interception/pressure; landing and L-cancel govern risk. | Split leg silhouette with the contact sole extended through the late interval. |
| fair | Forward aerial spacing and positional conversion. | Rising forward kick, counterlean, deliberate knee-first recovery. |
| bair | Rearward aerial spacing and stronger interception. | Heel cuts backward while chest balances forward; simulation facing is preserved. |
| uair | Vertical juggle continuation/anti-air. | Torso reclines to clear the upward leg from the crest. |
| dair | Downward challenge/edgeguard with substantial whiff and landing commitment. | Compact upper body, single descending heel and folded other knee. |
| special_n | Original ranged interruption and approach-pressure tool, with explicit defensive answers. | Palm-generated energy discharge, asymmetric brace, compact recoil; no weapon imitation. |
| special_side | Committed horizontal displacement and strike. | Low drive with a trailing off hand; existing travel remains authoritative. |
| special_up | Recovery trajectory plus an ascending defensive hit. | Palm leads the actual ascent, legs trail asymmetrically. |
| special_down | Immediate close-range electric interruption with significant recovery. | Short body-centered pulse, elbows opening around the chest; no fictitious limb reach. |
| throw_f | Forward positional release to pursue or establish edge pressure. | Two-hand hold transitions into leading right-palm release. |
| throw_b | Reverse field position and threaten the nearby edge. | Chest twists, head looks back and left hand leads the actual victim release. |
| throw_u | Launch above for a vertical chase; DI and weight matter. | Two-hand lift with the head clear of the release silhouette. |
| throw_d | Low release into a modest vertical chase/reset hypothesis. | Hip/knee fold toward the real target position, rise after release. |

## Neutral-special: observations requiring runtime reproduction

`fighter.rs::try_start_attack` requests a projectile immediately on entering SpecialN. `state.rs` spawns it at root + facing × 14 and half character height, then integrates it in the same tick. For Kestrel this yields the measured first after-step center at `[21, 15]`, distinct from the emission origin `[14, 15]`. Current global projectile constants are speed 7, radius 4 and lifetime 60. The existing attack also exposes its separate zero-damage, radius-2 fighter hitbox at state frame 9. These are current implementation facts, not desired new balancing values.

Three source-level concerns need minimal runtime tests before any design change:

1. `update_projectiles` appears to call `apply_hit` directly after capsule collision without the shielding branch used in `resolve_combat`. Test a frontal projectile against an established full shield and a fresh powershield, both facings. Record percent, state, shield health, hitlag, owner hitlag and projectile lifetime. Determine whether it bypasses shielding rather than assuming it is blockable.
2. `tick_shield` enters SpecialN when SPECIAL is pressed but does not visibly request `spawn_projectile`, unlike `try_start_attack`. Test grounded neutral SPECIAL from idle versus held shield using the same input edge. Establish whether the shield transition produces the animation without a discharge.
3. The later zero-damage fighter hitbox still reaches the normal hit/clank pipeline. Test it against a nearby victim and an opposing active attack, with the emitted projectile independently kept out of that collision. Record hitlag, state/velocity, clank, stale queue and effect events. Zero damage alone does not establish absence of gameplay effects.

Keep these follow-up results separate from the animation regression. The current animation delivery must continue to represent both real events faithfully. If gameplay intentionally removes or changes an event later, update simulation tests, direction and capture requirements together, preserving the earlier evidence rather than redefining it as a pass.

## Original discharge direction to prototype after the baseline is confirmed

Use a compact, palm-born cyan-white angular pulse with a short taper and restrained electric impact; the off hand braces below the chest and the torso gives a small recoil. The projectile should read as Kestrel's sharp interruption tool in ground and aerial play, with clear shielding/jumping answers and a punishable commitment. Preserve immediate responsiveness unless measured gameplay justifies a timing change. Do not add a large beam, stun-lock system, resource meter or copied firing animation.

Evaluate grounded and short-hop use at close, medium and long separation; on hit, block and whiff; with both facings; against all three Alpha archetypes. Measure the time to resume dash/jump/guard relative to defender recovery. Decide range, flight and recovery from these results. No external move table is a target to match, and no timing/balance change is approved by this document alone.

The reference role is disruption of approaches and positional pressure through a projectile that causes a reaction, as described by [SSBWiki's Falco neutral-special overview](https://www.ssbwiki.com/Falco_(SSBM)/Neutral_special). [FightCore's Falco page](https://fightcore.gg/characters/218/falco/) provides separate ground/air frame-data context. These were consulted for function and commitment, not copied numbers. The prior [Rivals/Ranno pose-family study](kestrel-reference-study.md) is retained; Dragdown's direct fetch failed during this follow-up, so no new access is claimed. The supplied `melee-master.zip` was identified as a decompilation reference from its README only; no code or assets were imported.
