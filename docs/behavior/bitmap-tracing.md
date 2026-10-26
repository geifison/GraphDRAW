# Bitmap tracing (bitmap to vector)

| Mode | Use | Defaults |
|---|---|---|
| Quick trace | one click | the preset of Options > Global > Bitmap Tracing (Clipart by default) |
| Centreline: technical illustration, line drawing | strokes | |
| Outline: line art, logo, detailed logo, clipart, low/high quality image | filled regions | smoothing 25, detail per preset, colours per preset |

| Control | Default | Range |
|---|---|---|
| Detail | per preset | 0..100 |
| Smoothing | 25 | 0..100 |
| Corner smoothness | 0 | 0..100 |
| Colour mode | RGB | RGB, CMYK, Grayscale, Black and white |
| Number of colours | per preset | 1..256 |
| Delete original | off (Options > Global > Bitmap Tracing) | |
| Remove background | on (Options > Global > Bitmap Tracing) | auto or colour |

## Dialog preview

The Trace Bitmap dialog opens with the bitmap before and its trace after,
side by side (200 points each, a checkerboard behind transparency). The
trace is worked out on a copy of the bitmap at most 240 pixels on its
longer side, again whenever a setting changes: closed regions filled with
their colours, centrelines stroked. Preview still counts the curves of the
full trace.

## Blocky pictures

Code: `apps/graphdraw/src/trace_grid.rs`. A small picture enlarged
without smoothing is made of square blocks of one colour. Tracing the
blocks would draw their steps, so the picture is traced from one pixel
per block and the shapes are put back over the blocks.

- Colour changes (a channel differing by more than 24) are counted on
  every line between columns and between rows. Blocks at least 3 px wide
  leave most lines without a change: with changes on more than 70 % of
  the lines the picture is not blocky.
- The block size is the largest from 64 px down (steps of 0.05) whose
  grid (phase in steps of 0.25) carries 90 % of the changes within
  `clamp(0.15 b, 0.6, 1.25)` px of its lines (an enlarged picture that
  was resampled smears each line over a pixel or two); then, within 15 %
  below it, the size whose lines the changes follow most closely, with
  the phase centred on them. Across and down must agree within 10 %.
- 85 % of the pixels inside the blocks (away from the smeared lines)
  must match their block's centre.
- Each whole block gives one pixel, its centre's colour. The trace then
  works as below with these changes: no edge-preserving smoothing;
  speck sizes are measured in the picture's own pixels; every mixed
  region is a fringe, however wide, the removed ground counts as a
  partner for it, and a region within 0.12 of the ground's colour is
  ground; no gradients (mixed edge blocks would read as shading). Stroke
  widths of centrelines scale by the block size.

## Two-tone pictures and black and white

Code: `apps/graphdraw/src/trace_ink.rs`. Black and white mode, and any
outline trace of a picture whose pixels are all mixes of two colours
(most logos, icons and lettering), follow the edge where the picture's
anti-aliasing puts it instead of tracing colour regions.

- **Two tones**: two means in Oklab, started from the darkest and the
  lightest pixel (12 rounds). The picture is two-tone when they are at
  least 0.1 apart, 98.5 % of the opaque pixels lie within 0.06 of the
  mix between them, and no more than 12 pixels off the mix have 7 or 8
  of their 8 neighbours off it too (compression leaves stray colours
  along edges; a third colour is a patch of them). A tone within 24 of
  pure black or white in every channel is drawn pure.
- **Amount of ink**: the tone of a pixel is its luma (black and white)
  or how far it lies from the darker colour to the lighter (two tones).
  Otsu splits the tones; the means of the two classes are paper and
  ink, and a pixel's ink is its tone between them, 0 to 1. The ink is
  the tone the border is not (light lines on a dark ground work).
  Black and white mode is first smoothed as in step 1 below.
- Ink specks (8-connected) and paper pinholes (4-connected, not on the
  border) smaller than the speck size are cleared or filled.
- A picture drawn without anti-aliasing (under 25 % of its edge pixels
  between 0.1 and 0.9) is softened with weights `0.2 0.6 0.2` each way,
  so its edges run through the middle of the steps.
- **Outline**: the ink is interpolated (Catmull-Rom, the picture's edge
  held beyond it) at `k x k` cells per pixel,
  `k = clamp(floor(sqrt(40 000 000 / pixels of the ink's box)), 1, 8)`,
  and cut at one half. Interpolating the share of a pixel an edge covers
  puts the edge up to an eighth of a pixel off.
- Curves are fitted as in step 9 with
  `tolerance = 0.2 + (100 - detail) / 100 * 0.6` pixels.
