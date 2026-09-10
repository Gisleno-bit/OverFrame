# Reviews against evidence

One file per reviewed source commit: `docs/art/reviews/<source_sha>.md`,
written by the art reviewer (ChatGPT) after reading the matching
`builds/<source_sha>/attempt-*/manifest.json` and images on the
`visual-evidence` branch. Each observation follows the YAML block in
`docs/art/EXCHANGE.md` (`piece_id`, `expected`, `observed`, `change`,
`acceptance`, `verdict`). Corrections are applied on `main` and closed by a
new run with the same cameras and fixtures.
