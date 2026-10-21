# Importing files

[[menu.file > menu.file.import]] (<kbd>Ctrl+I</kbd>) adds the content of a
file to the drawing that is open, on the active layer, as one undo step.
[[menu.file.open]] instead opens a file as a drawing of its own. Both read
the formats listed in [Supported file formats](supported-formats.md#open).

## Where imported objects go {#placement}

At first, imported objects go where the file has them. To center them in the
view instead, change the setting on the [[options.file_formats]] page of the
global options; see [Global options](../customize/global-options.md#file-formats).

## Notes on some formats {#formats}

- **PDF and AI**: a protected file asks for its password in the
  [[pdf_password.title]] dialog box. Text keeps its fonts when they are
  installed.
- **EPS and PostScript**: each printed page becomes a page.
- **PSD**: each layer becomes a bitmap with its opacity, visibility and mask.
- **DXF**: drawing units are taken from the file.
- **Text files** (TXT, RTF, DOCX): become paragraph text, with bold, italic,
  sizes and fonts where the file has them.
- **Bitmaps**: placed at their resolution; see [Bitmaps](../bitmaps/index.md).

When some objects of a file cannot be read, or fonts are missing, a message
says so; see [Notes after opening](../get-started/open-and-save.md#open-notes).
You can also paste from other programs; see
[Paste Special](../objects/paste-special.md).

## Dropping files on the window {#drop}

Drag one or more files from the file manager onto the window. While they
are over it, the drawing shows a dashed blue frame and "Drop to import".
Each file is imported and embedded in the drawing, the first one centred
where you drop it and the next ones a little further each, and everything
imported is selected. A file GraphDraw cannot read opens the message
[[alert.unsupported_file]] with its name.