- **Pieces**: outer rings turn like the largest ring; each hole goes to
  the smallest outer ring round its first point. Each outer ring with
  its holes is one shape (an island in a hole is a shape of its own),
  largest first. With remove background off, a rectangle of the paper
  colour goes under them.

## Algorithm (outline mode)

Code: `apps/graphdraw/src/trace.rs` (pipeline), `trace_color.rs` (colour),
`trace_region.rs` (shaded regions), `trace_fit.rs` (curves).

1. **Edge-preserving smoothing** when detail < 70: a bilateral filter,
   radius 1 (radius 2 below detail 35), colour sigma
   `12 + (70 - detail) * 0.6` (0..255 units). Grain and compression noise
   go; edges and corners stay (a plain blur would round them).
2. **Background** (remove background on): flooded from the border through
   4-neighbour steps of at most 0.035 in Oklab, measured both against
   the neighbour and over three pixels on the same line (a blurred edge
   climbs in small steps, a shaded ground barely moves), while the 5 x 5
   mean round the pixel stays within 0.25 of the border's median colour;
   then enclosed pixels within 0.05 of it. Up to three 3 x 3 majority
   passes straighten the result (only pixels within 0.25 of the ground
   may turn into ground). The ground is left out before the palette.
3. **Palette** of `n` colours in the Oklab perceptual space: median cut
   (lightness range counts double, never cutting through a run of equal
   values), then up to 8 rounds of k-means. Pixels take the nearest
   palette colour in Oklab. Alpha below 128 is transparent. (Black and
   white and two-tone pictures are traced as above instead.)
4. **Clean-up** of the label map, up to 4 passes over its 4-connected
   regions:
   - a speck smaller than `(1 + 40 d^2) * max(1, sqrt(w h / 250000))`
     pixels, `d = 1 - detail/100`, joins the neighbour it shares the
     longest border with;
   - a fringe about one pixel thick (`2 area < 1.5 perimeter`) whose
     colour lies between two neighbours' colours in Oklab (projection
     between 5 % and 95 %, off the line by at most
     `max(0.03, sqrt(0.02) |ab|)`) joins the nearer of the two. This removes
     the rings anti-aliasing leaves round shapes. Not in black and white.
5. **Shaded regions** (gradients on, colour): connected pieces of one
   palette colour are merged across soft borders, where the mean Oklab
   step between the pixels either side is at most
   `0.03 + 0.03 d` (`d = 1 - detail/100`), softest first, while a
   quadratic colour surface (`1, u, v, u^2, u v, v^2` per sRGB channel,
   least squares from summed moments) explains the union nearly as well
   as the parts: the merge may add at most `(0.012 + 0.02 d)^2` to the
   mean squared error. sRGB, not Oklab, because fountains mix their
   colours straight in sRGB. Each region then gets a flat colour (its
   mean) or, when its colour changes by at least 0.025 along the main
   direction of its colour gradient, a linear fountain along that
   direction: the mean colour of bins 2 px wide (at most 64), a light
   `[1 2 1]` pass, then stops kept by largest error until every bin is
   within 0.01 of the straight mix of its neighbouring stops (at most 8
   stops). Flat regions within 0.015 in Oklab share one layer (one
   compound shape, area-weighted colour).
   Without gradients each palette colour is one layer.
6. **Background** without a flooded one: the layer touching the border
   most, dropped when "remove background" is on.
7. **Stacking**: layers go largest area first. Each layer's mask
   also takes the pixels of layers above it within 2 pixels of it, and
   the holes it would have that lie wholly under layers above. Neighbouring
   colours therefore overlap underneath instead of meeting edge to edge,
   which would let the background show through as hairline seams.
8. **Contours.** A picture smaller than 1600 px on its longer side has its
   outlines followed `k = min(4, 1600 / longer side)` times finer (at
   most 4 million cells). Each layer gets a coverage per pixel: 1 inside
   it; on its edge pixels `1 - t` and on the outside pixels next to it
   `t`, where `t` is how far the pixel's colour lies along the mix of the
   two colours either side (the anti-aliasing of the edge, as in the
   fringe rule); past the picture's edge the coverage carries on as at
   the edge. The coverage is softened (weights `1 6 1` each way), sampled
   bilinearly at the centres of `k x k` cells per pixel and cut at 0.5
   (0.35 inside a pixel covered whole, so thin lines and dots survive).
   A staircase becomes a continuous line through the middle of its
   steps, and a mixed pixel moves the line to where the true edge
   crosses it. Contours then follow cell edges (filled side on the left;
   outer rings counter-clockwise and holes clockwise in page space); each
   ring becomes the midpoints of its unit edges, which lie on the line a
   staircase approximates, kept inside the picture.
