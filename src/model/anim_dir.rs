//! Authored animation directions (`docs/art/procedural/anim/<character>.json`,
//! contract `docs/art/procedural/anim/FORMAT.md`).
//!
//! A direction file is prose per runtime action — limb, arc, torso,
//! counterbalancing hand, anticipation fraction, recovery — plus the exact
//! contact bone and piece the measurement must use. It is *not* a
//! simulation table: nothing here changes damage, timing, hitboxes or
//! geometry. The file is embedded, parsed strictly (an unknown field at any
//! level is an authoring error), validated against the built model and the
//! runtime action set, and identified by its SHA-256 in every export that
//! depends on it.
//!
//! The translation of the prose into poses lives in
//! [`super::anim_directed`]; this module only owns the contract.

use super::characters::CharacterModel;
use super::math3::{v3, V3};
use crate::sim::attacks::{self, MoveId};
use serde::Deserialize;

pub const KESTREL_ANIM_JSON: &str = include_str!("../../docs/art/procedural/anim/kestrel.json");

/// Every shipped direction file, by character id.
pub const ALL_DIRECTIONS: [(&str, &str); 1] = [("kestrel", KESTREL_ANIM_JSON)];

// ----------------------------------------------------------------- schema

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DirectionFile {
    pub schema_version: u32,
    pub character_id: String,
    pub status: String,
    pub source_sha: String,
    pub evidence_commit: String,
    pub runtime_actions: String,
    pub intent: String,
    pub timing_contract: TimingContract,
    pub geometry_contract: GeometryContract,
    pub reference_review: Vec<ReferenceReview>,
    pub reference_audit_status: String,
    pub actions: Vec<ActionDirection>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TimingContract {
    pub authority: String,
    pub anticipation_fraction: String,
    pub active: String,
    pub recovery: String,
    pub variants: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GeometryContract {
    pub units: String,
    pub contact: String,
    pub special_cases: String,
    pub extras: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ReferenceReview {
    pub url: String,
    pub accessed: String,
    pub method: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ActionDirection {
    pub action_id: String,
    pub contact_bone: String,
    pub contact_piece_id: String,
    pub pose_family: String,
    pub anticipation_fraction: f32,
    pub arc: String,
    pub torso: String,
    pub other_hand: String,
    pub recovery: String,
}

/// The pose families the evaluator implements. A family named in a
/// direction file that is not listed here fails validation — there is no
/// silent fallback to a generic aim (FORMAT.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoseFamily {
    Punch,
    Kick,
    Uppercut,
    LowKick,
    PalmDrive,
    OverheadDrive,
    LowSweep,
    RunningForearm,
    SplitKick,
    RisingKick,
    BackKick,
    OverheadKick,
    HeelDrop,
    ProjectileRelease,
    RisingDrive,
    LateralDrive,
    RadialPulse,
    ThrowForward,
    ThrowBackward,
    ThrowUpward,
    ThrowDownward,
}

impl PoseFamily {
    pub fn parse(s: &str) -> Option<PoseFamily> {
        use PoseFamily::*;
        Some(match s {
            "punch" => Punch,
            "kick" => Kick,
            "uppercut" => Uppercut,
            "low_kick" => LowKick,
            "palm_drive" => PalmDrive,
            "overhead_drive" => OverheadDrive,
            "low_sweep" => LowSweep,
            "running_forearm" => RunningForearm,
            "split_kick" => SplitKick,
            "rising_kick" => RisingKick,
            "back_kick" => BackKick,
            "overhead_kick" => OverheadKick,
            "heel_drop" => HeelDrop,
            "projectile_release" => ProjectileRelease,
            "rising_drive" => RisingDrive,
            "lateral_drive" => LateralDrive,
            "radial_pulse" => RadialPulse,
            "throw_forward" => ThrowForward,
            "throw_backward" => ThrowBackward,
            "throw_upward" => ThrowUpward,
            "throw_downward" => ThrowDownward,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        use PoseFamily::*;
        match self {
            Punch => "punch",
            Kick => "kick",
            Uppercut => "uppercut",
            LowKick => "low_kick",
            PalmDrive => "palm_drive",
            OverheadDrive => "overhead_drive",
            LowSweep => "low_sweep",
            RunningForearm => "running_forearm",
            SplitKick => "split_kick",
            RisingKick => "rising_kick",
            BackKick => "back_kick",
            OverheadKick => "overhead_kick",
            HeelDrop => "heel_drop",
            ProjectileRelease => "projectile_release",
            RisingDrive => "rising_drive",
            LateralDrive => "lateral_drive",
            RadialPulse => "radial_pulse",
            ThrowForward => "throw_forward",
            ThrowBackward => "throw_backward",
            ThrowUpward => "throw_upward",
            ThrowDownward => "throw_downward",
        }
    }
}

// ----------------------------------------------------------------- resolved

/// One action's direction resolved against the built model.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub id: MoveId,
    pub action_id: &'static str,
    pub family: PoseFamily,
    pub fraction: f32,
    /// Contact bone (rig index) and the exact piece (index into
    /// `CharacterModel::pieces`) the measurement uses.
    pub bone: usize,
    pub piece: usize,
    pub piece_id: String,
    /// Centroid of the piece's vertices in the contact bone's frame — the
    /// point the solver drives to the runtime target.
    pub effector: V3,
    pub arc: String,
    pub torso: String,
    pub other_hand: String,
    pub recovery: String,
}

/// A validated direction file bound to a model.
#[derive(Clone, Debug)]
pub struct Directions {
    pub character_id: String,
    pub status: String,
    pub source_sha: String,
    pub evidence_commit: String,
    /// SHA-256 of the embedded JSON text, as evidence identifies it.
    pub sha256: String,
    pub actions: Vec<Resolved>,
}

impl Directions {
    pub fn get(&self, id: MoveId) -> Option<&Resolved> {
        self.actions.iter().find(|a| a.id == id)
    }
}

pub fn parse(json: &str) -> Result<DirectionFile, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

fn nonempty(s: &str, what: &str) -> Result<(), String> {
    if s.trim().is_empty() {
        Err(format!("{what}: empty string"))
    } else {
        Ok(())
    }
}

fn hex40(s: &str, what: &str) -> Result<(), String> {
    if s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{what}: not a 40-hex-digit sha"))
    }
}

/// Every runtime action id the exporter knows for a fighter (the full
/// `frame-data.csv` action set).
pub fn runtime_action_ids() -> Vec<&'static str> {
    attacks::ALL_MOVES
        .iter()
        .map(|m| crate::export::action_id(*m))
        .collect()
}

fn move_by_id(s: &str) -> Option<MoveId> {
    attacks::ALL_MOVES
        .iter()
        .copied()
        .find(|m| crate::export::action_id(*m) == s)
}

/// Validate the complete file against FORMAT.md and bind it to `model`
/// (bones, pieces) and the runtime action set.
pub fn resolve(json: &str, model: &CharacterModel) -> Result<Directions, String> {
    let file = parse(json)?;
    if file.schema_version != 1 {
        return Err(format!(
            "unsupported schema_version {}",
            file.schema_version
        ));
    }
    let model_id = model.spec_id.unwrap_or("");
    if file.character_id != model_id {
        return Err(format!(
            "character_id `{}` does not match the model `{model_id}`",
            file.character_id
        ));
    }
    nonempty(&file.status, "status")?;
    hex40(&file.source_sha, "source_sha")?;
    hex40(&file.evidence_commit, "evidence_commit")?;
    nonempty(&file.runtime_actions, "runtime_actions")?;
    nonempty(&file.intent, "intent")?;
    let t = &file.timing_contract;
    for (s, w) in [
        (&t.authority, "timing_contract.authority"),
        (
            &t.anticipation_fraction,
            "timing_contract.anticipation_fraction",
        ),
        (&t.active, "timing_contract.active"),
        (&t.recovery, "timing_contract.recovery"),
        (&t.variants, "timing_contract.variants"),
    ] {
        nonempty(s, w)?;
    }
    let g = &file.geometry_contract;
    for (s, w) in [
        (&g.units, "geometry_contract.units"),
        (&g.contact, "geometry_contract.contact"),
        (&g.special_cases, "geometry_contract.special_cases"),
        (&g.extras, "geometry_contract.extras"),
    ] {
        nonempty(s, w)?;
    }
    if file.reference_review.is_empty() {
        return Err("reference_review: empty".into());
    }
    for (i, r) in file.reference_review.iter().enumerate() {
        nonempty(&r.url, &format!("reference_review[{i}].url"))?;
        nonempty(&r.accessed, &format!("reference_review[{i}].accessed"))?;
        nonempty(&r.method, &format!("reference_review[{i}].method"))?;
    }
    nonempty(&file.reference_audit_status, "reference_audit_status")?;

    // Actions: unique, exactly the runtime set, bound to real bones/pieces.
    let runtime = runtime_action_ids();
    let mut seen: Vec<&str> = Vec::new();
    let mut actions = Vec::with_capacity(file.actions.len());
    for a in &file.actions {
        let w = format!("actions[{}]", a.action_id);
        if seen.contains(&a.action_id.as_str()) {
            return Err(format!("{w}: duplicate action_id"));
        }
        seen.push(&a.action_id);
        let id = move_by_id(&a.action_id)
            .ok_or_else(|| format!("{w}: not a runtime action of this fighter"))?;
        let family = PoseFamily::parse(&a.pose_family).ok_or_else(|| {
            format!(
                "{w}: pose_family `{}` is not implemented (explicitly pending)",
                a.pose_family
            )
        })?;
        if !a.anticipation_fraction.is_finite() || !(0.0..=1.0).contains(&a.anticipation_fraction) {
            return Err(format!(
                "{w}: anticipation_fraction must be finite in 0..=1"
            ));
        }
        for (s, f) in [
            (&a.arc, "arc"),
            (&a.torso, "torso"),
            (&a.other_hand, "other_hand"),
            (&a.recovery, "recovery"),
        ] {
            nonempty(s, &format!("{w}.{f}"))?;
        }
        let bone = model
            .rig
            .bone(&a.contact_bone)
            .ok_or_else(|| format!("{w}: unknown contact_bone `{}`", a.contact_bone))?;
        let (piece, pr) = model
            .pieces
            .iter()
            .enumerate()
            .find(|(_, p)| p.id == a.contact_piece_id)
            .ok_or_else(|| format!("{w}: unknown contact_piece_id `{}`", a.contact_piece_id))?;
        if pr.bone != bone {
            return Err(format!(
                "{w}: piece `{}` belongs to bone `{}`, not `{}`",
                a.contact_piece_id, model.rig.bones[pr.bone].name, a.contact_bone
            ));
        }
        if pr.vertices.1 <= pr.vertices.0 {
            return Err(format!(
                "{w}: piece `{}` has no vertices",
                a.contact_piece_id
            ));
        }
        let mesh = &model.rig.bones[bone].mesh;
        let range = pr.vertices.0 as usize..pr.vertices.1 as usize;
        let n = range.len() as f32;
        let effector = mesh.pos[range]
            .iter()
            .fold(V3::ZERO, |acc, p| acc + *p)
            .scale(1.0 / n);
        actions.push(Resolved {
            id,
            action_id: crate::export::action_id(id),
            family,
            fraction: a.anticipation_fraction,
            bone,
            piece,
            piece_id: a.contact_piece_id.clone(),
            effector: v3(effector.x, effector.y, effector.z),
            arc: a.arc.clone(),
            torso: a.torso.clone(),
            other_hand: a.other_hand.clone(),
            recovery: a.recovery.clone(),
        });
    }
    let missing: Vec<&str> = runtime
        .iter()
        .copied()
        .filter(|r| !seen.contains(r))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "actions: runtime actions without a direction: {}",
            missing.join(", ")
        ));
    }
    Ok(Directions {
        character_id: file.character_id.clone(),
        status: file.status.clone(),
        source_sha: file.source_sha.clone(),
        evidence_commit: file.evidence_commit.clone(),
        sha256: sha256_hex(json.as_bytes()),
        actions,
    })
}

