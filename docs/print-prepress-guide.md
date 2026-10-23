# Print and prepress guide

Reference for improving Print, Print Preview and the preflight. It
collects the prepress knowledge the print path has to respect and maps it
to what GraphDraw does today (`docs/behavior/printing.md`,
`docs/behavior/preflight.md`). It is guidance, not a spec: a feature taken
from here gets its rules in `docs/behavior/` and its row in
`docs/features.md` before it counts as done.

## 1. What the preview is for

The preview is the last check between the drawing and the press or the
RIP. A mistake found after the plates are made costs paper, ink, plates
and press time, so the preview must show the sheets exactly as they will
print, not an approximation:

- the page placed on the press sheet, with bleed, marks and imposition;
- each separation plate, so stray inks show up before the plates are made;
- overprint and knockout as the press will produce them;
- marks and file information where the finishing department expects them.

Rule for GraphDraw: the preview and Print build the sheets with the same
function (`export::print_sheets`). Any new option goes into that function,
never into the preview alone.

## 2. Navigation

Inspection works at every scale, from the whole sheet down to a single
node or halftone dot.

| Need | Expected behaviour | GraphDraw today |
| --- | --- | --- |
| Numeric zoom | Exact percent, plus Fit | 10 to 6400 % of the fitted size, Fit |
| Zoom to area | Drag a rectangle to fill the view | works |
| Wheel and keys | Wheel and Ctrl+plus / Ctrl+minus zoom around the pointer | works (keys zoom about the centre) |
| Pan | Middle button drag pans at any zoom without changing it | works |
| Real size | 100 % equals physical size on a calibrated screen (uses the ruler calibration) | works (Real size button) |
| Sheet navigation | Previous, next, Page Up, Page Down | works |

Real-size zoom matters most: it is how small type and bleed margins are
judged before anything is printed.

## 3. Colour separations

Print colour is subtractive: cyan absorbs red, magenta absorbs green,
yellow absorbs blue. A real C+M+Y mix gives a muddy brown, not black, so a
black ink (K) adds contrast, neutral shadows and sharp small type. Spot
inks are premixed and each one gets its own plate; they hold colours that
CMYK cannot reach and stay exact for brand colours and small coloured
type.

The preview shows one plate per ink (C, M, Y, K, then each spot ink), as
grey coverage where black is full ink. GraphDraw does this today.

What the plate view must make easy to catch:

- **Rich black by accident.** Text or a logo meant to be 100 K that came in
  as RGB black converts to something like 75 C 68 M 67 Y 90 K and lands on
  all four plates. Any press movement then shows coloured fringes. Small
  text on more than one plate is a preflight warning (already checked as
  "rich black text").
- **Underbase.** On dark substrates (textiles, screen printing) a solid
  white base plate goes under bright inks. Worth supporting as a spot ink
  named for that purpose, shown as its own plate.
- **Trapping.** A slight spread or choke of edges where two inks meet, so a
  small misregistration does not show the substrate. See section 9.

### Overprint and knockout

Overprint prints an ink on top of the inks below; knockout removes them
first. Cyan overprinting yellow gives green. The preview must simulate
overprint so the designer sees both accidental darkening and intended
two-ink mixes (a cheap way to get extra colours from two spot inks).
GraphDraw already has Simulate Overprints and Always overprint black; the
print preview should show overprint in the composite view too, not only on
the plates.

## 4. Halftones and blend modes

Presses are binary: ink or no ink. Tones come from halftone dots of
varying size, which the eye merges at viewing distance.

- **Screen frequency (LPI)** and **screen angles** per ink. Classic angles
  sit 30 degrees apart for the strong inks (for example C 15, M 75, K 45)
  with yellow, the least visible, 15 degrees from its neighbour (0 or 90).
  Wrong angles produce moiré.
- **Dot gain (tone value increase, TVI).** A 50 % dot grows on the press as
  ink spreads into the paper; on uncoated stock it can print well above
  60 %. Separations and proofs should allow a dot gain curve per paper.