9. **Curve fitting** (`trace_fit.rs`), with `K = 3 k` points:
   - turn at point `i` = angle between `p[i] - p[i-K]` and `p[i+K] - p[i]`;
     a corner turns by at least `50 + corner_smoothness * 0.85` degrees,
     is a local maximum within `K` points and turns at least half that
     angle more than the points `2K` away (so small circles stay smooth);
   - a corner moves to the crossing of the straight runs either side when
     that is within 1 pixel (restores square corners); each run is the
     least-squares line through the points `K / 2` to `2K` from the
     corner, away from the rounding a finer outline already has;
   - `k * round(smoothing / 25)` binomial passes `(a + 2p + b) / 4` move
     the other points;
   - a run between corners within `0.6 * tolerance` of its chord is a
     line; otherwise least-squares cubics with fixed end tangents, chord
     length parameters refined by up to 6 Newton steps, split at the worst
     point (tangent through its neighbours) until every point is within
     `tolerance = 0.25 + (100 - detail) / 100 * 1.5` pixels;
   - a loop with no corner is one smooth run with the same tangent on both
     sides of its seam.
   Pieces with area under 0.5 px^2 are dropped.

## Algorithm (centreline mode)

Code: `apps/graphdraw/src/trace_center.rs`.

1. Ink: opaque pixels darker than the Otsu threshold. Ink specks
   (8-connected) and paper pinholes (4-connected, not touching the
   border) smaller than the speck size above are removed.
2. Half-width: chamfer 3-4 distance from each ink pixel to paper.
3. Zhang-Suen thinning, then pixels that only round off a staircase (their
   neighbours stay one 8-connected group and they are not line ends) are
   removed.
4. The skeleton is read as a graph: nodes are pixels with other than two
   neighbours (touching node pixels form one junction), runs go between
   nodes, loops with no node are kept whole.
5. Runs from a junction to a free end shorter than `max(3, 3 * mean
   half-width)` pixels are spurs and are dropped.
6. At a junction the run ends (direction over up to 6 pixels) are paired
   most opposite first, only when they carry on within 60 degrees of
   straight; at a node with two runs left they always join. A crossing
   gives two strokes.
7. Each stroke is fitted as in outline mode (at least one smoothing pass),
   closed when it loops. Width = mean of `2 * half-width - 1` over its
   pixels (at least 1 px), colour = mean colour of its pixels.

## Checks

Tests in `trace.rs`, `trace_color.rs`, `trace_region.rs`, `trace_grid.rs`,
`trace_ink.rs`,
`trace_fit.rs`,
`trace_center.rs`:

- A black circle (r 30) on white, line art: one region, area within 2 %,
  at most 12 segments; the white background is not emitted.
- A 40 x 30 square, logo: four straight lines with exact corners.
- A soft-edged disc with scattered noise, logo: one region, no fringe ring
  and no specks.
- Two colours meeting on a slant, background kept: drawn back, no pixel
  inside shows the transparent background.
- A disc shaded red to blue, clipart: one shape with a linear fountain
  whose axis on the page is the traced ramp; drawn back, its middle is
  the mix. A ramp cut into palette bands merges back into one region; a
  hard edge keeps two flat ones.
- A 30 px disc, line art: followed 4 times finer, area within 5 %, at
  most 8 segments.
- A quarter ring with grey anti-aliased edges, drawn at 24 px and
  enlarged 10 times without smoothing, logo: blocks of 10 found, one
  black shape, area within 5 % of the true ring, at most 12 segments.
  Pictures enlarged 10, 7 and 4 times show their blocks; a smooth
  picture shows none.
- A 5 px line, technical: one straight stroke 5 +- 1 px wide. A cross: two
  strokes. A ring: one closed stroke.
- A rectangle with exact anti-aliased edges, black and white: its edges
  within 0.15 px of the true ones. A soft black disc on white is two-tone
  and traces in colour to one pure black shape, area within 1 %; with a
  red dot of radius 4 it is not two-tone.
- Empty, 1 x 1 and fully transparent pictures trace to nothing with every
  preset.

`TRACE_IN=pic.png TRACE_OUT=dir cargo test --release -p graphdraw
trace_a_file -- --ignored --nocapture` writes each preset's result drawn
back next to the picture, with shape and node counts and timing;
`TRACE_SCALE=0.15` shrinks the picture first, `TRACE_CROP=x,y,w,h` also
writes that part of picture and trace at full size. `trace_to_svg`
(`TRACE_IN`, `TRACE_SVG`, `TRACE_PRESET`) writes one preset's trace as an
SVG in the picture's pixels; `quick_trace_to_svg` imports the picture,
Quick Traces it and exports the page as SVG framed on the picture, for
comparing with a reference trace.
