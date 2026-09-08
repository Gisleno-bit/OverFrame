//! OVERFRAME entry point.
//!
//! With the default `gui` feature this launches the game window. Command-line
//! shortcuts make testing online play with two instances quick:
//!
//! ```text
//! overframe                 # menu
//! overframe --host          # host on the configured port (default 7777)
//! overframe --host 7800     # host on a specific port
//! overframe --join C0G81-5N2E1     # join by room code
//! overframe --join 192.168.1.37:7777   # join by ip:port
//! overframe --versus kestrel,viper,tidegate   # straight into a local match
//! overframe --demo boulder,kestrel,meridian   # attract-mode demo
//! overframe --demo --screenshot shot.png --at 180   # save a frame, quit
//! overframe --export-glb kestrel kestrel.glb   # rig for Blender (docs/ART_PIPELINE.md)
//! overframe --anim viper        # animation viewer: every state and move
//! ```

#[cfg(feature = "gui")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = overframe::render::LaunchOpts::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--host" => {
                let port = args
                    .get(i + 1)
                    .and_then(|p| p.parse::<u16>().ok())
                    .unwrap_or_else(|| overframe::config::Settings::load_or_create().host_port);
                opts.host = Some(port);
                if args
                    .get(i + 1)
                    .and_then(|p| p.parse::<u16>().ok())
                    .is_some()
                {
                    i += 1;
                }
            }
            "--join" => {
                if let Some(code) = args.get(i + 1) {
                    opts.join = Some(code.clone());
                    i += 1;
                } else {
                    eprintln!("--join needs a room code or ip:port");
                    std::process::exit(2);
                }
            }
            "--versus" | "--demo" => {
                let flag = args[i].clone();
                let spec_text = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned();
                let spec =
                    match overframe::render::MatchSpec::parse(spec_text.as_deref().unwrap_or("")) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("{flag}: {e}");
                            std::process::exit(2);
                        }
                    };
                if spec_text.is_some() {
                    i += 1;
                }
                if flag == "--versus" {
                    opts.versus = Some(spec);
                } else {
                    opts.demo = Some(spec);
                }
            }
            "--anim" => {
                let spec = args.get(i + 1).cloned().unwrap_or_else(|| "kestrel".into());
                let mut parts = spec.split(':');
                let name = parts.next().unwrap_or("kestrel");
                let clip = parts.next().and_then(|c| c.parse::<usize>().ok());
                let frame = parts.next().and_then(|c| c.parse::<u32>().ok());
                let id = overframe::sim::roster::CharacterId::ALL
                    .iter()
                    .copied()
                    .find(|c| c.name().eq_ignore_ascii_case(name));
                match id {
                    Some(id) => opts.anim = Some((id, clip, frame)),
                    None => {
                        eprintln!("--anim: unknown character '{name}' (kestrel, boulder, viper)");
                        std::process::exit(2);
                    }
                }
                if args.get(i + 1).is_some() {
                    i += 1;
                }
            }
            "--export-glb" => {
                let what = args.get(i + 1).cloned().unwrap_or_default();
                let out = args
                    .get(i + 2)
                    .cloned()
                    .unwrap_or_else(|| format!("{what}.glb"));
                match export_glb(&what, &out) {
                    Ok(msg) => {
                        println!("{msg}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("--export-glb: {e}");
                        std::process::exit(2);
                    }
                }
            }
            "--record" => {
                // --record <dir> [<from> <count>]
                let dir = args.get(i + 1).cloned().unwrap_or_else(|| "frames".into());
                let from = args.get(i + 2).and_then(|a| a.parse::<u64>().ok()).unwrap_or(60);
                let count = args.get(i + 3).and_then(|a| a.parse::<u64>().ok()).unwrap_or(180);
                opts.record = Some((dir, from, count));
                i += if args.get(i + 3).is_some() { 3 } else { 1 };
            }
            "--screenshot" => {
                let path = args.get(i + 1).cloned().unwrap_or_else(|| {
                    eprintln!("--screenshot needs a file path");
                    std::process::exit(2);
                });
                let at = opts.screenshot.as_ref().map(|s| s.1).unwrap_or(120);
                opts.screenshot = Some((path, at));
                i += 1;
            }
            "--at" => {
                let at = args
                    .get(i + 1)
                    .and_then(|a| a.parse::<u64>().ok())
                    .unwrap_or(120);
                let path = opts
                    .screenshot
                    .as_ref()
                    .map(|s| s.0.clone())
                    .unwrap_or_else(|| "screenshot.png".into());
                opts.screenshot = Some((path, at));
                i += 1;
            }
            "-h" | "--help" => {
                println!(
                    "overframe [--host [port]] [--join <code|ip:port>]\n\
                     \x20         [--versus p1,p2,stage,pal1,pal2] [--demo [spec]]\n\
                     \x20         [--screenshot file.png --at N]\n\
                     Characters: kestrel, boulder, viper.  Stages: lattice, meridian, tidegate.\n\
                     Settings file: {}",
                    overframe::config::Settings::path().display()
                );
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    overframe::render::launch(opts);
}

/// Write a fighter's rig or a stage's geometry as `.glb` for Blender.
#[cfg(feature = "gui")]
fn export_glb(what: &str, out: &str) -> Result<String, String> {
    use overframe::model::{characters, gltf_io, stage3d};
    use overframe::sim::roster::CharacterId;
    use overframe::sim::stage::{Stage, StageId};
    let w = what.to_ascii_lowercase();
    if let Some(id) = CharacterId::ALL
        .iter()
        .copied()
        .find(|c| c.name().eq_ignore_ascii_case(&w))
    {
        let m = characters::build(id);
        let bytes = gltf_io::export_rig(&m.rig, m.palette(0));
        std::fs::write(out, &bytes).map_err(|e| e.to_string())?;
        return Ok(format!(
            "wrote {out}: {} ({} bones, {} triangles). Open in Blender; see docs/ART_PIPELINE.md",
            id.name(),
            m.rig.len(),
            m.rig.triangle_count()
        ));
    }
    if let Some(id) = StageId::ALL.iter().copied().find(|s| {
        let n = s.name().replace(' ', "");
        n.eq_ignore_ascii_case(&w) || n.trim_start_matches("The").eq_ignore_ascii_case(&w)
    }) {
        let stage = Stage::by_id(id);
        let model = stage3d::build(&stage);
        // Pack the stage as a two-node rig: slab geometry + far decor.
        let mut rig = overframe::model::Rig::new();
        rig.add("slab", "", overframe::model::math3::V3::ZERO);
        rig.add("decor", "slab", overframe::model::math3::V3::ZERO);
        rig.attach("slab", model.near);
        rig.attach("decor", model.far);
        let bytes = gltf_io::export_rig(&rig, &model.palette);
        std::fs::write(out, &bytes).map_err(|e| e.to_string())?;
        return Ok(format!(
            "wrote {out}: {} ({} triangles)",
            id.name(),
            rig.triangle_count()
        ));
    }
    Err(format!(
        "unknown '{what}'. Characters: kestrel, boulder, viper. Stages: lattice, meridian, tidegate."
    ))
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!(
        "overframe was built without the `gui` feature.\n\
         Build the playable game with:  cargo run --release --features gui\n\
         (the `gui` feature is on by default: `cargo run --release`)"
    );
}
