//! Kestrel's compact discharge: one immediate palm-born release, a small
//! recoil, one settle back to guard.
//!
//! Every assertion here is about the **presentation** of an action whose
//! only real event is the shot leaving on its first tick. Nothing in this
//! file may change, and nothing here reads, any causal value: the timing,
//! the 32-tick commitment, the emission origin, the projectile's speed,
//! range and damage, the hitboxes, the hurt capsule, the root, the scale
//! and the bone lengths are all pinned below exactly as the simulation has
//! them, and the whole point of the pins is that this batch left them
//! alone.
//!
//! The measured geometric limit this direction asked to be reported rather
//! than worked around is asserted here too, as a *bound*: Kestrel's right
//! arm cannot put the palm on the emission origin, so the release is
//! required to point at the origin along the shoulder's own line, with a
//! real elbow bend and a compact chest, and the residual gap is not allowed
//! to quietly grow.
//!
//! Run with: `cargo test --test special_n_presentation -- --nocapture`

use overframe::export;
use overframe::model::anim_directed::{self, PROJECTILE_SPAWN_LOCAL_X};
use overframe::model::characters::build;
use overframe::model::math3::Xf;
use overframe::model::rig::Pose;
use overframe::sim::attacks::{self, MoveId};
use overframe::sim::fighter::{Fighter, State};
use overframe::sim::roster::CharacterId;
use overframe::sim::Vec2;

/// Every case of Kestrel's discharge this batch is responsible for.
const CASES: [&str; 4] = ["ground", "air", "air_full", "from_shield"];

// ------------------------------------------------------------- failures

#[derive(Default)]
struct Failures {
    what: String,
    items: Vec<String>,
}

impl Failures {
    fn new(what: &str) -> Self {
        Failures {
            what: what.to_string(),
            items: Vec::new(),
        }
    }
    fn check(&mut self, ok: bool, msg: impl FnOnce() -> String) {
        if !ok {
            self.items.push(msg());
        }
    }
    fn fail(&mut self, msg: String) {
        self.items.push(msg);
    }
    fn finish(self) {
        if !self.items.is_empty() {
            panic!(
                "{}: {} failed expectation(s)\n  - {}",
                self.what,
                self.items.len(),
                self.items.join("\n  - ")
            );
        }
    }
}

// ------------------------------------------------------------- geometry

/// The release geometry of the drawn pose, measured on the rig.
struct Geom {
    palm: [f32; 2],
    shoulder: [f32; 2],
    shoulder_to_origin: f32,
    palm_reach: f32,
    gap: f32,
    off_line: f32,
    elbow: f32,
    lean: f32,
    root_y: f32,
}

fn pose_of(grounded: bool, sf: u32) -> (overframe::model::characters::CharacterModel, Pose) {
    let m = build(CharacterId::Kestrel);
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, Vec2::ZERO);
    f.facing = 1.0;
    f.grounded = grounded;
    f.set_state_pub(State::Attack {
        id: MoveId::SpecialN,
        aerial: !grounded,
    });
    f.state_frame = sf;
    let _ = md;
    let p = anim_directed::fighter_pose(&m, &f, 0);
    (m, p)
}

