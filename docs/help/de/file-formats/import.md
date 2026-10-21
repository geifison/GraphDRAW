# Dateien importieren

[[menu.file > menu.file.import]] (<kbd>Ctrl+I</kbd>) fügt den Inhalt einer
Datei in die geöffnete Zeichnung ein, auf der aktiven Ebene, als ein einziger
Rückgängig-Schritt. [[menu.file.open]] öffnet eine Datei dagegen als eigene
Zeichnung. Beide lesen die Formate, die unter
[Unterstützte Dateiformate](supported-formats.md#open) aufgeführt sind.

## Wohin importierte Objekte kommen {#placement}

Anfangs kommen importierte Objekte an die Stelle, an der sie in der Datei
liegen. Um sie stattdessen in der Ansicht zu zentrieren, ändern Sie die
Einstellung auf der Seite [[options.file_formats]] der globalen Optionen; siehe
[Globale Optionen](../customize/global-options.md#file-formats).

## Hinweise zu einigen Formaten {#formats}

- **PDF und AI**: Eine geschützte Datei fragt im Dialogfeld
  [[pdf_password.title]] nach ihrem Kennwort. Text behält seine Schriften,
  wenn sie installiert sind.
- **EPS und PostScript**: Jede gedruckte Seite wird zu einer Seite.
- **PSD**: Jede Ebene wird zu einer Bitmap mit Deckkraft, Sichtbarkeit und
  Maske.
- **DXF**: Die Zeichnungseinheiten werden aus der Datei übernommen.
- **Textdateien** (TXT, RTF, DOCX): werden zu Mengentext, mit Fett, Kursiv,
  Größen und Schriften, sofern die Datei sie hat.
- **Bitmaps**: werden in ihrer Auflösung platziert; siehe
  [Bitmaps](../bitmaps/index.md).

Wenn einige Objekte einer Datei nicht gelesen werden können oder Schriften
fehlen, weist eine Meldung darauf hin; siehe
[Hinweise nach dem Öffnen](../get-started/open-and-save.md#open-notes). Sie
können auch aus anderen Programmen einfügen; siehe
[Inhalte einfügen](../objects/paste-special.md).

## Dateien auf das Fenster ziehen {#drop}

Ziehen Sie eine oder mehrere Dateien aus dem Dateimanager auf das Fenster.
Solange sie darüber sind, zeigt die Zeichnung einen gestrichelten blauen
Rahmen und „Zum Importieren loslassen“. Jede Datei wird importiert und in die
Zeichnung eingebettet: die erste mittig an der Stelle, an der Sie sie
loslassen, die folgenden jeweils etwas weiter; alles Importierte wird
ausgewählt. Eine Datei, die GraphDraw nicht lesen kann, öffnet die Meldung
[[alert.unsupported_file]] mit ihrem Namen.
