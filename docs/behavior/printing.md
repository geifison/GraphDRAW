# Printing

File > Print (Ctrl+P) builds a print-ready PDF of the sheets and opens it
in the system viewer, where the printer is chosen (in the browser the PDF
is downloaded). `export::print` builds it; the sheets come from
`imposition.rs`.

## Print dialog

Tabs: General, Color, Composite, Layout, Prepress, PostScript and the
preflight's Issues ("No Issues" or the count; hidden when Options >
Global > Printing turns the preflight off). Under them Print Preview
(hands the settings to the preview), Print, Cancel and Apply (keeps the
settings as Options > Global > Printing's). The dialog starts with those
preferences, and with the drawing's own bleed as the bleed limit when it
has one.

- General:
  - Destination: Printer is the system printer (a PDF opened in the
    system viewer, which owns the printer and the copy count; in the
    browser the PDF is downloaded), a PDF file or a PostScript file (saved
    where chosen). Orientation: Match orientation (the Layout tab's
    Landscape box), Portrait or Landscape, for fixed sheets.
  - Print range: Current document, Current page, Pages (a list such as
    `1-3, 5, 8-`; an empty or unusable list prints them all), Selection
    (the selected objects of the current page).
  - Copies: a file gets its copies inside, as whole sets (Collate) or
    each sheet repeated.
  - Print as bitmap: each sheet becomes one picture at the resolution
    given (36 to 1200 dpi).
- Color: Composite or Separations (one grey page per ink and sheet: C, M,
  Y, K, then the spot inks; black = full ink); Convert spot colors to
  CMYK prints spot inks as their CMYK mix at their tint.
- Composite: Print text in black (fills, outlines and character colours
  of every text); Always overprint black (100 % black fills and outlines
  overprint).
- Layout: As in document, Fit to page or Scale (percent); Bleed limit
  (how far beyond the page objects print, 4 mm by default); the
  imposition, sheet, gap and the sketch (below).
- Prepress: Invert and Mirror (the whole sheet, marks included); Print
  file information (with its text, the drawing's name when left empty)
  and Print page numbers (each page's number under it); Crop/fold marks;
  Registration marks; Color bars and Densitometer scale.
- PostScript: with a PostScript file as the destination, the language
  level written in its header (2 or 3).
- Issues: what the preflight finds, and the way to its dialog.

## Print Preview

File > Print Preview, or Print Preview in the Print dialog (which hands
over its settings), fills the window with the sheets exactly as Print
sends them: the imposition, the marks, Mirror and Invert, built by the
same function (`export::print_sheets`).

- Top bar: Print (prints these settings and closes), Close (Esc), the
  sheet shown with previous and next (Page Up, Page Down), the Plate
  (Composite, then cyan, magenta, yellow, black and each spot ink) while
  Print separations is on, Zoom from 10 to 6400 % of the fitted size, Fit
  and Real size (100 % of the paper at the screen resolution of Options >
  Display > Ruler calibration).
- The sheet: the wheel (and a pinch) zooms by 15 % a notch about the
  pointer, Ctrl+plus and Ctrl+minus by 25 % about the centre; the middle
  button drags it; a left drag draws a rectangle and zooms so that it
  fills the view, centred; a double click fits. At least 48 points of
  the sheet stay on screen. Up to 4096 pixels on its longer side the
  whole sheet is rendered once; past that only the part on screen, so
  small type stays sharp at any zoom.
- Left: the Print dialog's options (all pages, fit, scale, prepress and
  layout with its sketch); every change rebuilds the sheets at once.
- A plate shows the ink's coverage in grey, black = full ink, as the
  separations print; registration colour shows on every plate.

## Imposition

- Pages keep their aspect ratio; each cell is as large as the largest
  page on its sheet and a smaller page sits in its middle.
- A booklet's page count is rounded up to a multiple of four with blank
  pages at the back. The sheet sides pair the pages as a folded booklet
  needs: for eight pages 8 and 1, 2 and 7, 6 and 3, 4 and 5 (left and
  right). Its two pages meet at the spine with no gap and no bleed between
  them.
- Every page is clipped to its trim box plus the bleed, so nothing spills
  onto its neighbours or the margin. Hidden and non-printing layers are
  left out; master layers print with every page they show on; a page
  background prints under its page.

## Marks

With any mark on, a 12 mm margin (plus the bleed) surrounds the pages.

- Crop marks: 5 mm lines in registration colour, 0.1 mm wide, starting
  1.5 mm beyond the bleed, at every cut line of the grid, outside the
  pages only. A booklet also gets a dashed fold mark at the spine.
- Registration marks: a target (circle and cross) centred on each side
  of the pages.
- Color bars: patches of 4 mm of cyan, magenta, yellow, black, red,
  green, blue (two-ink overprints) and 50 % tints of the four inks, left
  of the top target.
- Densitometer scale: black from 0 to 100 % in steps of ten, right of the
  top target.
- Page numbers: each page's number, 6 pt, under the page.
- Page information: the drawing's name (or the text given), the pages on the sheet, "sheet n
  of n" and the date and time, in 6 pt under the pages.

## Checks

- `export::tests::page_lists_and_ranges_pick_pages`
- `export::tests::print_colour_options_change_the_sheets_objects`
- `export::tests::copies_repeat_sheets_collated_or_not_and_ps_gets_its_level`

- `ui::print_preview::tests::plates_split_the_sheet_by_ink`
- `ui::print_preview::tests::the_settings_key_ignores_copies_and_preflight`
- `ui::print_preview::tests::zooming_keeps_the_point_under_the_pointer`
- `ui::print_preview::tests::a_zoom_rectangle_fills_the_view_centred`
- `ui::print_preview::tests::panning_keeps_part_of_the_sheet_on_screen`
- `ui::print_preview::tests::real_size_follows_the_ruler_calibration`
- `ui::print_preview::tests::a_part_of_the_sheet_renders_like_the_same_part_of_the_whole`

- `imposition::tests::booklets_pair_the_pages_for_folding`
- `imposition::tests::n_up_fills_sheets_in_reading_order_and_scales_to_fit`
- `imposition::tests::fitted_sheets_grow_with_marks_bleed_and_gutters`
- `imposition::tests::imposing_places_and_clips_every_page`
- `export::tests::the_print_dialog_settings_make_the_layout`
- `export::tests::colours_are_mapped_inside_groups_frames_and_text`: the
  separations and Invert reach into groups, clip frames and character
  fills.