fn geom(grounded: bool, sf: u32) -> Geom {
    let (m, p) = pose_of(grounded, sf);
    let rig = &m.rig;
    let d = m
        .directions
        .as_ref()
        .expect("Kestrel ships an authored direction")
        .get(MoveId::SpecialN)
        .expect("special_n is directed");
    let w = rig.world(&p, &Xf::IDENTITY);
    let sh = w[rig.bone("upper_arm_r").expect("upper_arm_r")].t;
    let el = w[rig.bone("forearm_r").expect("forearm_r")].t;
    let palm = w[d.bone].point(d.effector);
    let origin = [PROJECTILE_SPAWN_LOCAL_X, 15.0];
    let v1 = [el.x - sh.x, el.y - sh.y];
    let v2 = [palm.x - el.x, palm.y - el.y];
    let n1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt().max(1e-6);
    let n2 = (v2[0] * v2[0] + v2[1] * v2[1]).sqrt().max(1e-6);
    let elbow = ((v1[0] * v2[0] + v1[1] * v2[1]) / (n1 * n2))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees();
    let chest = w[rig.bone("chest").expect("chest")].t;
    let head = w[rig.bone("head").expect("head")].t;
    let lean = (head.x - chest.x)
        .atan2((head.y - chest.y).max(1e-6))
        .to_degrees();
    let dxy = [origin[0] - sh.x, origin[1] - sh.y];
    let dlen = (dxy[0] * dxy[0] + dxy[1] * dxy[1]).sqrt().max(1e-6);
    let pv = [palm.x - sh.x, palm.y - sh.y];
    Geom {
        palm: [palm.x, palm.y],
        shoulder: [sh.x, sh.y],
        shoulder_to_origin: dlen,
        palm_reach: (pv[0] * pv[0] + pv[1] * pv[1]).sqrt(),
        gap: ((palm.x - origin[0]).powi(2) + (palm.y - origin[1]).powi(2)).sqrt(),
        off_line: (pv[0] * dxy[1] - pv[1] * dxy[0]).abs() / dlen,
        elbow,
        lean,
        root_y: w[rig.bone("root").expect("root")].t.y,
    }
}

// --------------------------------------------------------------- the pose

/// The release itself: a bent-elbow palm on the shoulder's own line to the
/// emission origin, from a compact chest — and the measured shortfall that
/// the skeleton makes unavoidable, bounded so it cannot drift.
#[test]
fn the_release_is_a_bent_elbow_palm_pointing_at_the_real_origin() {
    let mut f = Failures::new("the release pose");
    for &grounded in &[true, false] {
        let g = geom(grounded, 1);
        let tag = if grounded { "ground" } else { "air" };
        println!(
            "[SPECIAL_N] {tag} release: palm=({:+.3},{:+.3}) gap={:.3} shoulder=({:+.3},{:+.3}) shoulder_to_origin={:.3} palm_reach={:.3} off_line={:.4} elbow={:.1} lean={:.1}",
            g.palm[0], g.palm[1], g.gap, g.shoulder[0], g.shoulder[1],
            g.shoulder_to_origin, g.palm_reach, g.off_line, g.elbow, g.lean
        );
        // Points *at* the origin: the palm sits on the shoulder->origin
        // line, so the discharge leaves along the emission line even though
        // it cannot reach the point itself.
        f.check(g.off_line < 0.5, || {
            format!("{tag}: the release must point at the origin, it is {:.4}u off the shoulder->origin line", g.off_line)
        });
        // A real bend. The presentation this replaces solved every lean
        // with the elbow at 0.5 degrees -- a straight-armed thrust.
        f.check((30.0..=70.0).contains(&g.elbow), || {
            format!(
                "{tag}: a bent-elbow release, not a straight arm: elbow {:.1} deg",
                g.elbow
            )
        });
        // A compact chest. The presentation this replaces measured 38.2.
        f.check(g.lean < 26.0, || {
            format!(
                "{tag}: a compact torso, not a lunge: chest {:.1} deg",
                g.lean
            )
        });
        // The centroid's distance is *not* a contact test and is not used
        // as one here: it is only checked to be the arm's own reach limit,
        // so this is a pose that extends as far as the chain allows rather
        // than one that gave up early. Whether the piece's real surface
        // reaches the emission region is a separate, measured question, and
        // it is answered with the contract's projected-triangle method in
        // `the_emission_contact_is_measured_and_reported_not_claimed`.
        let limit = g.shoulder_to_origin - g.palm_reach;
        f.check((g.gap - limit).abs() < 0.25, || {
            format!(
                "{tag}: the centroid sits at the arm's reach limit ({limit:.3}u), got {:.3}u",
                g.gap
            )
        });
        f.check(g.palm_reach > 9.0, || {
            format!(
                "{tag}: the arm is actually extended towards the origin: {:.3}u of {:.3}u possible",
                g.palm_reach, g.shoulder_to_origin
            )
        });
    }
    f.finish();
}

