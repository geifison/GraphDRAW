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

## Algorithm (outline mode)

Code: `apps/graphdraw/src/trace.rs` (pipeline), `trace_color.rs` (colour),
`trace_fit.rs` (curves).

1. **Edge-preserving smoothing** when detail < 70: a bilateral filter,
   radius 1 (radius 2 below detail 35), colour sigma
   `12 + (70 - detail) * 0.6` (0..255 units). Grain and compression noise
   go; edges and corners stay (a plain blur would round them).
2. **Palette** of `n` colours in the Oklab perceptual space: median cut
   (lightness range counts double, never cutting through a run of equal
   values), then up to 8 rounds of k-means. Pixels take the nearest
   palette colour in Oklab. Black and white instead splits luma at the
   threshold with the largest between-class variance of the picture's
   tones (Otsu), not a fixed 128. Alpha below 128 is transparent.
3. **Clean-up** of the label map, up to 4 passes over its 4-connected
   regions:
   - a speck smaller than `(1 + 40 d^2) * max(1, sqrt(w h / 250000))`
     pixels, `d = 1 - detail/100`, joins the neighbour it shares the
     longest border with;
   - a fringe about one pixel thick (`2 area < 1.5 perimeter`) whose
     colour lies between two neighbours' colours in Oklab (projection
     between 5 % and 95 %, off the line by at most
     `max(0.03, sqrt(0.02) |ab|)`) joins the nearer of the two. This removes
     the rings anti-aliasing leaves round shapes. Not in black and white.
4. **Background**: the colour touching the border most, dropped when
   "remove background" is on.
5. **Stacking**: colours are layered largest area first. Each layer's mask
   also takes the pixels of layers above it within 2 pixels of it, and
   the holes it would have that lie wholly under layers above. Neighbouring
   colours therefore overlap underneath instead of meeting edge to edge,
   which would let the background show through as hairline seams.
6. **Contours** follow pixel edges (filled side on the left; outer rings
   counter-clockwise and holes clockwise in page space). Each ring becomes
   the midpoints of its unit edges, which lie on the line a staircase
   approximates.
7. **Curve fitting** (`trace_fit.rs`):
   - turn at point `i` = angle between `p[i] - p[i-3]` and `p[i+3] - p[i]`;
     a corner turns by at least `50 + corner_smoothness * 0.85` degrees,
     is a local maximum within 3 points and turns at least half that angle
     more than the points 6 away (so small circles stay smooth);
   - a corner moves to the crossing of the straight runs either side when
     that is within 1 pixel (restores square corners);
   - `round(smoothing / 25)` binomial passes `(a + 2p + b) / 4` move the
     other points;
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

Tests in `trace.rs`, `trace_color.rs`, `trace_fit.rs`, `trace_center.rs`:

- A black circle (r 30) on white, line art: one region, area within 2 %,
  at most 12 segments; the white background is not emitted.
- A 40 x 30 square, logo: four straight lines with exact corners.
- A soft-edged disc with scattered noise, logo: one region, no fringe ring
  and no specks.
- Two colours meeting on a slant, background kept: drawn back, no pixel
  inside shows the transparent background.
- A 5 px line, technical: one straight stroke 5 +- 1 px wide. A cross: two
  strokes. A ring: one closed stroke.
- Empty, 1 x 1 and fully transparent pictures trace to nothing with every
  preset.

`TRACE_IN=pic.png TRACE_OUT=dir cargo test --release -p graphdraw
trace_a_file -- --ignored --nocapture` writes each preset's result drawn
back next to the picture, with shape and node counts and timing.
