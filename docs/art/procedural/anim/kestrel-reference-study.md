# Kestrel movement-reference study — 2026-09-10

Scope: visual reference study for the 22 runtime actions, not approval of the implementation. No reference-game assets, combat values, exact animation curves or rigs are imported.

Sources opened in the browser: [FightCore](https://www.fightcore.gg/characters/224/fox/), [RoA2 move imagery and descriptions](https://dragdown.wiki/wiki/RoA2/Ranno), and [RoA2 timing terminology](https://dragdown.wiki/wiki/RoA2/Frame_Data). Direct fetching of Dragdown returned 403; the browser loaded it successfully. RoA2 imagery was inspected by move section, including all four throws. FightCore provided additional jab, tilt, strong, projectile, dash-special and radial-effect comparisons. This is a pose-family study, not a measurement of every frame in either reference game.

| OVERFRAME action | Study focus | Kestrel decision |
|---|---|---|
| jab | Short lead-hand action | Direct right fist; immediate runtime contact |
| jab2 | Alternating follow-up | Left crossing fist, right retracts |
| ftilt | Kick/guard separation | Knee then sole, asymmetric guard |
| utilt | Upward silhouette | Upright uppercut, no handstand |
| dtilt | Low support shape | One bent support knee, one low sole |
| dash_attack | Forward mass | Leading forearm and compact running diagonal |
| fsmash | Palm and body weight | One leading palm, no copied effect |
| usmash | Overhead contact | Palm above crest, other hand low |
| dsmash | Low spread | One forward sweep; no invented rear hit |
| nair | Airborne leg separation | Leading sole and folded rear knee |
| fair | Forward kick line | One forward rising kick |
| bair | Rear contact | Heel backward; preserve runtime facing |
| uair | Overhead leg | Tucked torso, one upward foot |
| dair | Downward leg | One heel drop; no borrowed multi-hit behavior |
| special_n | Emission versus melee | Palm release at the existing projectile event |
| special_side | Displacement readability | Existing lateral drive, no grab/tongue mechanic |
| special_up | Ascent line | Palm-led existing trajectory |
| special_down | Body versus effect | Immediate centered pulse, no bubble mechanic |
| throw_f | Hold/release distinction | Forward palm release |
| throw_b | Rearward target line | Chest twist and rear hand release |
| throw_u | Target above body | Hands lift; preserve actual target location |
| throw_d | Low release | Hips fold toward actual release point |

The accompanying JSON gives limb, arc, torso, other hand, authored anticipation fraction and recovery for every action. Fractions are visual choices within available startup, not copied timing. Runtime export controls all active windows, including zero-startup actions, hitlag and late-active phases. Measured contact diagnostics must settle feasibility before any pass verdict.

## Implementation questions identified in the baseline source

In `src/model/anim.rs`, `attack` computes its aim with `hb.x.max(0.01)`. The rearward offset is therefore lost for attacks such as back air. It also computes the direction from the hitbox offset without solving from the transformed shoulder/hip origin. These are candidates for the measured contact gaps, not approval of a guessed angle correction.

The same evaluator switches to recovery after `startup + active`, while the exported move data also includes `late_active`. Claude should verify the full real active interval with the new diagnostics before implementing the directions. Preserve simulation values; correct rendering only.