/// One release, one recoil, one settle: the chest's forward line peaks on
/// the emission tick and never comes back, and the palm never returns as
/// far forward as it was at the release. That is what "no second extension
/// at the old state-frame-9 marker" means measured on the pose.
#[test]
fn the_discharge_has_one_peak_and_never_extends_again() {
    let mut fl = Failures::new("one peak");
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    for &grounded in &[true, false] {
        let tag = if grounded { "ground" } else { "air" };
        let at_release = geom(grounded, 1);
        let mut min_gap_after = f32::INFINITY;
        let mut max_lean_after = f32::NEG_INFINITY;
        let mut recoiled = false;
        for sf in 2..md.total() {
            let g = geom(grounded, sf);
            min_gap_after = min_gap_after.min(g.gap);
            max_lean_after = max_lean_after.max(g.lean);
            if g.gap > at_release.gap + 1.0 {
                recoiled = true;
            }
            // No artificial root drift anywhere in the action.
            fl.check(g.root_y.abs() < 0.01, || {
                format!("{tag} sf{sf}: the drawn root drifted to {:.4}", g.root_y)
            });
        }
        println!(
            "[SPECIAL_N] {tag}: release gap {:.3} lean {:.1} | after the release, closest gap {:.3}, largest lean {:.1}",
            at_release.gap, at_release.lean, min_gap_after, max_lean_after
        );
        fl.check(min_gap_after >= at_release.gap - 1e-3, || {
            format!("{tag}: the palm comes back towards the emission after the release ({min_gap_after:.3} vs {:.3}) -- a second extension", at_release.gap)
        });
        fl.check(max_lean_after < at_release.lean - 1.0, || {
            format!("{tag}: the chest leans forward again after the release ({max_lean_after:.1} vs {:.1} deg)", at_release.lean)
        });
        fl.check(recoiled, || {
            format!("{tag}: the palm never withdraws -- there is no recoil, only a hold")
        });
    }
    fl.finish();
}

// -------------------------------------------------------------- the run

