# Shape brushes: Smooth, Smear, Twirl, Attract and Repel, Smudge, Roughen

Drag over a selected curve (objects that are not curves are converted
first; with nothing selected, the object under the pointer is taken) to
reshape the outline within the nib. The whole drag is one undo step; the
next drag is another.

## Common behaviour

- The nib is a circle of the bar's Nib size (radius, 10 mm at first)
  around the pointer in page space, taken into each object's own space.
- The effect falls off from full at the centre to nothing at the edge of
  the nib (`w = 1 - d / radius`).
- Each subpath the nib reaches is measured into points (a third of the
  nib apart for Smear and Twirl, a quarter for Smooth, the spike spacing
  for Roughen), moved and fitted back into curves (Roughen: straight
  lines, so the spikes stay sharp). Subpaths out of reach stay exactly as
  they were; open curves stay open, closed ones closed.

## Tools and their bars

Every bar starts with Nib size and ends with "Drag over a selected
object". The settings are kept in the preferences (`tools.brush_radius`,
`tools.roughen_amount`, `tools.brush`).

| Tool | Bar | What a step does |
|---|---|---|
| Smooth | Rate 1 to 100 % (20) | the run of points within the nib simplified within `0.05 + 1.45 (rate - 1) / 99` mm and re-fitted |
| Smear | Pressure 1 to 100 % (100) | points move by the pointer's move times pressure times `w^2` |
| Twirl | Speed 1 to 100 % (50); Counterclockwise or Clockwise (Alt or Shift while dragging turns the other way) | points turn about the pointer by `speed / 1000` rad per mm the pointer moved, times `w`; held still, it goes on as if the pointer moved 10 mm a second |
| Attract and Repel | Speed 1 to 100 % (50); Attract or Repel (Alt or Shift while dragging swaps them) | nodes and control points move towards (away from) the pointer by `speed x 0.006` of the pointer's move times `w`, never past the middle; held still, it goes on as if the pointer moved 10 mm a second |
| Smudge | Pressure 1 to 100 % (100) | a smooth (0.2 mm), then a smear at the pressure, so the outline is pulled without kinks |
| Roughen | Frequency 1 to 10 spikes per 10 mm (5); Spike size (2 mm) | points `10 / frequency` mm apart within the nib pushed off the curve by up to the spike size times `w`, the push from the point's place |

## Checks

- `graphdraw_core::effects::tests::brushes_touch_only_the_subpaths_they_reach`
- `brushes::tests::a_stroke_is_one_step_and_follows_the_settings`
- `brushes::tests::attract_repel_and_twirl_follow_their_modes`
- `ui::brush_bars::tests::brush_settings_scale_and_stay_in_range`
- `ui::brush_bars::tests::every_brush_bar_draws`