- **Blend modes before flattening.** Multiply is `base * blend / 255` per
  channel, which darkens like overlapping transparent markers. Logical
  modes (AND, OR, XOR) work on the bits of the channel values and give hard
  inversions. When the output cannot keep live transparency (PostScript
  level 2), the preflight should warn that it will be flattened.

GraphDraw today: separations print as continuous grey coverage; there is
no screen frequency, angle or dot shape, and no dot gain compensation.

## 5. Printers' marks

Marks live outside the trimmed product and tell the press and finishing
machines what to do.

| Mark | Purpose | Placement |
| --- | --- | --- |
| Crop and fold marks | Hairlines that guide the guillotine (trim box) and the folder | Outside the job, at every cut and fold |
| Bleed limit | Artwork extends past the trim (usually 3 to 5 mm) so a small cutting error leaves no white edge | Band right around the trim |
| Registration marks | Targets in registration colour (100 % of every ink) to align the plates | Centre of each side, sometimes corners |
| Colour bars | Solid primaries and two-ink overprints, measured on press with a densitometer or spectrophotometer | Along the sheet edge, usually the tail |
| Densitometer scale | Grey steps from paper white to full ink, to check grey balance and dot gain | Next to the colour bars |
| File information | File name, plate name, date and time, sheet number, so no plate from an old version gets mounted | Margin, head or foot |

GraphDraw today: all six exist (`printing.md` "Marks"). Gaps:

- the plate name in the file information on each separation;
- a way to move marks (for one-off die-cut packaging the default margin
  does not fit): marks relative to the page or to the bounding box of
  selected objects, with adjustable distances;
- a marks offset value instead of the fixed 12 mm margin.

## 6. Imposition

Imposition places many pages on one press sheet so that, after folding and
trimming, they come out in reading order, front and back aligned.

- **Saddle stitch.** Sheets are folded, nested and stapled. Paper thickness
  pushes the inner pages outward (creep). Compensation shifts each inner
  page towards the spine by about paper thickness times its nesting depth;
  without it the trim cuts into the outer margins of the centre pages.
- **Perfect bound.** Folded signatures (16 or 32 pages) are stacked, milled
  and glued. Creep exists only inside each signature and restarts with the
  next one.
- **Work and turn / work and tumble.** One plate prints both sides: the
  sheet turns side to side (turn) or head to tail (tumble). The preview
  must show the page rotations that match.

GraphDraw today: as in document, n-up, booklet (saddle stitch pairing) and
step and repeat. Gaps: creep compensation with paper thickness, perfect
bound signatures, work and turn / tumble layouts, fold marks between
signatures.

## 7. Job tickets (JDF and XJDF)