/// The same thing again, but measured on the exported evidence rather than
/// on the pose: both facings, every case, on the data the sheets carry.
#[test]
fn every_exported_case_shows_one_release_and_a_withdrawal() {
    let mut f = Failures::new("exported cases");
    for case in CASES {
        let j = match export::contact_json(CharacterId::Kestrel, MoveId::SpecialN, case) {
            Ok(j) => j,
            Err(e) => {
                f.fail(format!("{case}: the case did not run: {e}"));
                continue;
            }
        };
        for fc in j["facings"].as_array().expect("facings") {
            let facing = fc["facing"].as_f64().unwrap();
            let samples = fc["samples"].as_array().expect("samples");
            let tag = format!("{case} facing {facing:+}");
            let Some(spawn) = samples.iter().position(|s| !s["projectile"].is_null()) else {
                f.fail(format!("{tag}: the case never shows its projectile"));
                continue;
            };
            // The piece's real projected surface against the emission
            // region, tick by tick -- the contract's measurement, not the
            // centroid's.
            let gap = |s: &serde_json::Value| -> f64 {
                s["emission_origin"]["mesh_distance"]
                    .as_f64()
                    .expect("the emission origin carries the surface measurement")
            };
            let at_spawn = gap(&samples[spawn]);
            println!(
                "[SPECIAL_N] {tag}: spawn at sample {spawn} (state_frame {}), palm surface {:.3}u from the emission origin",
                samples[spawn]["state_frame"], at_spawn
            );
            // The emission origin and the first integrated projectile
            // position are reported apart, by exactly the one step the
            // simulation takes.
            let eo = samples[spawn]["emission_origin"]["center"][0]
                .as_f64()
                .unwrap();
            let pr = samples[spawn]["projectile"]["center"][0].as_f64().unwrap();
            f.check(
                ((pr - eo).abs() - attacks::PROJECTILE_SPEED as f64).abs() < 1e-3,
                || format!("{tag}: origin {eo:.3} and first integrated position {pr:.3} must differ by one step ({})", attacks::PROJECTILE_SPEED),
            );
            // One release: nothing later reaches back towards the emission.
            for (i, s) in samples.iter().enumerate().skip(spawn + 1) {
                f.check(gap(s) >= at_spawn - 1e-3, || {
                    format!("{tag}: sample {i} (state_frame {}) brings the palm surface back to {:.3}u, closer than the release's {at_spawn:.3}u", s["state_frame"], gap(s))
                });
            }
            // And it withdraws.
            f.check(
                samples
                    .get(spawn + 3)
                    .map(|s| gap(s) > at_spawn + 1.0)
                    .unwrap_or(false),
                || format!("{tag}: the palm holds its reach instead of recoiling"),
            );
            // Grounded feet stay above the floor on every grounded tick.
            for (i, s) in samples.iter().enumerate() {
                if s["support_reference"] != "floor" {
                    continue;
                }
                for fs in s["foot_support"].as_array().expect("foot_support") {
                    let dist = fs["support_distance"].as_f64().unwrap();
                    f.check(dist >= -1e-3, || {
                        format!(
                            "{tag}: sample {i} has {} at {dist:.4} below the floor",
                            fs["bone"]
                        )
                    });
                }
            }
        }
    }
    f.finish();
}

/// The long aerial case exists for one reason: to show the whole action in
/// the air. So it has to actually do that, and this checks the thing it
/// promises rather than a row count: every state frame of the move, 1
/// through the last, on consecutive ticks, airborne on every one of them,
/// and the state right after the move is an actionable `Air` — not landing
/// lag, and not the ground.
///
/// The short `air` case is kept beside it on purpose. It is what really
/// happens at low altitude: the ground interrupts the action on state frame
/// 12 of 32. Both are true; only one of them shows the recoil settling.
#[test]
fn the_long_aerial_case_runs_the_whole_action_in_the_air() {
    let mut f = Failures::new("aerial coverage");
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    for facing in [1.0f32, -1.0] {
        let ticks = match export::run_case_facing(
            CharacterId::Kestrel,
            MoveId::SpecialN,
            "air_full",
            facing,
        ) {
            Ok(t) => t,
            Err(e) => {
                f.fail(format!("air_full facing {facing:+}: {e}"));
                continue;
            }
        };
        // The action itself: the ticks the fighter is really in the move.
        // `before_release` is an after_step row too, which is why counting
        // rows would say 33 for a 32-frame action.
        let in_move: Vec<&export::CaseTick> = ticks
            .iter()
            .filter(|t| {
                matches!(
                    t.state.fighters[0].state,
                    State::Attack {
                        id: MoveId::SpecialN,
                        ..
                    }
                )
            })
            .collect();
        let frames: Vec<u32> = in_move.iter().map(|t| t.row.state_frame).collect();
        let ticks_of: Vec<i64> = in_move.iter().map(|t| t.tick_index).collect();
        let airborne = in_move.iter().all(|t| !t.state.fighters[0].grounded);
        println!(
            "[SPECIAL_N] air_full facing {facing:+}: {} ticks in the action, state_frames {}..{}, airborne throughout: {airborne}",
            in_move.len(),
            frames.first().copied().unwrap_or(0),
            frames.last().copied().unwrap_or(0)
        );
        // Exactly 1..=total, no gaps, no repeats.
        let want: Vec<u32> = (1..=md.total()).collect();
        f.check(frames == want, || {
            format!("air_full facing {facing:+}: the case must cover state_frame 1..={} with no gaps, got {frames:?}", md.total())
        });
        // …on consecutive simulation ticks.
        let consecutive = ticks_of.windows(2).all(|w| w[1] == w[0] + 1);
        f.check(consecutive, || {
            format!("air_full facing {facing:+}: the action's ticks must be consecutive, got {ticks_of:?}")
        });
        f.check(airborne, || {
            format!("air_full facing {facing:+}: the case must stay airborne for every frame of the action")
        });
        // And what it leaves into: an actionable aerial state, not landing.
        match ticks.iter().find(|t| t.label == "after") {
            Some(a) => {
                let fz = &a.state.fighters[0];
                println!(
                    "[SPECIAL_N] air_full facing {facing:+}: leaves into {:?}, grounded {}",
                    fz.state, fz.grounded
                );
                f.check(matches!(fz.state, State::Air) && !fz.grounded, || {
                    format!("air_full facing {facing:+}: the action must end in an actionable aerial state, it ended in {:?} (grounded {})", fz.state, fz.grounded)
                });
            }
            None => f.fail(format!(
                "air_full facing {facing:+}: the case never records the tick after the action"
            )),
        }
        // …and the short case is still the interrupted one, on purpose.
        let short =
            export::run_case_facing(CharacterId::Kestrel, MoveId::SpecialN, "air", facing).unwrap();
        let short_last = short
            .iter()
            .rfind(|t| {
                matches!(
                    t.state.fighters[0].state,
                    State::Attack {
                        id: MoveId::SpecialN,
                        ..
                    }
                )
            })
            .map(|t| t.row.state_frame)
            .unwrap_or(0);
        println!("[SPECIAL_N] air facing {facing:+}: interrupted by the ground on state_frame {short_last}");
        f.check(short_last < md.total(), || {
            format!("air facing {facing:+}: the short case is the interrupted one; it reached {short_last}")
        });
    }
    f.finish();
}

