# The `.tdraw` format

A `.tdraw` file is the drawing's `Document` as JSON (`Document::to_json`
in `crates/core/src/document.rs`), with `format_version` at the top
level. Fonts the drawing carries are embedded in it.

## Versions

| Version | First written by | Change |
|---|---|---|
| 0 | TraceDraw 0.4 | Files written before versions were recorded (no `format_version`). |
| 1 | TraceDraw 0.5 | `format_version` recorded; the layout is the same as 0. |

Save As offers every version, newest first ("v1 (TraceDraw 0.5)", from
`TDRAW_RELEASES`). An older one is written by taking the current layout
down one step at a time (`Document::to_json_version`; `DOWN_STEPS[i]`
takes version `i + 1` to `i`). A drawing keeps the version it was opened
in or last saved as, so Save writes the same version again.

## The promise

- Every `.tdraw` file TraceDraw has written opens in every later
  TraceDraw. A file of an older version is brought up to date on
  reading, one step per version (`migrate`; `STEPS[i]` takes version `i`
  to `i + 1`).
- A new field always comes with a default, so files without it read as
  before; adding one does not raise the version.
- A field renamed, moved or given another meaning raises
  `TDRAW_FORMAT_VERSION` and adds the step that rewrites older files.
- A file from a newer TraceDraw opens as far as this one knows it: fields
  it does not know are left out (saving it again drops them).

## Checks

- `document::tests::tdraw_files_carry_their_format_version_and_older_ones_open`
- `document::tests::tdraw_files_can_be_written_in_an_older_format_version`
