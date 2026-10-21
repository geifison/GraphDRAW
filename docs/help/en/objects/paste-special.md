# Paste Special

[[menu.edit > menu.edit.paste_special]] chooses how the content of the
clipboard comes into the drawing, when it could come in more than one way:
for example text copied from another program can become a text object, and
an SVG copied from a web page can become editable shapes or a picture.

![The Paste Special dialog box](../../img/objects-paste-special.png)

## To paste in another form {#paste-as}

1. Copy something, in GraphDraw or in another program.
2. Choose [[menu.edit > menu.edit.paste_special]].
3. Under [[dialog.paste_as]], choose one of the forms the clipboard offers:
   - [[dialog.paste_objects]]: objects copied in GraphDraw, in this or
     another window, as they were;
   - [[dialog.paste_svg]]: SVG, turned into shapes you can edit;
   - [[dialog.paste_bitmap]]: a picture, as a bitmap;
   - [[dialog.paste_files]]: each file imported as by File > Import;
   - [[dialog.paste_text]]: plain text, as a line of text.
4. Click [[dialog.ok]].

When the clipboard holds nothing that can be pasted, the dialog box says
[[dialog.clipboard_empty]].

The ordinary [[menu.edit.paste]] (<kbd>Ctrl+V</kbd>) pastes in the best form
available: GraphDraw objects, then SVG, files, pictures and text. Options >
Edit > [[options.clipboard_section]] chooses whether it pastes at the copied
position or in the middle of the view; [[menu.edit.paste_in_view]]
(<kbd>Ctrl+Shift+V</kbd>) always pastes in the middle of the window.

Copying in GraphDraw puts the objects on the clipboard as GraphDraw objects,
SVG, SVG text and a picture, so other programs can paste them too.