/// Honest labels, from the events that really happen.
#[test]
fn the_labels_name_the_real_events() {
    let mut f = Failures::new("event labels");
    for case in CASES {
        for facing in [1.0f32, -1.0] {
            let ticks =
                match export::run_case_facing(CharacterId::Kestrel, MoveId::SpecialN, case, facing)
                {
                    Ok(t) => t,
                    Err(e) => {
                        f.fail(format!("{case}: {e}"));
                        continue;
                    }
                };
            let tag = format!("kestrel {case} facing {facing:+}");
            let labels: Vec<&str> = ticks.iter().map(|t| t.label).collect();
            let in_move: Vec<&&str> = labels
                .iter()
                .zip(&ticks)
                .filter(|(_, t)| t.row.sample_phase == "after_step" && t.label != "after")
                .map(|(l, _)| l)
                .collect();
            println!("[SPECIAL_N] {tag}: {}", labels.join(" "));
            // Exactly one emission, and it is the tick the shot appears.
            let emissions = in_move.iter().filter(|l| ***l == "emission").count();
            f.check(emissions == 1, || {
                format!("{tag}: exactly one emission tick, got {emissions}")
            });
            let first_proj = ticks.iter().position(|t| t.projectile.is_some());
            let emission_at = ticks.iter().position(|t| t.label == "emission");
            f.check(first_proj.is_some() && first_proj == emission_at, || {
                format!("{tag}: the emission label must sit on the tick the projectile appears ({first_proj:?} vs {emission_at:?})")
            });
            // A real recoil beat, and no leftover claims.
            f.check(in_move.iter().any(|l| **l == "recoil"), || {
                format!("{tag}: the withdrawal must be labelled")
            });
            f.check(!labels.contains(&"windup"), || {
                format!("{tag}: nothing in this action is a windup -- the shot leaves on its first tick")
            });
            f.check(!in_move.iter().any(|l| **l == "active"), || {
                format!(
                    "{tag}: this action has no fighter hitbox on any frame, so no tick is `active`"
                )
            });
        }
    }
    // The other two owners keep their real active ticks and their state
    // frames exactly as they were.
    for (ch, startup) in [
        (
            CharacterId::Boulder,
            attacks::data(CharacterId::Boulder, MoveId::SpecialN).startup,
        ),
        (
            CharacterId::Viper,
            attacks::data(CharacterId::Viper, MoveId::SpecialN).startup,
        ),
    ] {
        let ticks = export::run_case_facing(ch, MoveId::SpecialN, "ground", 1.0).unwrap();
        let active: Vec<u32> = ticks
            .iter()
            .filter(|t| t.label == "active")
            .map(|t| t.row.state_frame)
            .collect();
        println!("[SPECIAL_N] {ch:?} keeps active on state_frame {active:?} (startup {startup})");
        f.check(active == vec![startup], || {
            format!("{ch:?}: its real fighter hitbox must still be labelled active on state_frame {startup}, got {active:?}")
        });
        f.check(
            !anim_directed::is_release_only(ch, MoveId::SpecialN),
            || format!("{ch:?}: keeps a real fighter hitbox, so it is not a release-only action"),
        );
    }
    // And an ordinary melee action is untouched.
    let ftilt =
        export::run_case_facing(CharacterId::Kestrel, MoveId::Ftilt, "ground", 1.0).unwrap();
    let md = attacks::data(CharacterId::Kestrel, MoveId::Ftilt);
    let active: Vec<u32> = ftilt
        .iter()
        .filter(|t| t.label == "active")
        .map(|t| t.row.state_frame)
        .collect();
    let want: Vec<u32> = (md.startup..md.startup + md.active + md.late_active).collect();
    f.check(active == want, || {
        format!("ftilt's active ticks are unchanged: {active:?} vs {want:?}")
    });
    f.finish();
}

