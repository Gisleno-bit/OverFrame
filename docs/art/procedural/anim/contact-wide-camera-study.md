# Contact diagnostic camera study

Status: camera specification based on a local **uncommitted candidate**, not an art approval or published-SHA review.

The first Claude evidence batch, built locally with the art tree at `7d5bedf68291bb868464c3c160bb3c08fa3b6408`, was captured as `7d5bedf-UNCOMMITTED-CANDIDATE` on Windows/NVIDIA. It produced 57 images with no skipped output. The initial wide height 88 and anchor Y +22 clipped the rising special's last active hitbox. Neutral air's long active interval descends outside that same fixed view, and the projectile flight cell also exceeds its horizontal bounds.

Across the 23 case variants and both facings, measured capsule, contact-piece and hit-region bounds relative to the anchor are:

| Extent | Game units | Limiting sample |
|---|---:|---|
| Minimum facing-relative X | -28.1900 | back air, last active |
| Maximum facing-relative X | 60.0000 | neutral special, projectile in flight |
| Minimum Y | -52.9413 | neutral air, first after last active |
| Maximum Y | 71.2050 | up special, last active |

The camera is therefore revised to common orthographic height **176**, X offset **10 × facing**, Y offset **24**, with the same 512² cells and fixed anchor rule. Its vertical bounds are -64 to +112 relative to the anchor. Reserving the first 90 pixels for diagnostic captions leaves visible geometry up to +81.0625. This contains the measured limits with margin; it is not a per-pose auto-fit. The primary three-cell camera remains unchanged for detailed comparisons.

The measurements above do not include the complete mesh of every fighter or a thrown opponent after it leaves the review region. They establish a better fixed camera, not a proof of all future framing. The corrected implementation must record full drawn-model bounds and clipping of required evidence, keep captions readable, and be visually checked again after pose changes. Off-screen victims after release must not be misrepresented as missing throw execution. An off-screen required contact region remains unresolved evidence.

The original first-batch nearest-vertex distance is insufficient to prove positive mesh separation. No pose correction or geometry approval is based on that approximation. Likewise, a small filled attack effect is not the boundary of the simulation's hitbox: the measured full-radius wire overlay must determine contact review.
