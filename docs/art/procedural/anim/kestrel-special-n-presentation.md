# Kestrel: compact discharge after the gameplay correction

Direction, 2026-09-11. Apply after gameplay candidate `4ddb10315b936f8b70e5957a5adcd986e8039e9c` is integrated. Its identical portable test reproduces 12 behavioral failures in the old simulation and passes all 20 tests with the fixes in independent Windows builds. This direction changes presentation; it does not authorize a different projectile spawn time, origin, flight, damage or action commitment.

## Observed baseline

The matching Windows `882cac54644a131a3716aa379007cb8ffa58e3e4` motion sequence currently fires immediately, then winds into a second, extended punch at state frame 9 to serve the real later melee event. The sampled motion sheet is useful for comparing silhouettes but is not a full-rate playback approval. Once that second event is deliberately removed, its second punch and late reach correction should also disappear.

The actual release origin is local `(14,15)` for Kestrel; the first after-step projectile is already at `(21,15)`. A palm should relate to the origin. Chasing the already moving projectile makes the torso reach too far and misrepresents the release. The direction is a short palm-born electrical interruption, not a broad thrust or an imitation weapon.

The subsequent Windows gameplay WIP `d764aaa7` has the corrected single event: all 68 frames across both facings were inspected. Its evaluator is byte-identical to revision 2. Removing the event changes the rendered animation even without a new pose family; do not describe the output as unchanged. The second punch is gone, but the large forward lean and extended hold remain through approximately state frame 12. The projectile is still a yellow orb. These are the next visual changes, not completed work.

The motion labels still call the actual spawn tick `windup` and the old state-frame-9 marker `active`, even though no fighter hitbox exists. Correct these presentation labels from actual emission/recoil/recovery events while retaining state-frame values and all causal commitment. Preserve correct active labels for actions that genuinely have fighter hitboxes.

## Physical sequence

- **Immediate release:** snap from the existing guard to a bent-elbow palm release at the actual origin. Preserve the responsive first-tick discharge. Do not label a later preparatory pose as anticipation for an event that already happened. If the present skeleton cannot reach that point with a compact torso, report the measured geometric limitation before proposing any causal change.
- **Recoil:** withdraw the palm and elbow briefly toward the ribs after emission, with a small opposing shoulder motion. The left hand braces below the sternum; it must remain visually separate from the firing forearm. Keep the head looking along the discharge and the grounded support stable. Test a compact torso as the first candidate rather than retaining the old large lean assist needed for the now removed melee hit.
- **Recovery:** settle once from recoil into guard. Preserve the runtime lockout even if the visible hand settles earlier. Do not produce another forward punch at the old state-frame-9 marker or use a second pose peak that suggests a second shot. Ground and air share the upper-body identity; legs follow their actual grounded/airborne state.
- **Energy:** a small cyan-white angular pulse with a short taper and restrained release/impact flash. The shape should clearly leave the palm. Brightness and cosmetic extent must not suggest a large beam or a larger damaging region. Identify Kestrel-owned presentation without silently redesigning Boulder/Viper projectiles. Cosmetic lifetimes must remain outside causal simulation/rollback state.

## Acceptance

Capture actual ground and aerial release, recoil and return to action in both facings, including neutral entry and the repaired shield entry. Provide continuous lossless 60 Hz frames and honest event labels. Show the emission origin separately from the first integrated projectile position. Validate the palm against the real origin at release; stop requiring the old fighter contact only after the gameplay regression establishes its removal. Preserve contact checks for every other move.

Inspect at the fixed contact camera and the actual gameplay camera: readable palm/forearm separation, a single release, small recoil, grounded feet above the floor, no artificial root drift and no movement of simulation geometry. Compare hit/block/whiff at close, middle and far spacing against all three Alpha fighters before any later range/recovery balancing decision. Full Kestrel approval still requires the rest of the move review.
