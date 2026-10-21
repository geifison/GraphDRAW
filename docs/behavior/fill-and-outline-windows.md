# Edit Fill and Outline Pen windows

Two floating windows edit the fill (F11, or a double click on the status
bar's fill) and the outline (F12, the Outline Pen tool, or a double click
on the status bar's outline) of the selected objects. With nothing
selected they edit the fill and outline new objects get (the Change
Defaults question follows OK, as for any change made with nothing
selected).

## Floating, with a live preview

- The windows do not block the program: they open at the top right of
  the screen, move by their title bar and keep their place while open.
  The drawing can be zoomed, scrolled and selected while they are open.
- While a window has changes, the drawing shows them on the objects it
  edits. The document itself changes only with OK, as one undo step (fill
  and overprint together; the outline of every object together). Cancel,
  Esc or the close button leave the drawing as it was.
- A new selection reloads a window that has no changes yet; a window with
  changes keeps editing the objects it was opened for.
- With the pointer over a window, its keys are not drawing shortcuts:
  Enter is OK, Esc is Cancel and Delete removes the chosen gradient node.
- "In the docker" opens the same settings in the Properties docker.

## Edit Fill

| Part | What it does |
|---|---|
| Modes | No fill, Uniform, Gradient, Vector pattern, Bitmap pattern, Two-color pattern, Texture; Mesh when the fill is a mesh already; PostScript shown disabled |
| Uniform | Fill transparency 0..100% (the fill alone; multiplies with the object transparency; saved, drawn and exported as fill opacity in SVG and PDF); colour picker inline (hue ring with saturation and brightness square, model list with sliders, hexadecimal, palettes with search, eyedropper); Overprint |
| Gradient types | Linear, Radial, Conical, Square, Shaped |
| Ramp | Double click on the band: new node with the gradient's colour there; double click on a middle node: removes it (the ends stay); drag a node to move it; Delete removes the chosen node; a palette colour dragged from the picker onto the band becomes a node |
| Node | Colour (pop-up picker), position (0..100%) |
| Presets | The 29 built-in gradients |
| Matrix | Angle with dial (Shift: 15 degree steps) for linear, conical and square; centre X and Y (-100..100%) for radial, conical and square; repeat mirrored or plain with the cycle count; edge pad (0..49%); bilinear; invert colours; Reverse (the first node goes to the end) |
| Patterns | Vector: "Use copied objects" makes the tile from the clipboard (Ctrl+C); Bitmap: "Choose image..."; Two colours: 8 tiles, front and back colours; tile width (and height for vector tiles) |
| Texture | Clouds, Marble, Wood, Noise, Granite; seed with Shuffle; noise scale (mm); detail (1..8 layers), contrast, resolution (ppi); front and back colours |
| Tile layout | Every pattern: scale, rotation, resolution (bitmap and two colours), row and column offsets, mirror alternate tiles, transform with object (off: fixed to the page); two-colour cells clickable (a built-in tile becomes Custom) |
| Samples | New fill above the original one, over a chequerboard |

## Outline Pen

| Part | What it does |
|---|---|
| Has outline | Off removes the outline |
| Colour | Pop-up picker with eyedropper; Transparency 0..100 % (the outline's own, on top of the object's) |
| Width | Typed with the unit inside the field (pt, mm, cm, in, px, m; 3 decimals), Presets (hairline, 0.1 to 36 pt), Hairline (0.0762 mm) |
| Line style | Solid, Dashed (4, 2), Long dash (8, 3), Dotted (1, 2, round caps), Dash-dot (5, 2, 1, 2), the user's styles; Edit... opens the line style editor (2..48 cells, one cell = one width; Use, Add, Replace, Close); user styles are kept in the settings |
| Corners | Miter, Round, Bevel; miter limit 1..100 (default 4), for mitred corners |
| At corners | Standard, Align (a whole dash through every corner), Fixed (the pattern starts again at every corner); dashed lines only |
| Caps | Flat, Round, Extended |
| Arrowheads | Start and end with thumbnails (the presets and the custom heads); Options... has each end's size (5..5000 %), rotation (degrees), mirror and offset (a length, out past the end; negative goes back along the curve), Reset, and swaps start and end (with their options) |
| Calligraphic pen | Nib view (drag the tip to turn, the side to flatten), stretch 1..100%, angle 0..360 degrees, Default |
| Variable width | Uniform, Taper end, Taper start, Taper both, Bulge; the shape follows a new width |
| Position | Outside, Centred, Inside (closed paths; open curves are always centred) |
| Options | Behind fill, Scale with object, Overprint outline |

With several objects selected and an outline before, OK changes on each
object only the settings changed in the window.

## Next step

Nothing is left: every setting the earlier windows had now works (the
outline ones are described in `outline-pen.md`).