/// **Is the release a contact?** Measured with the contract's own method.
///
/// The contract measures a contact piece by projecting its triangles onto
/// the XY fighting plane, taking the minimum distance from the reference
/// centre to that surface, and subtracting the reference radius. A
/// projectile action has no fighter hitbox, which is exactly why the
/// temptation is to substitute the piece's centroid and call a small number
/// a contact. The centroid proves nothing either way, and here it is *still*
/// outside the region (4.89u from a radius-4 circle) while the surface is
/// inside it -- the two answers genuinely differ.
///
/// The surface reaches: `signed_separation` is negative in every case and
/// both facings. It reaches because the chest line is stated absolutely
/// (see `lean_absolute`) instead of being added to four disagreeing base
/// poses, which is what used to leave the aerial release 1.33u short. The
/// numbers are pinned in both directions, so neither a regression nor a
/// silent improvement passes unreported.
#[test]
fn the_emission_contact_is_measured_with_the_contract_s_own_method() {
    let mut f = Failures::new("emission contact");
    // Measured with the delivered pose: chest 26 deg, elbow 45 deg, the
    // same in every condition. The surface reaches ~0.14u into the radius-4
    // region on the ground and out of shield, ~0.12u in the air. If a later
    // pose changes these, this says so instead of re-deriving them.
    let expected: [(&str, f32); 4] = [
        ("ground", -0.1355),
        ("air", -0.1152),
        ("air_full", -0.1152),
        ("from_shield", -0.1406),
    ];
    for (case, want) in expected {
        let j = match export::contact_json(CharacterId::Kestrel, MoveId::SpecialN, case) {
            Ok(j) => j,
            Err(e) => {
                f.fail(format!("{case}: {e}"));
                continue;
            }
        };
        for fc in j["facings"].as_array().expect("facings") {
            let facing = fc["facing"].as_f64().unwrap();
            let samples = fc["samples"].as_array().expect("samples");
            let Some(spawn) = samples.iter().position(|s| !s["projectile"].is_null()) else {
                f.fail(format!(
                    "{case} facing {facing:+}: no projectile in the case"
                ));
                continue;
            };
            let e = &samples[spawn]["emission_origin"];
            let (Some(mesh), Some(sep), Some(cent)) = (
                e["mesh_distance"].as_f64(),
                e["signed_separation"].as_f64(),
                e["contact_piece_centroid_gap"].as_f64(),
            ) else {
                f.fail(format!(
                    "{case} facing {facing:+}: the emission region must carry the contract's own measurement, got {e}"
                ));
                continue;
            };
            println!(
                "[SPECIAL_N] {case} facing {facing:+}: emission mesh_distance {mesh:.4}, signed_separation {sep:+.4} (radius {}), centroid {cent:.4} -- CONTACT {}",
                e["radius"],
                if sep <= 0.0 { "MET" } else { "NOT MET" }
            );
            // The measurement is the surface's, not the centroid's.
            f.check(mesh < cent - 0.5, || {
                format!("{case} facing {facing:+}: the surface measurement must be the piece's mesh ({mesh:.4}), clearly nearer than its centroid ({cent:.4})")
            });
            // And it is pinned at what it measures, in both directions.
            f.check((sep - want as f64).abs() < 0.01, || {
                format!("{case} facing {facing:+}: the measured separation moved: {sep:+.4} vs the recorded {want:+.4}")
            });
            // Stated plainly, because it is the finding: the piece's real
            // surface reaches into the emission region. If that ever stops
            // being true this fails rather than downgrading quietly.
            f.check(sep < 0.0, || {
                format!("{case} facing {facing:+}: the surface no longer reaches the emission region ({sep:+.4}) -- contact is NOT met, re-report it")
            });
            // The emission region is not the projectile: the shot has
            // already integrated a step by the time it is exported, and its
            // own separation is a different, larger number.
            let proj = samples[spawn]["projectile_separation"].as_f64();
            if let Some(ps) = proj {
                f.check(ps > sep + 4.0, || {
                    format!("{case} facing {facing:+}: the projectile's separation ({ps:.4}) must stay distinct from the emission's ({sep:+.4})")
                });
            }
        }
    }
    f.finish();
}