In automated plants a job ticket (CIP4 JDF, or the flatter XJDF) carries
the job from estimate through prepress, press (ink key presets computed
from the plates' coverage) and finishing. GraphDraw's role there is to
deliver a clean PDF/X; the RIP and the plant software do the rest. Low
priority. A possible small step: write ink coverage per plate zone, which
is what ink key presets need.

## 8. Tiled printing

Large work (posters, banners, murals) printed on small sheets is split
into tiles.

- **Tile overlap.** Printers cannot print to the paper edge. Each tile
  repeats a strip of its neighbour's content so the joins can overlap and
  hide the unprinted edge.
- **Tiling marks.** Marks at the tile corners show where to trim each tile
  before mounting, plus a row and column label on each tile.
- **Reprint one tile.** If one tile is damaged, print only that tile
  (pick it in the preview).

GraphDraw today: missing. A good candidate: it is self-contained, belongs
in `imposition.rs` as another layout, and is easy to test (tile count,
overlap, every point of the page covered by at least one tile).

## 9. Preflight

The preflight compares each object with what the chosen output can do.
GraphDraw already has thirteen checks (`docs/behavior/preflight.md`). Items
from this guide to compare against them:

| Check | Rule | Status |
| --- | --- | --- |
| RGB in a CMYK job | Every RGB or unknown-model colour; converting a wide RGB gamut to a press profile clips saturated colours | exists |
| Fonts | Missing fonts, and fonts not embedded in the output; the fix is embedding or converting text to curves | exists (missing fonts) |
| Image resolution | Effective resolution at placed size. Rule of thumb: 1.5 to 2 times the screen frequency, 300 ppi for 150 to 175 lpi | exists (low and very high) |
| Rich black small text | Small text on more than one plate | exists |
| Transparency to PostScript | Blend modes or transparency sent to an output that will flatten them | to add |
| Spot ink count | More spot inks than the job allows, or two spot inks with the same name and different values | to add |
| Total ink | Sum of C+M+Y+K above the paper's limit (often 300 to 340 %) | to add |
| Bleed | Objects touching the trim edge that do not extend into the bleed | to add |

## 10. Colour management and proofing

- **ICC profiles.** The document has a source RGB profile and an output
  profile for the press (for example a FOGRA39 or GRACoL / SWOP based CMYK
  profile). Conversion goes through a device-independent space (CIELAB).
- **Rendering intents.** Perceptual, saturation, relative and absolute
  colorimetric; the intent decides how out-of-gamut colours are mapped.
- **Soft proofing.** The preview should be able to show the sheet through
  the output profile, so the screen shows what the press can reach.

ISO 12647 sets the process control targets:

| Part | Covers | Consequence for output |
| --- | --- | --- |
| 12647-1 | Measurement conditions and terms | How colour bars and scales are measured |
| 12647-2 | Offset litho | Solid densities, TVI curves, grey balance, paper types |
| 12647-3 and -4 | Coldset newspaper, gravure | Lower ink limits, coarser screens |
| 12647-6 | Flexography | Tighter traps, careful spot ink handling |
| 12647-7 | Contract proofs from digital data | Proof printed under D50 with a control wedge, measured against the reference data |

Proof levels: a concept proof checks layout only, a comprehensive proof
checks trim and finishing, a contract proof (ISO 12647-7, with a measured
control strip) is the colour the press must match. Tolerances are given as
colour differences (Delta E) against the reference.

GraphDraw today: see `docs/behavior/colour-management.md`. Gaps for print:
soft proof in the print preview, rendering intent per output, a control
wedge mark for proofs.

## 11. Output: PDF/X

Print shops want PDF/X:

- **PDF/X-1a:** CMYK and spot only, no transparency (flattened).
- **PDF/X-3:** allows colour-managed RGB and Lab with an output intent.
- **PDF/X-4:** keeps live transparency and layers; the RIP flattens.

Overprint flags on spot and process inks must reach the PDF so every RIP
and viewer renders them the same way. Whatever the preview set up (marks,
separations, imposition, tiling) must end up in the PDF the same way.

GraphDraw today: Publish to PDF has the PDF/X levels and output condition
(`docs/behavior/publish-to-pdf.md`); Print writes a print-ready PDF.

## 12. Priorities

Suggested order, smallest and most visible first:

1. Real-size zoom, zoom to area, wheel zoom and middle-button pan in the
   preview.
2. Composite overprint simulation in the preview; plate name in the file
   information.
3. Preflight: total ink, bleed, transparency to PostScript, spot inks.
4. Tiled printing with overlap, tiling marks and single-tile reprint.
5. Movable marks and marks offset.
6. Creep compensation, perfect bound signatures, work and turn / tumble.
7. Soft proof through the output profile and rendering intent.
8. Halftone screens (frequency, angle, dot shape) and dot gain curves.

Each step: a command for every new setting (golden rule 1), tests on
`export::print_sheets` or `imposition.rs`, a section in
`docs/behavior/printing.md`, and the rows in `docs/features.md`.
