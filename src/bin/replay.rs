//! `overframe-replay` — render the scripted demo match to an animated GIF (and
//! optionally a PNG still) with no display or GPU. Used for docs/CI previews.
//!
//! Usage:
//!   overframe-replay [out.gif] [width] [height]
//!   overframe-replay --png out.png <tick> [width] [height] [--training]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(|s| s.as_str()) == Some("--diag") {
        let mut gs = overframe::demo::match_state();
        for frame in 0..overframe::demo::DEMO_LEN {
            let inp = overframe::demo::inputs(&gs, frame);
            gs.step(&inp);
            if frame % 10 == 0 || (168..=180).contains(&frame) {
                let a = &gs.fighters[0];
                let b = &gs.fighters[1];
                println!(
                    "f{:>3} P1 x={:>7.1} st={:<9} | P2 x={:>7.1} y={:>6.1} {:>4.0}% stk={} st={:?}",
                    frame,
                    a.pos.x,
                    format!("{:?}", a.state),
                    b.pos.x,
                    b.pos.y,
                    b.percent,
                    b.stocks,
                    b.state
                );
            }
        }
        return;
    }

    if args.get(1).map(|s| s.as_str()) == Some("--png") {
        let path = args.get(2).cloned().unwrap_or_else(|| "frame.png".into());
        let tick: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(120);
        let w: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(640);
        let h: u32 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(360);
        let training = args.iter().any(|a| a == "--training");
        match overframe::headless::render_demo_png(&path, w, h, tick, training) {
            Ok(()) => println!("wrote {path} ({w}x{h}) at tick {tick}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "overframe-demo.gif".into());
    let w: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(640);
    let h: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(360);

    match overframe::headless::render_demo_gif(&path, w, h) {
        Ok(()) => println!("wrote {path} ({w}x{h})"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
