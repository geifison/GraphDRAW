# Transparency

| Control | Default | Range |
|---|---|---|
| Type | Uniform | None, Uniform, Gradient (linear, radial, conical, square, shaped), Pattern, Texture |
| Transparency | 50% | 0..100 |
| Merge mode | Normal | Normal, Add, Subtract, Difference, Multiply, Divide, If lighter, If darker, Screen, Overlay, Hard light, Hue, Saturation, Colour, Invert, And, Or, Xor, Red, Green, Blue |
| Apply to | Fill and outline | fill, outline, both |
| Freeze | button | Object > Freeze Transparency, and on the Transparency tool's bar |

Gradient transparency uses the same geometry as gradient fills with
grayscale stops where white = opaque and black = transparent. The
object is rendered to a scratch layer, multiplied by the transparency
mask, then composited with the merge mode. A stop's opacity scales its
level, so 0% hides the object there whatever its grey
(`transparency_stop_opacity_fades_the_object`). A shaped gradient follows
the object's outline (`tests::shaped_transparency_follows_the_object`).
A mesh fill under a transparency gradient keeps its own node
transparency: the two multiply.

## Apply to

The transparency goes on the fill alone, the outline alone, or both.
The part it does not apply to is drawn as it is, in its usual order (an
outline behind the fill first)
(`transparency_applies_to_the_fill_or_the_outline_alone`). A picture
is its fill: on the fill (or both) it fades with the mask, on the
outline alone only its frame does (`transparency_works_on_pictures`).
Uniform
transparency on one part, or with a merge mode other than Normal, is
kept as a plain grey mask of its level; on both parts with Normal it is
the object's opacity.

## Merge modes

Where the object and what is beneath are both opaque, the colour is
`B(cb, cs)`, channel by channel (0..1); where only one of them is, its
own colour shows (`co = cs(1 - ab) + cb(1 - as) + as ab B`):

| Mode | B(cb, cs) |
|---|---|
| Normal | cs |
| Add | min(cb + cs, 1) |
| Subtract | max(cb - cs, 0) |
| Difference | abs(cb - cs) |
| Multiply | cb cs |
| Divide | min(cb / cs, 1), 1 where cs = 0 |
| If lighter / If darker | max / min |
| Screen | 1 - (1 - cb)(1 - cs) |
| Overlay | multiply where cb <= 0.5, screen otherwise (doubled) |
| Hard light | the same, chosen by cs |
| Invert | 1 - cs |
| And, Or, Xor | bitwise on the 8-bit values |
| Red, Green, Blue | that channel from the object, the others from beneath |
| Hue, Saturation, Colour | the object's hue, saturation or both over the backdrop's |

(`merge_modes_follow_their_formulas`,
`transparency_merge_modes_mix_with_what_is_beneath`). The merge modes
are saved by name; Screen, Overlay and Hard light were added after the
others and older files never use them. PDF, SVG and CDR exports keep
the mask's average level and draw the object with Normal.

## Handles on the canvas

With the Transparency tool, an object selected alone with a gradient
transparency shows the same handles as Interactive Fill: the start and
end squares (the centre and radius for the centred kinds, plus the y
radius of elliptical and square ones), the axis, and the grey stops on
it. Dragging an end moves it (Shift turns the axis about its centre,
Ctrl in 15 degree steps); dragging a stop slides it; a double click on
the axis adds a stop with the level already there; a click on a stop
opens its balloon (level, opacity, midpoint); a palette colour clicked
goes to the chosen stop; Delete removes it. Each change is one undo
step, a drag is one step, and the fill is not touched
(`transparency_gradient_has_handles_on_the_canvas`). Dragging across
the object away from the handles starts a new transparency of the kind
chosen on the bar, keeping the grey stops it had: linear from the press
point to the release point, radial (and the other centred kinds)
centred on the press point with its radius reaching the release point
(`dragged_fountain_keeps_its_kind`). The bar shows the selected
object's mask, so changing its kind or angle keeps its stops.

## Copy and clone

Object > Copy Effect > Transparency From and Clone Effect >
Transparency From carry the transparency to the selection in
proportion: a gradient already sits relative to its object's box, and
a pattern's tiles or a texture's grain grow or shrink with the ratio of
the two boxes' diagonals (`copied_transparency_keeps_its_proportions`).

## Freeze

Freeze turns each selected object with transparency (opacity, fill
opacity or a transparency mask) into a 300 dpi bitmap of what shows
there now: the object over the visible objects beneath it, kept only
where the object covers (its outline, shadow and live effects
included). Where nothing is beneath, the picture keeps the object's
own transparency. The bitmap takes the object's place in the stacking
order and its id; moved later, it keeps the look it had. One undo step
(`frozen_transparency_keeps_the_look_where_it_was`). With nothing to
freeze, the status bar says so.
