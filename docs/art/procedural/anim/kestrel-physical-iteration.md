# Kestrel: low sweep and upward release

Directed on 2026-09-11 after the Windows `882cac54644a131a3716aa379007cb8ffa58e3e4` pose/phase review. Claude implements this in `src/model/anim_directed.rs`; the source of authority is the current Alpha 0.7 product direction and [checkpoint review](../../reviews/882cac54644a131a3716aa379007cb8ffa58e3e4.md).

## LowSweep / dsmash

The current low smash shares the horizontal forward-kick silhouette with dtilt. Make dsmash a low heel sweep: sit the pelvis over a bent support knee, extend the attacking leg diagonally toward the floor and counterbalance with the torso/free arm. The sweep's windup and recovery should differ from dtilt's compact knee chamber, extension and return.

Choose an authored contact target inside the real active region rather than always its center. For the existing center `(20, 13)` and radius `10`, a point around `(14, 6)` is a geometric starting hypothesis, not a required final constant. Derive reach from the real skeleton, measure the rendered foot and solve without changing bones, capsule, model scale, frame data or hitboxes. If that candidate cannot produce safe support and real contact, choose another feasible low target inside the same region and explain it with measurements.

## UpperSmash / usmash

Move the deep leg load into startup and held charge. Extend the knees and raise the pelvis into the first active upward strike; keep the striking arm reaching the actual region and the support feet above the floor. The root of the simulation must not jump to fake the rise. Avoid maximum compression persisting through the active interval. Utilt retains a smaller, upright anti-air silhouette.

Any regression that enforces the old maximum active crouch is a test of an outdated visual decision. Replace it with a meaningful compression-to-extension assertion plus the existing strict actual contact/support checks. Do not relax the contact acceptance or skip inconvenient active frames.

## Delivery and acceptance

- Separate visual patch after the gameplay-baseline diagnostic archive. SpecialN gameplay and pose remain pending a subsequent decision.
- Both facings, every active/late tick: designated actual mesh contact is finite and nonpositive; both feet have valid support measurements; no change to causal state or fighter dimensions.
- Continuous anticipation, contact, follow-through and recovery, with no second strike during recovery.
- Matching primary/wide/support images and lossless 60 Hz PNG sequences/sidecars. Inspect the whole transition, not just the best active frame.
- Formatter, Clippy, meaningful regression tests, runtime fixture comparison, fresh source archive and hashes. Numerical success remains separate from visual approval.
