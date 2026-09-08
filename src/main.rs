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

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!(
        "overframe was built without the `gui` feature.\n\
         Build the playable game with:  cargo run --release --features gui\n\
         (the `gui` feature is on by default: `cargo run --release`)"
    );
}
