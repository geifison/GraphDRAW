# Create a New Document

File > New (Ctrl+N), the New button of the standard toolbar, the New
button after the last document tab and the Welcome Screen's New Document
open the Create a New Document dialog. With "Do not show this dialog
again" ticked (or Options > General > Show New Document dialog box
cleared) these commands create the drawing at once with the settings last
used in the dialog. Every new drawing opens in its own tab.

## New Document window

These commands now open the New Document window
(`new_document_window.rs`) while Options > General > Show New Document
dialog box is on; otherwise the drawing is created at once with the
settings last used. The Create a New Document dialog described further
down stays in the code, and its settings are what both build drawings
from.

Left: a search field ("/" focuses it; words match the format names, in
the interface language or in English, and sizes such as `1080` or
`210x297`), the filters All, Print, Screens/UI, Social and Custom, and a
card per format with its sheet to scale, name, size, dpi and colour mode.
The formats are `graphdraw_core::page_presets::PRESETS` (A0 to A6, B4,
B5, Letter, Legal, Tabloid, cards, DL envelope, a 1 x 2 m banner, screens
and social media formats at 96 dpi RGB). Picking one sets size,
resolution, colour mode and unit (px for screen formats, mm for print).
A double-click on a card picks it and creates the document at once, as
Create does (`new_document_window::tests::double_clicking_a_format_creates_the_document`).

Right: the page to scale with its bleed (dashed), a badge with how many
times it fits on A4 and A3, then Name (empty: the next `Untitled-N`),
Unit (mm, cm, in, px, pt), Width and Height, Orientation, Bleed (off; on
starts at 3 mm), Resolution (72 to 2400 dpi) and CMYK or RGB. Length
fields show the unit inside (`210 mm`) and take sums, other units,
fractions and changes of the value (`8.5 in`, `100+10 mm`, `21 cm`,
`3 1/8 in`, `+15`, `50%`); Enter or leaving the field applies, Esc
restores, the arrows add 1 (Shift: 10). Create document (Ctrl+Enter)
opens one page with those settings, sets the bleed and the drawing unit;
Cancel or Esc closes the window.

## Layout

| Section | Control | Default |
|---|---|---|
| General | Name | the next `Untitled-N` of the session |
| | Preset (destination) and a menu button with Save Preset / Delete Preset | GraphDraw default |
| | Number of pages | 1 (1..999) |
| | Primary color mode: CMYK or RGB | CMYK |
| Dimensions | Page size (paper list, or Custom) | A4 |
| | Width, with the drawing units list | 210 mm, millimeters |
| | Height, with the portrait and landscape buttons | 297 mm, portrait |
| | Resolution (list and field), dpi | 300 |
| Color settings (collapsed) | RGB, CMYK and grayscale profiles, rendering intent | sRGB IEC61966-2.1, the built-in CMYK, Dot Gain 20%, Relative colorimetric |

Under the sections: a help button, "Do not show this dialog again", OK
and Cancel. Enter presses OK.

The dialog is 480 x 507 px with Color
settings closed; each open section row adds 33 px): the common dialog
frame (`options.md`), bold section headings, labels right-aligned to
190 px, 28 px controls from 195 px in rows 33 px apart: white text
fields with a grey border (blue while typing), grey lists, number fields
with up and down arrows inside their border (typed values apply on Enter
or when the field loses focus), the orientation buttons framed when
chosen, the Resolution field with its list of usual resolutions behind
an arrow, a triangle before Color settings, and 100 x 28 px OK and
Cancel buttons.

## Presets

| Preset | Colour mode | Size | Units | Resolution |
|---|---|---|---|---|
| GraphDraw default | CMYK | A4 | mm | 300 dpi |
| Default CMYK | CMYK | A4 | mm | 300 dpi |
| Default RGB | RGB | A4 | mm | 300 dpi |
| Web | RGB | 1920 x 1080 px | px | 72 dpi |

Choosing a preset sets those five values. Changing any of them afterwards
shows Custom unless the values match the preset again. Save Preset asks
for a name and adds the current values to the list; Delete Preset removes
the selected saved preset (built-in ones cannot be deleted).

Choosing a page size keeps the current orientation. A typed width and
height that match a paper size in either orientation select its name;
anything else shows Custom.

## Pixels

A pixel is one dot at the drawing's resolution: 1920 px is 162.56 mm at
300 dpi and 677.33 mm at 72 dpi. The dialog measures pixels at its own
Resolution; when the units are pixels, changing the resolution keeps the
page's pixel size. Everywhere else pixels follow the active drawing's
resolution.

## Result

OK opens a drawing with the given name, number of pages (each named
"Page N" with one layer), page size, resolution (document metadata, used
for rasterised effects), primary colour mode and profiles, and switches
the property bar's units to the chosen drawing units. The dialog's values
are kept in the settings file (`new_document`) for the next time.

## Checks

- `new_document::tests::build_makes_the_pages_size_resolution_and_colour_mode`
- `new_document::tests::editing_a_value_turns_the_preset_into_custom`
- `new_document::tests::page_sizes_keep_the_orientation_and_typed_sizes_find_their_name`
- `new_document::tests::ok_opens_a_new_tab_and_remembers_the_settings`
