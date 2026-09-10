//! `docs/art/capture-suite.json` — the declared cameras, cells, fixtures
//! and contracts of the evidence suite (`docs/art/EXCHANGE.md`).
//!
//! The file is owned by the art reviewer; the capture tool **must** use
//! what it declares and refuse anything it does not understand. Every
//! object is parsed with `deny_unknown_fields`, nested ones included, so a
//! misspelt key is an authoring error, never a silently ignored setting.
//! Camera positions that depend on the sampled fighter are written as small
//! expressions (`"player.x + 10*facing"`, `"root.y@anchor + 22"`), parsed
//! here into [`CamAxis`] and evaluated by the capture tool.

use serde::Deserialize;

pub const SUITE_JSON: &str = include_str!("../docs/art/capture-suite.json");

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub schema_version: u32,
    pub id: String,
    pub units: String,
    pub kind_required: String,
    pub warmup_ticks: u32,
    pub turnaround: TurnaroundSpec,
    pub combat: CombatSpec,
    pub gif: GifSpec,
    pub runtime_frame_data: String,
    pub characters: Vec<String>,
    #[serde(default)]
    pub characters_note: Option<String>,
    /// Baseline inventory only: coverage is every case of the runtime
    /// exporter (`contact_actions_note`).
    #[serde(default)]
    pub contact_actions: Vec<ContactAction>,
    #[serde(default)]
    pub contact_actions_note: Option<String>,
    pub contact_camera: ContactCamera,
    pub contact_wide_camera: ContactWideCamera,
    pub contact_exceptions: ContactExceptions,
    #[serde(default)]
    pub contact_render_contract: Option<String>,
    pub fixtures: Fixtures,
    pub combat_size: CombatSize,
    pub ui_selection: UiSelection,
    #[serde(default)]
    pub tool: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TurnaroundSpec {
    pub output: [u32; 2],
    pub grid: [u32; 2],
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub orthographic_height: f32,
    pub root_yaw_deg: Vec<f32>,
    pub pose: String,
    pub lag_enabled: bool,
    #[serde(default)]
    pub yaw_convention: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct CombatSpec {
    pub output: [u32; 2],
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub orthographic_width: f32,
    pub players: Vec<CombatPlayer>,
    pub fixture_tick: u32,
    pub depth_eye: [f32; 3],
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct CombatPlayer {
    pub root: [f32; 3],
    pub facing: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GifSpec {
    pub output: [u32; 2],
    pub simulation_hz: u32,
    pub duration_ticks: u32,
    pub sample_stride: u32,
    pub playback_fps: u32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContactAction {
    pub action_id: String,
    pub variant_id: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContactCamera {
    pub eye: [serde_json::Value; 3],
    pub target: [serde_json::Value; 3],
    pub orthographic_height: f32,
    pub cell: [u32; 2],
    pub cells: Vec<String>,
    pub overlay: String,
    pub values: String,
    pub output: [u32; 2],
    pub sample_phase: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContactWideCamera {
    pub output: [u32; 2],
    pub cell: [u32; 2],
    pub grid: [u32; 2],
    pub eye: [serde_json::Value; 3],
    pub target: [serde_json::Value; 3],
    pub up: [f32; 3],
    pub orthographic_height: f32,
    pub rows_facing: Vec<f32>,
    pub cells: Vec<String>,
    pub anchor: String,
    pub camera_rule: String,
    pub naming: String,
    pub overlay: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContactExceptions {
    pub projectile_primary: Vec<String>,
    pub projectile_wide: Vec<String>,
    pub throw_primary: Vec<String>,
    pub throw_wide: Vec<String>,
    pub contract: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Fixtures {
    pub idle: String,
    pub combat: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct CombatSize {
    pub output: [u32; 2],
    pub cells: Vec<CombatSizeCell>,
    pub source: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct CombatSizeCell {
    pub projected_height_px: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct UiSelection {
    pub screen: String,
    pub window: [u32; 2],
    pub p1: SelectSlot,
    pub p2: SelectSlot,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct SelectSlot {
    pub character: String,
    pub palette: u8,
}

// ----------------------------------------------------------------- cameras

/// One axis of a declared camera position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CamAxis {
    /// A fixed world coordinate.
    Const(f32),
    /// `<base>.x + k*facing` — the fighter's x plus `k` units forward.
    X { forward: f32 },
    /// `<base>.y + k` — the fighter's y plus `k`.
    Y { up: f32 },
}

/// What `<base>` refers to in a camera expression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CamBase {
    /// The fighter of the cell being drawn (`player`).
    Player,
    /// The fighter on the row's anchor tick (`root@anchor`).
    RootAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CamRule {
    pub base: CamBase,
    pub eye: [CamAxis; 3],
    pub target: [CamAxis; 3],
    pub orthographic_height: f32,
}

impl CamRule {
    /// Resolve to world coordinates for a fighter at `(x, y)` facing `facing`.
    pub fn resolve(&self, x: f32, y: f32, facing: f32) -> ([f32; 3], [f32; 3]) {
        let ax = |a: &CamAxis| match *a {
            CamAxis::Const(c) => c,
            CamAxis::X { forward } => x + forward * facing,
            CamAxis::Y { up } => y + up,
        };
        (
            [ax(&self.eye[0]), ax(&self.eye[1]), ax(&self.eye[2])],
            [
                ax(&self.target[0]),
                ax(&self.target[1]),
                ax(&self.target[2]),
            ],
        )
    }
}

fn parse_axis(v: &serde_json::Value, expect: char) -> Result<(CamAxis, Option<CamBase>), String> {
    if let Some(n) = v.as_f64() {
        return Ok((CamAxis::Const(n as f32), None));
    }
    let s = v
        .as_str()
        .ok_or_else(|| format!("camera axis must be a number or an expression, got {v}"))?;
    let s = s.trim();
    // "<base>.<axis>[@anchor] + <k>[*facing]"
    let (lhs, rhs) = s
        .split_once('+')
        .ok_or_else(|| format!("camera expression `{s}`: expected `<base>.<axis> + <k>`"))?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    let (base_name, axis_part) = lhs
        .split_once('.')
        .ok_or_else(|| format!("camera expression `{s}`: missing `.x`/`.y`"))?;
    let (axis_name, anchored) = match axis_part.strip_suffix("@anchor") {
        Some(a) => (a, true),
        None => (axis_part, false),
    };
    let base = match (base_name, anchored) {
        ("player", false) => CamBase::Player,
        ("root", true) => CamBase::RootAnchor,
        _ => {
            return Err(format!(
                "camera expression `{s}`: base must be `player.<axis>` or `root.<axis>@anchor`"
            ))
        }
    };
    if axis_name.len() != 1 || !axis_name.starts_with(expect) {
        return Err(format!(
            "camera expression `{s}`: axis `{axis_name}` on the {expect} component"
        ));
    }
    let axis = match expect {
        'x' => {
            let k = rhs.strip_suffix("*facing").ok_or_else(|| {
                format!("camera expression `{s}`: x offsets must be `<k>*facing`")
            })?;
            CamAxis::X {
                forward: k
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| format!("camera expression `{s}`: bad number `{k}`"))?,
            }
        }
        'y' => CamAxis::Y {
            up: rhs
                .parse::<f32>()
                .map_err(|_| format!("camera expression `{s}`: bad number `{rhs}`"))?,
        },
        _ => return Err(format!("camera expression `{s}`: z must be a number")),
    };
    Ok((axis, Some(base)))
}

fn parse_rule(
    eye: &[serde_json::Value; 3],
    target: &[serde_json::Value; 3],
    ortho: f32,
    expected_base: CamBase,
    what: &str,
) -> Result<CamRule, String> {
    let mut bases = Vec::new();
    let mut parse3 = |v: &[serde_json::Value; 3]| -> Result<[CamAxis; 3], String> {
        let (x, bx) = parse_axis(&v[0], 'x')?;
        let (y, by) = parse_axis(&v[1], 'y')?;
        let (z, bz) = parse_axis(&v[2], 'z')?;
        bases.extend([bx, by, bz].into_iter().flatten());
        Ok([x, y, z])
    };
    let eye = parse3(eye).map_err(|e| format!("{what}.eye: {e}"))?;
    let target = parse3(target).map_err(|e| format!("{what}.target: {e}"))?;
    if bases.iter().any(|b| *b != expected_base) {
        return Err(format!("{what}: expressions must use {expected_base:?}"));
    }
    if !ortho.is_finite() || ortho <= 0.0 {
        return Err(format!("{what}: bad orthographic_height {ortho}"));
    }
    if !matches!(eye[2], CamAxis::Const(_)) || !matches!(target[2], CamAxis::Const(_)) {
        return Err(format!("{what}: z components must be numbers"));
    }
    Ok(CamRule {
        base: expected_base,
        eye,
        target,
        orthographic_height: ortho,
    })
}

impl ContactCamera {
    pub fn rule(&self) -> Result<CamRule, String> {
        parse_rule(
            &self.eye,
            &self.target,
            self.orthographic_height,
            CamBase::Player,
            "contact_camera",
        )
    }
}

impl ContactWideCamera {
    pub fn rule(&self) -> Result<CamRule, String> {
        parse_rule(
            &self.eye,
            &self.target,
            self.orthographic_height,
            CamBase::RootAnchor,
            "contact_wide_camera",
        )
    }
}

// ----------------------------------------------------------------- parse

/// Parse and validate the embedded suite.
pub fn load() -> Result<Suite, String> {
    parse(SUITE_JSON)
}

pub fn parse(json: &str) -> Result<Suite, String> {
    let s: Suite = serde_json::from_str(json).map_err(|e| format!("capture-suite.json: {e}"))?;
    validate(&s)?;
    Ok(s)
}

/// Consistency rules the capture tool relies on.
pub fn validate(s: &Suite) -> Result<(), String> {
    let e = |m: String| Err(format!("capture-suite.json: {m}"));
    if s.schema_version != 1 {
        return e(format!("unsupported schema_version {}", s.schema_version));
    }
    if s.units != "game_units" {
        return e(format!("units must be game_units, got `{}`", s.units));
    }
    if s.kind_required != "game3d" {
        return e(format!(
            "kind_required must be game3d, got `{}`",
            s.kind_required
        ));
    }
    // Turnaround grid × cells = output.
    let t = &s.turnaround;
    if t.grid[0] == 0
        || t.grid[1] == 0
        || t.output[0] % t.grid[0] != 0
        || t.output[1] % t.grid[1] != 0
    {
        return e("turnaround: output must be a multiple of grid".into());
    }
    if t.root_yaw_deg.len() != (t.grid[0] * t.grid[1]) as usize {
        return e("turnaround: one yaw per grid cell".into());
    }
    if t.pose != "rest" {
        return e(format!(
            "turnaround.pose `{}` is not supported (rest)",
            t.pose
        ));
    }
    // Combat.
    if s.combat.players.len() != 2 {
        return e("combat: exactly two players".into());
    }
    if s.gif.sample_stride == 0 || s.gif.playback_fps == 0 || s.gif.duration_ticks == 0 {
        return e("gif: stride, fps and duration must be positive".into());
    }
    // Characters.
    if s.characters.is_empty() {
        return e("characters: at least one".into());
    }
    for c in &s.characters {
        if crate::export::parse_character(c).is_none() {
            return e(format!("characters: unknown `{c}`"));
        }
    }
    // Contact camera: 3 cells laid out horizontally.
    let cc = &s.contact_camera;
    cc.rule()?;
    if cc.cells.len() != 3 {
        return e("contact_camera.cells: exactly three".into());
    }
    if cc.output != [cc.cell[0] * 3, cc.cell[1]] {
        return e(format!(
            "contact_camera.output {:?} must be cell × [3, 1] = {:?}",
            cc.output,
            [cc.cell[0] * 3, cc.cell[1]]
        ));
    }
    // Wide camera: grid × cells, one row per facing.
    let cw = &s.contact_wide_camera;
    cw.rule()?;
    if cw.grid[0] as usize != cw.cells.len() {
        return e("contact_wide_camera: grid[0] must equal the number of cells".into());
    }
    if cw.grid[1] as usize != cw.rows_facing.len() {
        return e("contact_wide_camera: grid[1] must equal rows_facing".into());
    }
    if cw.output != [cw.cell[0] * cw.grid[0], cw.cell[1] * cw.grid[1]] {
        return e("contact_wide_camera.output must be cell × grid".into());
    }
    if cw.rows_facing.iter().any(|f| *f != 1.0 && *f != -1.0) {
        return e("contact_wide_camera.rows_facing: ±1 only".into());
    }
    // Exceptions: same cell counts as the sheets they replace.
    let x = &s.contact_exceptions;
    if x.projectile_primary.len() != cc.cells.len() || x.throw_primary.len() != cc.cells.len() {
        return e("contact_exceptions: primary lists must match contact_camera.cells".into());
    }
    if x.projectile_wide.len() != cw.cells.len() || x.throw_wide.len() != cw.cells.len() {
        return e("contact_exceptions: wide lists must match contact_wide_camera.cells".into());
    }
    // Fixtures must be the embedded ones (the executable carries them).
    if s.fixtures.idle != "docs/art/fixtures/idle-v1.json"
        || s.fixtures.combat != "docs/art/fixtures/combat-v1.json"
    {
        return e("fixtures: paths must name the embedded fixtures".into());
    }
    if s.combat_size.cells.is_empty()
        || s.combat_size
            .cells
            .iter()
            .any(|c| c.projected_height_px <= 0.0)
    {
        return e("combat_size: positive projected heights".into());
    }
    if s.combat_size.output[0] % s.combat_size.cells.len() as u32 != 0 {
        return e("combat_size.output width must divide into its cells".into());
    }
    for slot in [&s.ui_selection.p1, &s.ui_selection.p2] {
        if crate::export::parse_character(&slot.character).is_none() {
            return e(format!(
                "ui_selection: unknown character `{}`",
                slot.character
            ));
        }
    }
    // Declared contact inventory must be a subset of the runtime's cases.
    for a in &s.contact_actions {
        let known = s.characters.iter().any(|c| {
            crate::export::parse_character(c).is_some_and(|ch| {
                crate::export::cases_for(ch).iter().any(|(_, id, v)| {
                    crate::export::action_id(*id) == a.action_id && *v == a.variant_id
                })
            })
        });
        if !known {
            return e(format!(
                "contact_actions: `{}`/`{}` is not a runtime case",
                a.action_id, a.variant_id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_suite_parses_and_declares_the_agreed_layout() {
        let s = load().expect("suite");
        assert_eq!(s.contact_camera.output, [1536, 512]);
        assert_eq!(s.contact_camera.orthographic_height, 44.0);
        assert_eq!(s.contact_wide_camera.output, [2048, 1024]);
        // The wide camera is the reviewer's call (h176 after the NVIDIA
        // run showed h88 clipping special_up / nair / the projectile).
        assert!(s.contact_wide_camera.orthographic_height >= s.contact_camera.orthographic_height);
        assert_eq!(s.contact_wide_camera.rows_facing, vec![1.0, -1.0]);
        assert_eq!(s.characters, vec!["kestrel"]);
        // 23 declared cases over 22 actions.
        assert_eq!(s.contact_actions.len(), 23);
        let mut actions: Vec<&str> = s
            .contact_actions
            .iter()
            .map(|a| a.action_id.as_str())
            .collect();
        actions.dedup();
        assert_eq!(actions.len(), 22);
        let r = s.contact_camera.rule().unwrap();
        assert_eq!(r.base, CamBase::Player);
        assert_eq!(
            r.eye,
            [
                CamAxis::X { forward: 10.0 },
                CamAxis::Y { up: 22.0 },
                CamAxis::Const(120.0)
            ]
        );
        let (eye, target) = r.resolve(5.0, 1.0, -1.0);
        assert_eq!(eye, [-5.0, 23.0, 120.0]);
        assert_eq!(target, [-5.0, 23.0, 0.0]);
        let w = s.contact_wide_camera.rule().unwrap();
        assert_eq!(w.base, CamBase::RootAnchor);
    }

    #[test]
    fn unknown_fields_fail_at_every_level() {
        let top = SUITE_JSON.replacen("\"schema_version\"", "\"schema_versoin\"", 1);
        assert!(parse(&top).is_err());
        assert!(
            edited(|v| v["typo"] = 1.into()).is_err(),
            "top-level unknown field"
        );
        assert!(
            edited(|v| v["contact_wide_camera"]["zoom"] = 2.into()).is_err(),
            "nested unknown field must fail"
        );
        assert!(
            edited(|v| v["combat_size"]["cells"][0]["extra"] = true.into()).is_err(),
            "deeply nested unknown field must fail"
        );
        assert!(
            edited(|v| v["ui_selection"]["p1"]["skin"] = 0.into()).is_err(),
            "nested slot unknown field must fail"
        );
        assert!(
            edited(|v| v["contact_actions"][0]["note"] = "x".into()).is_err(),
            "array element unknown field must fail"
        );
    }

    fn edited(edit: impl FnOnce(&mut serde_json::Value)) -> Result<Suite, String> {
        let mut v: serde_json::Value = serde_json::from_str(SUITE_JSON).unwrap();
        edit(&mut v);
        parse(&v.to_string())
    }

    #[test]
    fn camera_expressions_are_strict() {
        assert!(
            edited(|v| v["contact_camera"]["eye"][0] = "player.x + 10".into()).is_err(),
            "x offsets must be scaled by facing"
        );
        assert!(
            edited(|v| v["contact_wide_camera"]["eye"][0] = "player.x + 10*facing".into()).is_err(),
            "wide camera must anchor on root@anchor"
        );
        assert!(
            edited(|v| v["contact_camera"]["eye"][1] = "player.x + 22".into()).is_err(),
            "y component must use .y"
        );
        assert!(
            edited(|v| v["contact_camera"]["eye"][2] = "player.z + 1".into()).is_err(),
            "z must be a number"
        );
        assert!(edited(|v| v["contact_camera"]["orthographic_height"] = (-1.0).into()).is_err());
    }

    #[test]
    fn layout_consistency_is_checked() {
        assert!(
            edited(|v| v["contact_camera"]["output"] = serde_json::json!([1024, 512])).is_err(),
            "primary output must be 3 cells"
        );
        assert!(
            edited(|v| v["contact_wide_camera"]["rows_facing"] = serde_json::json!([1])).is_err(),
            "grid rows must equal rows_facing"
        );
        assert!(
            edited(|v| v["contact_exceptions"]["throw_wide"] = serde_json::json!(["a", "b"]))
                .is_err(),
            "exception lists must match the cell counts"
        );
        assert!(
            edited(|v| v["fixtures"]["idle"] = "docs/art/fixtures/other.json".into()).is_err(),
            "fixtures must be the embedded ones"
        );
        assert!(
            edited(|v| v["contact_actions"][0]["variant_id"] = "nope".into()).is_err(),
            "declared inventory must be runtime cases"
        );
        assert!(
            edited(|v| v["characters"] = serde_json::json!(["trama"])).is_err(),
            "unknown character"
        );
    }
}
