# Publish to PDF

File > Publish to PDF... (also on the standard toolbar) opens the PDF
Settings dialog for the active drawing, with the tabs General, Color,
Document, Objects, Prepress, Security and the issues tab, then "?", OK
and Cancel. OK writes the file named on General (in the browser the file
is downloaded). The dialog is `ui/publish_pdf.rs`; the file comes from
`graphdraw_io::pdf` and its protection from `graphdraw_io::pdf_crypt`.

## General

- File name: the drawing's folder and name with `.pdf` (else the last
  folder used), and Browse... to pick another.
- Export range: Current document (every page, the default), Documents
  (every page of every open drawing, one drawing after the other; only
  with more than one open; symbols are renumbered so each drawing's
  instances keep theirs), Current Page, Pages with a list such as
  `1-3, 5` (pages that do not exist are left out; a list that names no
  page is refused), and Selection (only with a selection).
- Page size, for Selection: Defined by document (the page with the
  selected objects only) or Defined by selected objects (a page the size
  of the selection's bounds, the default).
- PDF preset: Document Distribution, Web, Prepress (PDF/X-4), PDF/X-1a,
  PDF/X-3, then the presets saved with "+" (it asks a name; a built-in
  name is not taken; the same name replaces the preset). "-" deletes the
  chosen saved preset. A preset sets the compatibility, output
  condition, flattening resolution, bookmarks and bleed.
- Compatibility: PDF 1.4, PDF/X-1a:2003, PDF/X-3:2003 or PDF/X-4.

## Color

For PDF/X: Output condition (FOGRA39 by default) and the note about the
output intent's profile (the CMYK profile of Colour Management). A plain
PDF keeps the drawing's colours as they are.

## Objects

Flatten transparency at (300 dpi), for PDF/X-1a and X-3 only; greyed
for the others, which keep transparency live.

## Prepress

Bleed: the drawing's bleed (Document Properties) to start with, 0 to
50 mm, around every page (trim box the page, media box the page and the
bleed).

## Issues

What the preflight finds in the drawing (`preflight.md`), each with its
warning or note sign and its page. The tab is titled "No Issues" when
there are none, "Issues (n)" otherwise.

## Document

- Title (the drawing's name), Author, Subject and Keywords, filled in
  from the drawing's properties. An empty field leaves the entry out of
  the file. Text that is not plain ASCII is written as UTF-16 with a
  byte order mark. PDF/X files repeat the four in their XMP metadata
  (`dc:title`, `dc:creator`, `dc:description`, `pdf:Keywords`).
- A bookmark for each page (on by default): one outline item per page in
  page order, named after the page, that goes to the top left of the
  page.
- Show the bookmarks when the file opens (off by default): the catalog's
  `/PageMode /UseOutlines`.

## Security

PDF/X files cannot be protected (the standards forbid encryption): with
a PDF/X level picked the page says so and its fields are disabled.

- Require a password to open the file, and its password.
- Restrict printing, copying and changes with a password, and its
  password. Without it, whoever opens the file may do everything.
- Show passwords (they are masked otherwise).
- Permissions, enabled with the second password: Printing (Not allowed,
  Low resolution only, Allowed), Copy text and graphics, Change the
  content, Add comments, Fill in forms, Insert, delete and rotate pages,
  Text access for screen readers. All allowed by default.

OK refuses, naming the problem in red and showing the page to fix: the
Security page when a password that is asked for is empty or the two
passwords are the same, General when the page list names no page or
there is no file name. The passwords are never kept; the other choices are kept in
the preferences (`publish_pdf`) for next time.

### How a file is protected

The standard security handler of ISO 32000-2, revision 6:

- A file key of 32 random bytes encrypts every string and stream of the
  file (the encryption dictionary aside) with AES-256 in CBC mode, a
  random 16-byte IV in front of the data and PKCS #7 padding.
- Passwords are UTF-8, cut to 127 bytes. Each password's check value
  (`/U`, `/O`) is the algorithm 2.B hash of the password and an 8-byte
  random validation salt (the owner's also of `/U`), followed by that
  salt and an 8-byte key salt; the hash of the password and the key
  salt wraps the file key (`/UE`, `/OE`, AES-256 without padding and a
  zero IV). Without an open password the user password is empty.
- `/P` holds the permissions: bits 3 (print), 4 (modify), 5 (copy),
  6 (annotate), 9 (fill forms), 10 (accessibility), 11 (assemble) and
  12 (full-quality print) on when allowed, bits 7, 8 and 13 to 32 set,
  1 and 2 clear. `/Perms` repeats them with `T` (metadata encrypted) and
  `adb`, encrypted with the file key (AES-256, one block).
- The file is written as PDF 1.7 with extension level 8. There is no
  top-level `/Length` (the standard allows it only with V 2 or 3); the
  crypt filter `/StdCF` gives `/AESV3` and `/Length 32`.

## Command line

`graphdraw-cli convert in out.pdf` takes the same choices from the
environment: `GRAPHDRAW_PDF_BOOKMARKS=1`, `GRAPHDRAW_PDF_OPEN_PASSWORD`,
`GRAPHDRAW_PDF_OWNER_PASSWORD` and `GRAPHDRAW_PDF_DENY` (a comma list of
print, hq-print, copy, modify, annotate, forms, assemble, accessibility;
it needs the owner password).

## Checks

- Page lists, presets, the selection at its size or on the page, and every open drawing
  (`ui::publish_pdf::tests::page_lists_presets_and_what_goes_in`).

- `ui::publish_pdf::tests::the_dialog_starts_from_the_drawing_and_checks_the_passwords`
- `ui::publish_pdf::tests::publishing_writes_the_pages_with_bookmarks_and_protection`
- `pdf::pdfx_tests::bookmarks_name_every_page_and_can_open_the_panel`
- `pdf::pdfx_tests::protected_files_open_with_their_passwords_only`: another
  implementation accepts both passwords, refuses a wrong one, decrypts
  the information and the content, and the `/UE` and `/Perms` values
  unwrap and decrypt as the standard says.
- `pdf::pdfx_tests::document_properties_fill_the_information_and_the_xmp`
- `pdf_crypt::tests::permissions_make_the_p_value`
- `pdf_crypt::tests::objects_get_their_strings_and_streams_encrypted`
- `tests::pdf_protection_comes_from_the_environment_values` (command line)
- Checked by hand: files written by the dialog and the command line open
  in two other PDF libraries and a PDF checker with either password,
  which report the permissions as chosen and no errors.