// ----------------------------------------------------------------- sha-256

/// SHA-256 (FIPS 180-4) of `data`, lowercase hex. Small and dependency-free
/// so the embedded direction text can be identified in exports exactly the
/// way `tools/evidence.py` hashes the file on disk.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h;
        for i in 0..64 {
            let s1 = a[4].rotate_right(6) ^ a[4].rotate_right(11) ^ a[4].rotate_right(25);
            let ch = (a[4] & a[5]) ^ (!a[4] & a[6]);
            let t1 = a[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a[0].rotate_right(2) ^ a[0].rotate_right(13) ^ a[0].rotate_right(22);
            let maj = (a[0] & a[1]) ^ (a[0] & a[2]) ^ (a[1] & a[2]);
            let t2 = s0.wrapping_add(maj);
            a = [
                t1.wrapping_add(t2),
                a[0],
                a[1],
                a[2],
                a[3].wrapping_add(t1),
                a[4],
                a[5],
                a[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(a[i]);
        }
    }
    let mut out = String::with_capacity(64);
    for x in h {
        out.push_str(&format!("{x:08x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::characters::build;
    use crate::sim::roster::CharacterId;

    fn kestrel() -> CharacterModel {
        build(CharacterId::Kestrel)
    }

    #[test]
    fn sha256_known_answers() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Two blocks (56 bytes forces the length into a second block).
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn kestrel_directions_resolve_against_the_model_and_runtime() {
        let m = kestrel();
        let d = resolve(KESTREL_ANIM_JSON, &m).expect("kestrel.json resolves");
        assert_eq!(d.character_id, "kestrel");
        assert_eq!(d.actions.len(), 22);
        assert_eq!(d.actions.len(), runtime_action_ids().len());
        assert_eq!(d.sha256.len(), 64);
        // Distinct runtime ids, one direction each.
        let mut ids: Vec<MoveId> = d.actions.iter().map(|a| a.id).collect();
        ids.sort_by_key(|m| *m as u32);
        ids.dedup();
        assert_eq!(ids.len(), 22);
        // The exact pieces the review names.
        let piece = |id: MoveId| d.get(id).unwrap().piece_id.clone();
        assert_eq!(piece(MoveId::Utilt), "k_hand_r");
        assert_eq!(piece(MoveId::DashAttack), "k_hand_r");
        assert_eq!(piece(MoveId::SpecialSide), "k_hand_r");
        assert_eq!(piece(MoveId::SpecialDown), "k_chest");
        assert_eq!(piece(MoveId::Bair), "k_foot_l");
        assert_eq!(piece(MoveId::Jab2), "k_hand_l");
        assert_eq!(piece(MoveId::ThrowB), "k_hand_l");
        // The effector is the piece's own centroid in its bone frame
        // (kestrel.json: k_hand_r at (0.25, -0.65, 0), k_foot_l at (0.65, -0.1, 0)).
        let hr = d.get(MoveId::Jab).unwrap();
        assert_eq!(m.rig.bones[hr.bone].name, "hand_r");
        assert!((hr.effector.x - 0.25).abs() < 1e-3 && (hr.effector.y + 0.65).abs() < 1e-3);
        let fl = d.get(MoveId::Bair).unwrap();
        assert_eq!(m.rig.bones[fl.bone].name, "foot_l");
        assert!((fl.effector.x - 0.65).abs() < 1e-3 && (fl.effector.y + 0.1).abs() < 1e-3);
        assert_eq!(d.get(MoveId::Fsmash).unwrap().family, PoseFamily::PalmDrive);
        assert_eq!(d.get(MoveId::Jab).unwrap().fraction, 0.0);
        assert_eq!(d.get(MoveId::Fsmash).unwrap().fraction, 0.7);
    }

    #[test]
    fn every_shipped_direction_file_binds_to_its_model() {
        for (id, json) in ALL_DIRECTIONS {
            let m = build(crate::export::parse_character(id).unwrap());
            let d = resolve(json, &m).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(d.character_id, id);
        }
    }

    fn edited(f: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut v: serde_json::Value = serde_json::from_str(KESTREL_ANIM_JSON).unwrap();
        f(&mut v);
        serde_json::to_string(&v).unwrap()
    }

    #[test]
    fn unknown_fields_fail_at_every_level() {
        let m = kestrel();
        let top = edited(|v| {
            v["extra_top"] = serde_json::json!("x");
        });
        assert!(resolve(&top, &m).unwrap_err().contains("extra_top"));
        let timing = edited(|v| {
            v["timing_contract"]["anticipation"] = serde_json::json!("x");
        });
        assert!(resolve(&timing, &m).is_err());
        let geom = edited(|v| {
            v["geometry_contract"]["scale"] = serde_json::json!("x");
        });
        assert!(resolve(&geom, &m).is_err());
        let refs = edited(|v| {
            v["reference_review"][0]["note"] = serde_json::json!("x");
        });
        assert!(resolve(&refs, &m).is_err());
        let act = edited(|v| {
            v["actions"][3]["contact_piece"] = serde_json::json!("k_hand_r");
        });
        assert!(resolve(&act, &m).unwrap_err().contains("contact_piece"));
        // A misspelt required field is unknown + missing: still an error.
        let misspelt = KESTREL_ANIM_JSON.replacen("\"pose_family\"", "\"pose_familly\"", 1);
        assert!(resolve(&misspelt, &m).is_err());
    }

    #[test]
    fn binding_errors_are_named() {
        let m = kestrel();
        let e = |s: String| resolve(&s, &m).unwrap_err();
        assert!(e(edited(|v| v["schema_version"] = serde_json::json!(2))).contains("schema"));
        assert!(
            e(edited(|v| v["character_id"] = serde_json::json!("boulder")))
                .contains("character_id")
        );
        assert!(e(edited(|v| v["source_sha"] = serde_json::json!("abc"))).contains("source_sha"));
        assert!(e(edited(
            |v| v["actions"][0]["pose_family"] = serde_json::json!("handstand")
        ))
        .contains("not implemented"));
        assert!(e(edited(
            |v| v["actions"][0]["anticipation_fraction"] = serde_json::json!(1.5)
        ))
        .contains("anticipation_fraction"));
        assert!(e(edited(
            |v| v["actions"][0]["contact_bone"] = serde_json::json!("wing")
        ))
        .contains("contact_bone"));
        // Piece exists but sits on another bone.
        assert!(e(edited(
            |v| v["actions"][0]["contact_piece_id"] = serde_json::json!("k_hand_l")
        ))
        .contains("belongs to bone"));
        // An accessory on the wrong bone is not a contact piece either.
        assert!(e(edited(
            |v| v["actions"][0]["contact_piece_id"] = serde_json::json!("k_scarf_a")
        ))
        .contains("belongs to bone"));
        assert!(e(edited(|v| v["actions"][0]["arc"] = serde_json::json!("  "))).contains("arc"));
        // Duplicate and missing actions.
        assert!(e(edited(
            |v| v["actions"][1]["action_id"] = serde_json::json!("jab")
        ))
        .contains("duplicate"));
        assert!(e(edited(|v| {
            v["actions"].as_array_mut().unwrap().pop();
        }))
        .contains("without a direction"));
        assert!(e(edited(
            |v| v["actions"][0]["action_id"] = serde_json::json!("jab3")
        ))
        .contains("not a runtime action"));
        assert!(e(edited(|v| v["reference_review"] = serde_json::json!([])))
            .contains("reference_review"));
    }
}
