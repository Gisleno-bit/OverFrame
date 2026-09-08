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
            "-h" | "--help" => {
                println!(
                    "overframe [--host [port]] [--join <code|ip:port>]\n\
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
