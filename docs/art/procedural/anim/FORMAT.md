# Animation direction contract v1

The JSON files in this directory are authored visual directions, not simulation tables. `kestrel.json` is the first delivery. Claude may translate prose into the existing procedural evaluator, but must validate the complete source file and identify its SHA-256 in implementation evidence. Reading a direction file does not make its implementation verified.

## Exact fields

Unknown fields are authoring errors at every object level. Do not silently ignore misspellings or use an arbitrary catch-all map.

- Top level: `schema_version`, `character_id`, `status`, `source_sha`, `evidence_commit`, `runtime_actions`, `intent`, `timing_contract`, `geometry_contract`, `reference_review`, `reference_audit_status`, `actions`.
- `schema_version` is integer 1. Character identity is the matching procedural model ID. Source and evidence SHAs refer to the reviewed baseline, not to a self-referential future commit.
- `timing_contract`: `authority`, `anticipation_fraction`, `active`, `recovery`, `variants`; all nonempty strings.
- `geometry_contract`: `units`, `contact`, `special_cases`, `extras`; all nonempty strings.
- Each `reference_review` entry: `url`, `accessed`, `method`; strings describing evidence actually accessed. This metadata is not game content.
- Each `actions` entry: `action_id`, `contact_bone`, `contact_piece_id`, `pose_family`, `anticipation_fraction`, `arc`, `torso`, `other_hand`, `recovery`.
- Action names are unique and match the full runtime-exported action set for that fighter. Kestrel currently exports 22. Never infer coverage from the length alone.
- Contact bone and piece must exist in the matching model; the named piece must belong to that bone. Other field values are nonempty strings, except `anticipation_fraction`, a finite number from 0 through 1.
- `pose_family` identifies this author's intended pose. It is not permission to fall back silently to a default if a family is unimplemented. An unsupported family must fail validation or remain explicitly pending.

## Implementation and evidence

The prose specifies limb, arc, torso and counterbalancing hand. Any translation to angles or offsets must be checked against transformed contact geometry. Record the translation in the implementation and tests; do not derive damage, active timing, displacement or collision dimensions from this file. The fraction sets the anticipation peak within available inactive startup, without creating a new inactive tick.

Aim from the actual transformed joint/contact geometry. Keep rearward signs and facing, and cover all real clean/late active ticks. Preserve the existing hitlag and state-transition rules. A close-enough bone origin is not necessarily an intersecting hand/foot piece; measure the actual designated contact geometry.

Projectile, radial-effect and throw actions use their runtime emission, effect center or victim-release event. They must not gain fictitious ordinary hitboxes to satisfy a generic contact assertion. The manifest distinguishes these cases.

Tests should validate the source contract and behavior at the actual failure boundaries: first contact, later active ticks, both facings, held charge, jab chaining and hitlag. Captures remain required for silhouette, readable pose and capsule review. No animation capability is artistically approved until the reviewer records a pass against the same source SHA's `game3d` evidence.


## Mixed projectile and fighter-hitbox actions

An exception names an event to review; it does not erase another real runtime hitbox. Kestrel special_n currently emits a projectile at after_step tick 0/state_frame 1 in the declared fixture and also exposes a fighter hitbox at tick 8/state_frame 9 (center [16,21], radius 2 in the reviewed positive-facing case). These are exported observations, not new combat parameters. Review the hand at emission and at every actual fighter-hitbox tick. Preserve projectile flight as a ranged exception: the hand does not have to follow the projectile in flight.

If the standard projectile sheet omits the later fighter-hitbox event, add a clearly named supplemental view using the existing declared contact cameras and event metadata. Do not silently replace the before/spawn/flight/after cells, fabricate a hitbox, suppress the real one, or mark a positive measured fighter-contact gap as verified solely because the move also fires a projectile. Report an unreachable constraint for an art decision.
