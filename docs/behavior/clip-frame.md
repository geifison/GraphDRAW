# Clip frames

Object > Clip to Frame: objects placed inside a container that clips
them.

## Model

`ShapeKind::ClipFrame { frame, contents }`: the frame is any closed
object, kept whole with its own transform, fill and outline; the
contents keep their own page-space transforms. The frame's fill and
outline are drawn, the contents are clipped to the frame's path.

## Commands

- Place Inside Frame: with contents selected, choose it (Object > Clip to
  Frame, or the object context menu), then click the frame; the pointer
  is the solid arrow while it waits. The contents keep their position
  (centred when Options > Clip Frames asks for it). Three ways in: the
  Object menu, the context menu, or dragging the contents onto the frame
  with the right button and choosing Place Inside.
- Extract Contents: the contents of the selected clip frames become
  ordinary objects again, selected; the frames stay. One undo step. An
  empty frame is left alone and a notice says there is nothing to
  extract.
- Edit Contents (Ctrl+click a clip frame with the Pick tool): the
  contents come out to be moved and edited, the frame is drawn with a
  dashed blue line, and the clip frame bar shows the level being edited.
  Finish Editing Contents (Esc, Ctrl+click an empty spot, the bar's
  button or the context menu) puts the contents back. A frame left with
  no contents stays an empty clip frame.
- Nested clip frames: Ctrl+click a clip frame among the contents being
  edited goes one level deeper; finishing goes back one level. The bar
  shows "level n of m": n counts from the outermost frame, m is n plus
  how many clip frames deep the contents go.
- Center Contents, Fill Frame Proportionally, Fit Contents
  Proportionally: the contents of each selected clip frame, as one block,
  are centred on the frame (size kept), scaled in proportion until they
  cover the frame (the parts outside are clipped), or scaled until they
  just fit inside it; one undo step each. Measured in the frame's own
  space (its bounds and the contents' bounds), so a turned clip frame
  works the same.
- Lock Contents to Frame (default on, stored as `clip_frame.unlocked` in
  the object data when off): with the lock on, moving, scaling or
  rotating the frame transforms the contents too. With it off, the frame
  moves alone and the contents stay where they are
  (`compensate_unlocked_clip_frames` undoes the transform on them).

## Clip frame bar

With clip frames selected and the Pick tool idle, a bar shows under them
(above them when there is no room below, kept inside the drawing
window): Edit Contents, Center Contents, Fill Frame Proportionally, Fit
Contents Proportionally, Extract Contents and a padlock for Lock Contents
to Frame (closed: the contents move with the frame). While contents are
edited it shows Finish, "Editing contents: level n of m" and a Ctrl key
cap with "+ click outside to finish". Every button has a hover tip with
its shortcut, which is also its accessible name, and takes keyboard
focus. The same commands are in Object > Clip to Frame and in the object
context menu, with the same icons.

## Notices

Shown on the status bar and for 2.5 s next to the pointer:

- Place Inside Frame clicked on a locked object: "The object is locked.
  Unlock it to continue."
- Place Inside Frame clicked on the only selected object: "Choose a
  different object as the frame."
- Extract Contents on empty frames: "There is no content to extract."
- Ctrl+click on an object that is not a clip frame: the ordinary
  Ctrl+click (a member of a group), nothing about clip frames.

## Opacity mask

Object > Clip to Frame > Opacity Mask (a check item) turns the selected clip
frames into opacity masks and back: the frame is no longer drawn and no
longer clips hard; its brightness (its fill, gradients and patterns
included, through the luminance of the colour) sets how much of the
contents shows, white fully, black not at all, nothing outside the
frame. Invert Mask swaps that (black shows). Place inside, extract, edit
in place and lock work as for any clip frame. One undo step each.

- Screen and bitmaps: the frame is drawn alone and its luminance masks
  the contents' layer (`Shape::opacity_mask`, `crates/render`).
- SVG: a `<mask>` holding the frame's fill (inverted through a
  `feColorMatrix` filter), on a group of the contents.
- PDF: the masked object goes in as a picture with its transparency
  (300 dpi, at most 40 million pixels); PDF/X-1a flattens it with what
  lies under it.

## Frame Type (object context menu)

- Create Empty Clip Frame: each selected object that is not a clip
  frame becomes one holding nothing, in place (turned or not), as Place
  Inside Frame with no contents does; one undo step. Pale when nothing
  selected can become one.
- No Frame: each selected empty clip frame becomes the object it was
  made from, with its id; one undo step. Pale without one.
- Create Empty Text Frame (one object selected): the object gives way to
  an empty paragraph text frame of its size (its bounds) in its place,
  in the font, size, style, alignment and hyphenation new text takes;
  one undo step. The frame stays selected (and stays when left empty)
  for the Text tool.

## Checks

- Given a locked clip frame, when the frame is moved by (10, 0), then the
  contents move by (10, 0).
- Given an unlocked clip frame, when the frame is moved by (10, 0), then
  the contents' page bounds are unchanged.
- Given a `.cdr` bitmap with a crop outline smaller than the image, then
  the reader produces a clip frame of the bitmap inside the crop path
  (confirmed with the 2019 banner file).
- `render::tests::opacity_masks_show_the_contents_by_the_frame_brightness`,
  `svg::tests::opacity_masks_are_svg_masks`,
  `pdf::tests::opacity_masks_go_in_as_transparent_pictures`,
  `command::tests::only_clip_frames_take_an_opacity_mask`,
  `app::tests::clip_frames_turn_into_opacity_masks_and_back`.
- `frame_type::tests::empty_clip_frames_come_and_go_in_place`,
  `frame_type::tests::an_object_gives_way_to_an_empty_text_frame_of_its_size`.
- `clip_frames::tests::fit_transforms_centre_fill_and_fit`,
  `clip_frames::tests::the_bar_centres_fills_and_fits_in_one_undo_step_each`,
  `clip_frames::tests::ctrl_click_edits_and_ctrl_click_outside_finishes`,
  `clip_frames::tests::nested_clip_frames_are_edited_level_by_level`,
  `clip_frames::tests::an_empty_frame_stays_a_frame_after_editing`,
  `clip_frames::tests::extracting_an_empty_frame_says_so_and_changes_nothing`,
  `clip_frames::tests::a_frame_cannot_take_itself_and_locked_targets_are_named`.
