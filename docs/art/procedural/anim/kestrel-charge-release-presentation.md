# Kestrel: continuous release from a charged forward smash

Direction, 2026-09-11. This is a presentation correction, separate from the repeated-impact shield diagnostic. Claude implements it after the shield work is reviewed. Preserve the compact neutral-special direction as a separate, reviewable change.

## Confirmed on the current implementation

The complete historical `882cac54644a131a3716aa379007cb8ffa58e3e4` charged sequence was inspected in both facings (102 actual frames per facing). The suspected release discontinuity was then reproduced with a fresh Windows capture from clean implementation `4ddb10315b936f8b70e5957a5adcd986e8039e9c`, executable SHA-256 `c25988cf63a9885e9b65876b81f6a7a3a9e1153151a012cff35d0a722e4604a2`.

Local proof: `work/game-evidence/windows-4ddb103-charge-motion`, with contiguous tick 61 through 71 inspection sheets and measurements in `work/game-evidence/windows-4ddb103-charge-boundary`. Both facing sheets were inspected. These are actual replay frames, not invented pose samples.

At the end of the full hold, tick 63 / state frame 4 shows the fully loaded arm. At tick 64 / state frame 5 the arm visibly drops back into an earlier preparation. It rises again over state frames 6 through 8, then drives toward the first active frame at tick 70 / state frame 11. The action visibly prepares twice after charging.

The positive-facing right-hand centroid falls from Y 22.047455 at tick 63 to Y 18.605017 at tick 64, then rises to Y 21.924208 at tick 67 before the strike. The negative-facing sequence repeats the same vertical motion. This identifies the observed discontinuity; the underlying phase-selection cause still needs source verification. Charge vibration alone is not the reported defect.

## Required motion

Keep the loaded body configuration at the hold/release boundary and release from it into one continuous drive toward the real first active frame. The shoulders, elbow and hand must not visibly unload into an earlier anticipation and then load again. Preserve a readable loaded silhouette throughout the hold, controlled charge vibration and the existing actual contact alignment during activity. Follow through and recover once.

Do not change charge duration, charge amount, state-frame progression, input release, startup, active/recovery timing, damage, launch, fighter roots, hitboxes, capsule dimensions, skeleton lengths or rollback state to make the interpolation easier. Avoid a blanket smoother that erases the sharp active pose or alters unrelated attacks. Determine whether the uncharged path shares the faulty boundary before changing it; the uncharged sequence currently has one readable preparation and strike.

## Acceptance evidence

- Verify the actual evaluator and live charge transition; explain the causal presentation boundary in the handoff.
- Add a focused regression that exercises hold and release through the real simulation and catches this visual return to preparation. Show that the old evaluator fails the same regression. A test that only restates new constants is insufficient.
- Supply continuous lossless frames for uncharged, partial-charge release and full-charge release in both facings. Include the last held frames, first released frame, first active frame and recovery with real tick/state-frame labels.
- Compare all relevant simulation exports before/after, excluding source SHA only. Preserve actual active contact, ground support and the corrected low/up-smash presentation.
- Inspect the result at the contact camera and gameplay camera. Passing a centroid bound alone is not visual approval.

This correction does not approve the full Kestrel move set or Alpha 0.7.
