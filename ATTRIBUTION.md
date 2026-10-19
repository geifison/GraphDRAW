# Attribution

Every asset bundled with GraphDraw, with its author, source and license.
Code dependencies (Rust crates) keep their own licenses, recorded in
their packages; `cargo tree` lists them from `Cargo.lock`.

| Asset | Path | Author and source | License |
|---|---|---|---|
| Selawik Regular and Bold (interface font) | `apps/graphdraw/assets/fonts/` | Copyright 2015 Microsoft Corporation, the open-source Selawik project | SIL Open Font License 1.1, see `Selawik-OFL.txt` next to the fonts |
| Quicksand (variable, test font) | `crates/text/testdata/` | Copyright 2011 The Quicksand Project Authors, from the google/fonts collection | SIL Open Font License 1.1, see `Quicksand-OFL.txt` next to the font |
| Fonts of the browser build (bundled into the module and placed in the published page's font folder when it is built; not stored in this repository) | `web/fonts.txt`, `apps/graphdraw/build.rs` | Liberation, DejaVu, Noto, TeX Gyre and the google/fonts families listed in `web/FONTS.md` | Their own free licences (SIL Open Font License 1.1, Apache License 2.0, Ubuntu Font Licence 1.0, GUST Font License, Bitstream Vera), copied into the font folder's `licences/` |
| Application icon (PNG sizes and `.ico`) | `assets/icon/` | Made for GraphDraw | MIT OR Apache-2.0 |
| Microsoft Store tiles | `packaging/windows/msix/Assets/` | Made for GraphDraw from the application icon | MIT OR Apache-2.0 |
| Tool, toolbar and panel icons | painted in code, `apps/graphdraw/src/ui/icons.rs` | Made for GraphDraw | MIT OR Apache-2.0 |
| Default colour palette (colour names and values) | `apps/graphdraw/src/palette.rs` | GraphDraw contributors; common process colour tints with descriptive names | MIT OR Apache-2.0 |
| Spell-check core word list | `apps/graphdraw/lang/core-words.txt` | Made for GraphDraw | MIT OR Apache-2.0 |
| Hyphenation patterns | `crates/text/src/hyphen.rs` | Made for GraphDraw (a small Liang-style set) | MIT OR Apache-2.0 |
| Interface translations | `apps/graphdraw/lang/*.toml` | Made for GraphDraw | MIT OR Apache-2.0 |
| Sample drawing | `sample.gdr` | Made for GraphDraw | MIT OR Apache-2.0 |
| Screenshots | `docs/images/` | GraphDraw itself, captured with `scripts/visual/readme_shots.sh` from drawings made for GraphDraw by `apps/graphdraw-cli/examples/showcase.rs`; their text is set in Inter and Lora (SIL Open Font License 1.1), fonts installed on the capture machine and not bundled | MIT OR Apache-2.0 |

Not bundled on purpose: licensed spot colour libraries, third-party
clipart, templates, fonts other than the ones above, and colour profiles
other than the ones a user loads. Test files that belong to someone else
are never committed.
