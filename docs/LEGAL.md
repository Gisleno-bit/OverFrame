# Legal strategy & originality policy

> This document explains why OVERFRAME can exist safely and the rules we follow
> to keep it that way. It is an engineering/policy document, **not legal advice**.
> If in doubt about a specific contribution, ask before merging.

## The core principle

Copyright protects the **expression** of a work, not the **ideas, systems, rules,
methods or mechanics** behind it. For a video game that means:

| Protected (never copy) | Not protected (free to reproduce) |
|---|---|
| Characters, their names, designs, models, textures, voices, animations | Game mechanics: percent/knockback damage, hitstun, recovery, L-cancel, wavedash, teching |
| Stage names, art, layouts-as-art, music | The *idea* of a layout (e.g. "a main platform with a few floating platforms") |
| Music and sound effects | Rules and modes (stocks, timer, versus/training) |
| Source code (including **decompiled** code) | Mathematical formulas and physics behaviour |
| Logos, trademarks, UI art | Input schemes and control conventions |

This is why other commercial platform fighters that recreate the same genre and
mechanics — building their own characters, art and code — have shipped without
issue. OVERFRAME takes the same route and adds two extra layers of safety: it is
**free** (no commercial harm argument) and **open source** (fully auditable and
transparently original).

## What we do

- **Original names and brands.** The game (*OVERFRAME*), the fighter (*Kestrel*),
  and the stage (*The Lattice*) are our inventions. No character, stage, item or
  move carries a name from another game.
- **Original code, from scratch.** Every line here is written for this project.
  We do **not** read, port, translate, or reference decompiled or disassembled
  code from any other game.
- **Original assets.** All visuals are drawn procedurally in our own code; any
  future art, music and SFX will be created originally or sourced under a
  compatible license with attribution.
- **Self-contained.** The game depends on **no** external ROM, ISO, or data file
  from any other game. It is a standalone product.
- **Mechanics from public knowledge only.** Where we reproduce a mechanic (e.g.
  a knockback formula), it comes from the community's *public documentation of how
  the mechanic behaves*, not from another game's data or code. Our per-move frame
  data and tuning values are our own, chosen for feel (see `docs/DESIGN.md`).

## The clean-room rule (for contributors)

To keep originality provable, every contribution must follow this:

1. **Never** copy, paste, port, or transcribe assets, data tables, or code from
   another game or from a decompilation/disassembly of one.
2. **Never** add values "ripped" from another game's files (character stats,
   exact frame data, coordinates from a real stage, etc.). Tuning values must be
   authored here and justified by feel/tests.
3. Reproducing a **mechanic's behaviour** described in public, community-written
   documentation is fine. Reproducing an **asset or dataset** is not.
4. If a mechanic can only be matched by copying protected data, we approximate it
   with our own numbers instead.
5. When in doubt, open an issue and discuss before writing the code.

## Naming & presentation

- OVERFRAME is described as an original homage / community project, never as a
  clone, port, remake, or substitute "for" a specific game.
- We do not use another game's trademarks, logos, character names or slogans in
  the game, its store pages, its marketing, or its repository.

## If a rights-holder ever makes contact

Because the project uses no protected assets or code, a takedown would have no
valid target. Should any concern arise, the maintainers will engage in good
faith, and the open, auditable history makes the project's originality easy to
demonstrate.
