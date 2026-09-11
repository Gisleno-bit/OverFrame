# Continued Kestrel motion inspection, 2026-09-11

Historical source: `882cac54644a131a3716aa379007cb8ffa58e3e4`. The original PNG sequences were converted to tracking contact sheets without changing the original fixed-camera captures. The new `windows-882cac5-all-frames` inventory contains 45 sequences / 1,619 frames; this is a generated-artifact count, not a claim that all have been inspected. The separate throw-down retry remains necessary for the missing facing. Current low/up review uses b9/main15 instead.

Actually inspected here, every displayed frame: Bair, Nair and dash attack in both facings; Jab positive; Jab2 in both facings; side special, Fair and down special in both facings; up special and back throw positive. Prior sampled-pose review remains separate.

- Bair holds the rearward kick through actual clean/late activity, then folds the knee before landing. It does not recover prematurely. Its actual launch correction remains in the separate gameplay delivery.
- Nair preserves the extended strike through the last captured active tick and then lands. This fixture does not show a complete airborne recovery; do not call its 27 frames a full uninterrupted move cycle.
- Dash attack has a bent leading forearm and braced front leg, then retracts the arm and returns to guard once. Side special has its own longer coil and straighter palm extension. Both return without a second strike.
- Jab is immediately active and settles once. Jab2 shows the actual chained connection and repeated state-frame 2 during hitlag. The opponent occludes much of the hand in these combat views, so the separate contact geometry/wide evidence remains necessary for the strike-piece review.
- Fair chambers, extends once and folds, then lands. Up special rises with its arm before retracting during descent; the eventual landing ends the captured move state.
- Down special is an immediate body-centered pulse. The frontal arm silhouette still reads close to a short forward thrust in this camera; review its wide/opposite-facing images before closing its visual identity. No new implementation request is based on this single view yet.
- The old back-throw sequence visibly sends the target forward after release, consistent with the baseline bug. Its new gameplay and motion must be reviewed together after the fix; the old clip is not an acceptance artifact.

These are frame-by-frame fixture observations, not real-time playback, full-player interaction or full Kestrel approval.

## Gameplay correction WIP d764aaa7

Both full back-throw sequences (82 frames) now visibly send the victim behind Kestrel, crossing his position after release. The frame-by-frame review confirms the trajectory fix, but the body action still lacks the specified clear backward torso twist and glance. The held opponent stays in front until causal release, then travels across Kestrel. A future presentation pass must respect that actual target path rather than visually teleport it behind him. This is separate from accepting the causal launch fix.

Both SpecialN sequences (68 frames) now contain one release and one return, with no second punch at state frame 9. The long forward lean and hand hold remain; the projectile is still yellow. The compact discharge direction remains pending. Captions naming spawn `windup` and the former melee tick `active` need event-aware presentation labels. These observations apply to revision 2 as its animation/capture source is byte-identical; the changed export method prose is separately checked.
