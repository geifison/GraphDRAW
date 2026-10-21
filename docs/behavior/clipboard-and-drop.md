# Clipboard, paste and dropped files

Copy, Cut and Paste go through the system clipboard, so objects move
between GraphDraw windows with every property and between GraphDraw and
other programs in the formats those programs understand. Files dragged
from the file manager onto the window are imported where they fall.

## Copy (Ctrl+C) and Cut (Ctrl+X)

The selection stays on the internal clipboard and goes on the system
clipboard in several formats at once; each program picks the one it
reads:

| Format | For | Notes |
|---|---|---|
| GraphDraw Objects (private) | other GraphDraw windows | versioned JSON of the objects and the object styles they link to; symbol instances are expanded into groups |
| `image/svg+xml` | vector editors, browsers | the same SVG as File > Export SVG of the selection |
| Text | text editors, web tools | the SVG markup; Options > Edit can turn it off |
| PNG and a plain bitmap | office programs, image editors | screen resolution (96 dpi), longest side at most 2048 pixels; transparent parts on white in the plain bitmap; Options > Edit can turn it off |

When everything together passes 64 MiB the picture is left out first,
then the SVG text, then the SVG, then the private format (the internal
clipboard still pastes in this window).

On Windows every format is published together. Other desktops hold one
flavour (the SVG text, or the picture when the text is off); the browser
build hands the SVG text to the page.

## Paste (Ctrl+V)

Paste takes the best format the system clipboard offers, in this order,
and falls back to the next when one cannot be read:

1. GraphDraw Objects: the objects as they were, with their styles.
2. SVG: editable shapes.
3. Files copied in the file manager: each imported as by File > Import.
4. PNG: a bitmap, transparency kept.
5. A plain bitmap (a picture copied in a browser, a screenshot).
6. HTML with an embedded picture (`<img src="data:image/...;base64,...">`).
7. SVG markup carried as text: editable shapes.
8. Plain text: an artistic text at the page centre.

The internal clipboard is used only while the system clipboard still holds
the copy made in this window (its change counter is unchanged, or on
desktops without one the same SVG text), or holds nothing. Anything copied
in another program afterwards wins.

Pasted objects end up selected. A paste is one undo step, with its
placement and, for text, its fill and outline.

### Where it lands

Options > Workspace > Edit > Copy and paste, "When pasting with Ctrl+V":

- **At the copied position** (default): objects keep their coordinates;
  pictures and text from other programs go to the page centre.
- **At the centre of the view**: the pasted objects are centred in the
  visible area.

Edit > Paste in View (Ctrl+Shift+V) always centres them in the view.

### Paste Special

Edit > Paste Special lists the forms the clipboard holds and greys out the
others: GraphDraw objects, SVG, picture (PNG or bitmap), files copied in
the file manager, text. The chosen form is used even when a better one is
there (SVG markup pasted as text, for example).

### Content from outside

- Pasted SVG never reads files or the network: pictures inside it load
  only from embedded `data:` URLs. Scripts and event attributes are not
  run. SVG text over 64 MiB or with more than 500 000 elements is refused.
- HTML pictures are taken only from embedded `data:image/` URLs; remote
  and file addresses are never fetched.
- Pictures larger than 16 384 pixels on a side, or needing over 1 GiB to
  decode, are refused. The private format is refused over 256 MiB or when
  it comes from a newer GraphDraw version.

## Dropped files

- While files are dragged over the window, the canvas shows a dashed blue
  frame and "Drop to import" (with the number of files when there are
  several).
- Each dropped file is imported and embedded as by File > Import, as its
  own undo step. The first is centred where it was dropped (the view
  centre when dropped outside the canvas); each next one a Duplicate
  offset further, so they do not cover each other. Everything imported
  ends up selected.
- Bitmaps remember their file (Bitmaps > Update from Link reads it again);
  the pixels are always embedded, so moving or deleting the file changes
  nothing in the drawing.
- Native drawings (`.gdr`) can be dropped and imported too.
- A picture dragged from a web page arrives when the browser hands the
  system a file; when it does not, copy the picture and paste it instead.
- In the browser build dropped files are read in the background and each
  is imported when it has arrived.

## Errors

| Case | Message |
|---|---|
| A file no reader understands (dropped, pasted from the file manager or chosen in File > Import), or a picture that does not decode | "File or format not supported!" with the file names, one alert for a whole drop |
| The clipboard holds only formats no reader understands | "Format not supported. Try another format or import the file." |
| The clipboard is empty | nothing happens |

## Shortcuts

| Action | Shortcut |
|---|---|
| Copy | Ctrl+C |
| Cut | Ctrl+X |
| Paste | Ctrl+V |
| Paste in View | Ctrl+Shift+V |
| Paste Special | Edit menu |
| Import | File > Import (Ctrl+I) |

Tests: `clip_native`, `clip_offer`, `clip_system`, `clip_tests`,
`drop_import` and `svg_import::untrusted_tests`.
