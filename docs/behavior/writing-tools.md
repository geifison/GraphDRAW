# Writing tools

Text > Writing Tools: Spell Check, Grammar, Thesaurus and Autocorrect,
plus Text > Custom Optional Hyphens, Text > Encode and Text > Make Text
Web Compatible.

## Writing Tools dialog

Text > Writing Tools > Spell Check (Ctrl+F12), Grammar and Thesaurus open
one dialog on that tab: Spell Checker, Grammar and Thesaurus, with "?"
(the behaviour pages) and Close at the bottom. The checkers look at every
text object of the page (Check: Document) or the selected ones (Check:
Selection).

## Spell Checker

System Hunspell word lists (see `text.md`). Left: Replace with, the
Replacements list (suggestions; a click puts one in Replace with, a
double click replaces with it) and the status ("Ready: Begin spell
checking", the word not found, then "Spell check completed"). Right:
Start (Replace once a word is shown), Auto Replace (every place the word
is), Skip Once, Undo (the last replacement), Skip All (for this check),
Options (ignore words with numbers, ignore words in capitals), Add (to
the user word list) and Check.

## Grammar

Rule-based checks: doubled words, double spaces, a space before
punctuation, a missing space after punctuation, lower-case sentence
starts, unbalanced brackets, unbalanced quotes, repeated punctuation,
sentences over 40 words, a paragraph's last sentence of three words or
more with no end mark, and, for English, "a" before a vowel sound or
"an" before a consonant ("an hour", "a one-off" and "a university" are
right). The tab shows the replacement, the new sentence and the rule
name; Start (Replace once a finding is shown), Undo, Skip Once, Skip Rule
(for this check) and Check. When no finding is left it asks "Grammar
check completed. Close the grammar checker?": Yes closes the dialog, No
keeps it open for a new check.

## Thesaurus

The word field (the first word of the selected text at first) with the
words looked up before in its list, Look Up (or Enter), Insert and
Options (close after inserting; pick a MyThes `.dat` file). Sources: a
built-in English list for the most common words, or the MyThes file,
which is how the open-source office suites ship their thesauri. The
meanings show with their synonyms; a click picks one, a double click
looks it up. Insert swaps the word in the text keeping its
capitalisation, or types the synonym at the caret of the text being
edited.

## Custom Optional Hyphens

Text > Custom Optional Hyphens, and the gear of the Text docker's
Paragraph section. Words the user hyphenates by hand ("Graph-Draw"),
kept in the preferences with their language, break only there when
their text is hyphenated (they win over the patterns, capitals or not).

- Automatically insert custom optional hyphens: When typing (a text that
  holds a defined word when its editing ends gets hyphenation on) and
  When pasting and importing text (the same for pasted and imported
  text objects). Both on at first.
- Language (the interface's and English, or every one with Show all
  languages), Word, Hyphenated Word (hyphens where the word may break;
  the letters must match the word), Add Definition, Remove Definition,
  and the list of definitions (Word | Hyphenated Word) for the language.
- "?", Apply to Selection (keeps the definitions and turns hyphenation on
  for the selected texts that hold a defined word), OK (keeps them) and
  Cancel.

## Checks

- `ui::writing_tools::tests::every_tab_draws`,
  `ui::writing_tools::tests::spell_options_and_skips_leave_words_out`,
  `ui::writing_tools::tests::grammar_findings_skip_the_rules_skipped`,
  `ui::writing_tools::tests::hyphenated_words_give_break_points`,
  `ui::writing_tools::tests::custom_hyphens_turn_hyphenation_on_and_break_words`,
  `grammar::tests::a_and_an_follow_the_next_sound_and_sentences_need_an_end`,
  `tests::custom_hyphens_come_before_the_patterns` (text crate).

## Autocorrect

Applied while typing (`App::update_text`), per the Autocorrect dialog:

- capitalise the first letter of sentences (default on),
- correct two initial capitals ("THe" to "The", default on),
- typographic quotes (default on): `"` and `'` become opening or closing
  quotes depending on the character before them,
- replacement table: `(c)` ©, `(r)` ®, `(tm)` ™, `1/2` ½, `1/4` ¼,
  `3/4` ¾; the user can add rows. The quote style follows the UI
  language.

Autocorrect never changes text that was pasted or imported, only typed
characters.

## Encode

Text > Encode re-interprets the characters of the selected text: each
character is mapped back to the byte it came from under the "read as"
encoding and the bytes are decoded again with the right one. Encodings:
UTF-8, Windows-1252, ISO-8859-1, Mac OS Roman, Windows-1251. Characters
that cannot have come from the source encoding are kept. The dialog
previews the result before OK.

Check: "cafÃ©" read as Windows-1252, really UTF-8, becomes "café"; a
Cyrillic word shown as "Ïðèâåò" (read as Latin-1, really Windows-1251)
becomes "Привет".

## Make Text Web Compatible

For each selected text object: gradient, pattern, texture and mesh fills
become a solid colour (first stop, front colour, first node); the outline,
drop shadow and live effects are removed; per-character fills that are
not solid are cleared. Solid fills and character colours are kept.
