# AGENTS.md: guide for AI agents and contributors

GraphDraw is an open-source vector editor in **Rust only**: native egui/eframe
desktop app and the same app compiled to WebAssembly for the browser. No
Electron, Tauri or webview.
The product name is **GraphDraw** in prose; machine names are `graphdraw`
(`graphdraw-core`, `graphdraw-cdr`, `.gdr` files).

## Golden rules

1. **Everything is a command.** New user-visible behaviour is a variant of
   `graphdraw_core::Command`, applied through `Engine::run`. The UI only builds
   commands and reads the document; it never mutates it directly.
2. **Never panic on input.** Non-test code does not use `unwrap`, `expect`,
   `panic!`, `todo!` or raw indexing on data that came from a file or the
   user. Return `Result`, log a warning, skip the object. Opening any file
   must always yield a document.
3. **Clean-room and independence.** GraphDraw stands on its own: never
   name other graphics editors or their makers anywhere in the repo (code,
   comments, docs, commit messages, packaging, file names), and never
   describe GraphDraw or a feature as modelled on another program.
   Features carry our own names: generic terms (Fill, Blend, Contour,
   Envelope, Weld) are fine, distinctive product or feature names of other
   software are never used. File formats go by their usual names (`.cdr`,
   `.ai`, `.psd`, a format owner's name in a file dialog filter, a header
   string a format requires) only to describe compatibility; copyright
   notices that a license requires stay in `ATTRIBUTION.md`. Never decompile any program and never use
   decompiled code or dumps, not even in conversations with AI tools.
   Format readers come from public format notes and from inspecting files
   we own; never port code from libcdr or any other implementation.
   Record each piece of format knowledge, its source and date in
   `CLEANROOM.md` and the details in `docs/cdr-format.md`. Bundle only
   original or openly licensed assets and list them in `ATTRIBUTION.md`.
4. **Layering is enforced by review.** `core` has no workspace deps; `cdr`,
   `text` and `render` depend on `core` only; `io` depends on `core` and
   `render` (rasterised fills in exports); egui stays in `apps/`.
5. **Tests are the gate.** Every change comes with tests. Format code
   gets round-trip and malformed-input tests.
6. **Units.** Millimetres everywhere in the model. CDR files use
   1/254000 inch (= 0.0001 mm). Points only appear in text sizes.
7. **Code and comments in English.** Commit messages in English.
8. **No em dashes** in prose, code or commits.

## Workspace map

```text
crates/core   geometry.rs, document.rs, command.rs, engine.rs, style.rs,
              color.rs, nodes.rs, shaping.rs, effects.rs, id.rs
crates/cdr    container.rs (RIFF/ZIP), riff.rs (chunk tree), parse.rs
crates/text   font database, shaping, outlines
crates/render CPU rasteriser
crates/io     svg.rs, pdf.rs, lib.rs (native format)
apps/graphdraw    main.rs, app.rs, interaction.rs, canvas.rs, ops.rs,
                  tools.rs, tools2.rs, ui/ (menus, toolbar, toolbox,
                  dockers, palette, status, dialogs, icons)
apps/graphdraw-cli
```

## Visual checks

The app can be driven headlessly: start `Xvfb :99`, run the debug binary
with `DISPLAY=:99 LIBGL_ALWAYS_SOFTWARE=1`, send pointer and key events
with python-xlib (XTest) and capture with ImageMagick `import`. See
`scripts/visual/` for the driver and example scripts; `run.sh` starts
the app at interface size 100 % (one interface point per pixel on Linux),
the size the scripts' coordinates are taken at. Look at the PNGs
before claiming a UI change works. `scripts/visual/smoke_edit.py` and
`scripts/visual/monkey.py` (random actions, `MONKEY_SEED`) must leave the
application running with no panic in the log after any interaction change.
`scripts/visual/window_check.sh <binary>` (Xvfb, openbox, xprop) checks
that the main window can be maximized and resized after the splash
screen; CI runs it on the Linux build.

## Browser build

The app must keep compiling for `wasm32-unknown-unknown`. File dialogs and
file access go through `apps/graphdraw/src/files.rs` (never `rfd` or
`std::fs` directly in UI code); anything that needs a desktop (processes,
temporary folders, system clipboard images) checks `files::WEB` or is
`#[cfg(not(target_arch = "wasm32"))]`. `std::time::Instant`,
`SystemTime::now`, `std::env::temp_dir` and `std::process::id` panic in a
browser: use `web_time` or avoid them. See `docs/behavior/web.md`.

## Feature status

`docs/features.md` is the measure of progress: every tool and menu item
with its status. Move a row to "works" only with a test or a checked file
behind it, and never add a feature without updating the row.

## Workflow

- `cargo test --workspace` before every commit.
- Commits carry a `Signed-off-by:` line (DCO, see `CONTRIBUTING.md`).
- `cargo build -p graphdraw` and `cargo clippy --workspace` must stay
  warning-free (CI runs clippy with `-D warnings` on the library crates,
  on the pinned toolchain 1.97.0; use the same version locally so the
  lint set matches).
- Keep `docs/roadmap.md` honest: a feature is "done" when it has tests
  and works on a real file, not when the menu item exists.

## Icons

- **A changed icon changes everywhere.** A command has one picture in the
  toolbox, the bars, the property bar, the menus, the context menus, the
  dockers and the hints. Every surface draws through `svg_icons::paint`
  and the one map in `apps/graphdraw/src/ui/svg_icons.rs` (`svg_for`);
  never add a second icon table or a direct SVG draw for one screen.
- To replace an icon, change its entry in that map (all of `TOOLS`,
  `ACTIONS`, `PICS`, `GLYPHS` and `KEYS` that name it), drop the old file
  from `FILES` when nothing else uses it, and update `docs/icon-map.md`.
  Colour pictures live in `assets/icons/color/` and are mapped as
  `color/<stem>`; the test `a_changed_icon_changes_everywhere` fails while
  any table still names the old drawing of a stem that has a colour one.
- Check every place the command shows (search the enum variant and its
  menu keys) before calling an icon change done.

## Documentation duties

- A new tool or effect gets a page in `docs/behavior/` with its defaults,
  shortcuts and the formulas it uses, before its row in
  `docs/features.md` moves.
- User-visible strings go through the i18n table (`docs/i18n.md`), never
  as bare literals in the UI.
- Acceptance criteria in `docs/acceptance.md` are tests; when one changes,
  change the test.