// ------------------------------------------------------------ invariants

/// Nothing causal moved. These are pins, not derivations: if a later batch
/// edits a table, this fails and says so instead of quietly re-deriving the
/// new value.
#[test]
fn the_presentation_batch_changed_no_simulation_value() {
    let mut f = Failures::new("simulation invariants");
    // The projectile itself.
    f.check(attacks::PROJECTILE_SPEED == 7.0, || {
        "projectile speed".into()
    });
    f.check(attacks::PROJECTILE_RADIUS == 4.0, || {
        "projectile radius".into()
    });
    f.check(attacks::PROJECTILE_DAMAGE == 5.0, || {
        "projectile damage".into()
    });
    f.check(attacks::PROJECTILE_LIFE == 60, || "projectile life".into());
    f.check(PROJECTILE_SPAWN_LOCAL_X == 14.0, || {
        "emission origin x".into()
    });

    // The three special_n rows, in full.
    for (ch, startup, active, endlag, no_melee, radius, damage) in [
        (
            CharacterId::Kestrel,
            9u32,
            1u32,
            22u32,
            true,
            2.0f32,
            0.0f32,
        ),
        (CharacterId::Boulder, 14, 1, 28, false, 2.0, 0.0),
        (CharacterId::Viper, 8, 1, 18, false, 2.0, 0.0),
    ] {
        let md = attacks::data(ch, MoveId::SpecialN);
        let got = (
            md.startup,
            md.active,
            md.endlag,
            md.no_melee,
            md.hitbox.radius,
            md.hitbox.damage,
        );
        let want = (startup, active, endlag, no_melee, radius, damage);
        f.check(got == want, || {
            format!("{ch:?} special_n row moved: {got:?} vs {want:?}")
        });
        // The commitment: 32 ticks for Kestrel, and each character's own.
        f.check(md.total() == startup + active + endlag, || {
            format!(
                "{ch:?} special_n total {} vs {}",
                md.total(),
                startup + active + endlag
            )
        });
    }
    let k = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    f.check(k.total() == 32, || {
        format!("Kestrel's commitment is 32 ticks, got {}", k.total())
    });
    for sf in 0..k.total() {
        f.check(k.hitbox_at(sf).is_none(), || {
            format!("Kestrel special_n must have no fighter hitbox on state_frame {sf}")
        });
    }
    for ch in [CharacterId::Boulder, CharacterId::Viper] {
        let md = attacks::data(ch, MoveId::SpecialN);
        f.check(md.hitbox_at(md.startup).is_some(), || {
            format!("{ch:?} special_n keeps its own fighter hitbox")
        });
    }

    // Scale, bones and the hurt capsule. The bone lengths are measured off
    // the shipped skeleton in its rest pose rather than read from a
    // constant, so this pins what the rig actually is.
    let m = build(CharacterId::Kestrel);
    let rig = &m.rig;
    let rest = rig.world(&Pose::rest(rig.bones.len()), &Xf::IDENTITY);
    let seg = |a: &str, b: &str| -> f32 {
        let (pa, pb) = (
            rest[rig.bone(a).unwrap_or_else(|| panic!("{a}"))].t,
            rest[rig.bone(b).unwrap_or_else(|| panic!("{b}"))].t,
        );
        ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2) + (pb.z - pa.z).powi(2)).sqrt()
    };
    let upper = seg("upper_arm_r", "forearm_r");
    let fore = seg("forearm_r", "hand_r");
    println!(
        "[SPECIAL_N] measured skeleton: upper_arm {upper:.3} forearm {fore:.3} (chain {:.3})",
        upper + fore
    );
    // Measured off the shipped art spec, not copied from a constant: this
    // is what Kestrel's arm actually is, and it is why the palm cannot
    // reach the emission origin.
    f.check((upper - 5.0).abs() < 1e-4, || {
        format!("upper_arm is {upper}")
    });
    f.check((fore - 4.5).abs() < 1e-4, || format!("forearm is {fore}"));
    let ch = CharacterId::Kestrel.data();
    f.check(ch.height == 30.0 && ch.half_width == 9.0, || {
        format!(
            "Kestrel's capsule moved: height {} half_width {}",
            ch.height, ch.half_width
        )
    });
    f.finish();
}

// ------------------------------------------------------------ diagnostic

/// The whole timeline, printed. Not a pass/fail: this is the table the
/// review reads next to the frames.
///
/// `cargo test --test special_n_presentation -- --ignored --nocapture`
#[test]
#[ignore]
fn diag_the_whole_discharge_timeline() {
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    println!(
        "special_n: startup {} active {} endlag {} total {} no_melee {} | origin ({}, 15) | first integrated x {}",
        md.startup,
        md.active,
        md.endlag,
        md.total(),
        md.no_melee,
        PROJECTILE_SPAWN_LOCAL_X,
        PROJECTILE_SPAWN_LOCAL_X + attacks::PROJECTILE_SPEED
    );
    for &grounded in &[true, false] {
        println!("--- grounded={grounded}");
        for sf in 0..md.total() {
            let g = geom(grounded, sf);
            let beat = anim_directed::release_beat(&md, sf);
            println!(
                "sf{sf:>2} {:>9} palm=({:+7.3},{:+7.3}) gap={:6.3} off_line={:.4} elbow={:6.1} lean={:+6.1} root_y={:+6.3}",
                format!("{beat:?}"), g.palm[0], g.palm[1], g.gap, g.off_line, g.elbow, g.lean, g.root_y
            );
        }
    }
}
