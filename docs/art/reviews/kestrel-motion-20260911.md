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

## Additional frame review after gameplay integration

Historical source `882cac54644a131a3716aa379007cb8ffa58e3e4`, using its actual lossless frame sequences and labelled tracking sheets. This extends the earlier record; it is not a full-source art approval.

Inspected every displayed frame of dtilt, utilt, ftilt, uair and dair in both facings, uncharged fsmash in both facings (42 frames each), ThrowF and ThrowU in both facings (36 frames per sequence), ThrowD in both facings (39 frames each), fully charged fsmash in both facings (102 frames each), negative-facing jab (19 frames) and negative-facing up special (39 frames).

- Dtilt chambers its knee, extends at a lower height from a bent supporting leg, folds after activity and returns to guard. Ftilt uses a higher forward kick and a taller supporting stance. These remain visually distinct from the later corrected low-sweep smash.
- Utilt gathers briefly, raises the designated hand overhead for the complete four-tick active interval and lowers it once. The opposite arm stays near the chest; its shorter load differentiates it from the corrected rising upward smash.
- Uair shows the high kick through its actual active interval, then lowers and folds the leg while airborne; Dair extends down and forward with the other leg trailing before retracting. Both fixtures end in landing, so neither clip proves an uninterrupted airborne recovery after the captured landing transition.
- Uncharged fsmash winds its arm back and extends once into the active region, then retracts and holds guard in both facings. The earlier alleged contact gap remains withdrawn; these contact sheets are motion review, not a replacement for projected-surface contact measurements.
- Fully charged fsmash has a visible unload/reload on release: tick 63 / state frame 4 is fully loaded, tick 64 / state frame 5 drops the arm, state frames 6 through 8 load it again, and the strike reaches first activity at state frame 11. All 204 historical frames were inspected in eight paginated sheets. A fresh clean `4ddb103` Windows capture then reproduced the same boundary in both facings; contiguous ticks 61 through 71 were personally inspected. See the separate charge-release direction and measurements. No causal timing change is directed.
- ThrowF, ThrowU and ThrowD use different arm paths but still lack sufficiently distinct torso/leg effort in both facings. The held victim remains in front until the actual release. A physical throw presentation must respect that actual constraint rather than depicting a victim that was already carried overhead or behind the fighter. The local throw direction specifies distinct forward bracing, upward extension, downward compression and rearward rotation while retaining the real release path. ThrowD keeps the release arm extended for most of its recovery; require a deliberate single return into guard.
- Negative-facing jab is immediately active for its two actual active frames and settles once; no extra anticipation or timing change is required. Negative-facing up special raises the hand through actual activity, retracts while descending and reaches landing at the end of the captured state. Neither opposing view introduces a different motion issue; the landing still limits claims about uninterrupted aerial recovery.
- Current `4ddb103` wide down-special evidence was inspected in both facings. Its overlay and pulse obscure part of the arm silhouette. The earlier concern about a generic forward thrust is still an open visual question; do not claim it confirmed from this overlaid sheet alone.

The complete generated inventory still contains 45 sequences / 1,619 frames and is not the count of inspected or approved frames. The separate throw-down retry adds its missing facing. Remaining review concerns the source-changed low/up-smash and future SpecialN/charge/throw presentation, with final matching-source evidence required. This historical frame inspection does not establish real-time playback or interactive quality.

Direction: [charged release](../procedural/anim/kestrel-charge-release-presentation.md) and [four throws](../procedural/anim/kestrel-throw-presentation.md). Both are pending implementation; [compact discharge](../procedural/anim/kestrel-special-n-presentation.md) remains the next visual pass.


## Defense boundary checkpoint before compact discharge

Claude's shield correction `888fc01395f5bd2bc965228299f862039224b6a0` has passed the independent Windows regression (identical portable test: 7 pass / 5 fail before, 12 pass after), full 212-test suite, 205-test headless suite, 26 Python evidence tests, format/lint and release build. The matching 58-file production capture completed without skips; numerical contact/support audits passed. See the [implementation review](888fc01395f5bd2bc965228299f862039224b6a0.md) for provenance, behavior, measurement limits and pending remote CI.

This preserves surviving shield coverage during normal shieldstun and removes the free recovery powershield, while keeping broken shields vulnerable and genuine new raises able to parry. It changes no presentation. The current SpecialN wide and ThrowB primary captures were inspected again; compact discharge, charged-release continuity and physical throw presentation remain pending. The complete SpecialN implementation instruction has now been sent to the existing Claude Opus 5 Extra task as a separate visual delivery, with the four defense files frozen.


## 2026-09-12 — Compact Special N accepted incrementally

The [8e70585 Windows review](8e70585893f41ee534f224f60b4ebf540eb76994.md) records the completed compact discharge, exact source, 220 full-feature Rust tests, 213 no-GUI tests, 26 Python tests and clean release/captures. All 464 Windows frames across four cases, two facings and two cameras were personally inspected in tracking sheets, with original release/impact and wide diagnostic images reviewed separately. One release, brief recoil and return to guard are readable; the old large forward hold is gone. Full-air evidence covers the real 32-frame action and final actionable Air state. Existing causal rows and geometry remain unchanged.

Special N presentation is accepted as an incremental improvement. Kestrel's charge/throw work remains open. A separate Mesa perspective rendering defect does not appear in the same-source Windows/NVIDIA gamecam and is under diagnosis; no platform-wide visual pass is inferred. The user's 2026-09-12 ownership update reserves later fighter animations for the material they will supply unless explicitly requesting further work.
