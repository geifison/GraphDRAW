# Privacy policy

GraphDraw is a vector editor that runs on your own computer. This policy
covers the desktop program (installer, portable zip, Microsoft Store,
macOS and Linux packages), `graphdraw-cli` and the browser version.

## What GraphDraw collects

Nothing. GraphDraw has no accounts, no telemetry, no analytics, no crash
reporting and no advertising. It does not send your drawings, your files
or anything about you or your computer anywhere, except what you ask AI
Drawing to send to the AI service you set up yourself (see below).

## What stays on your computer

- Your drawings, where you save them.
- Preferences, the Shelf and thumbnails of recent drawings, in
  GraphDraw's preferences folder (in the browser version: the page's local
  storage in your browser). Deleting that folder removes them; Options >
  Global > Reset all settings puts the preferences back as they came.
- Automatic backups of open drawings, in the folder set in Options > Save
  (by default a `GraphDraw` folder in your system's temporary folder).
- AI Drawing's settings (service, address, model) with the preferences;
  your API key only if you tick "Remember the key on this computer", and
  then in plain text in the preferences file (in the browser version: the
  page's local storage). Otherwise the key is forgotten when GraphDraw
  closes.

## When GraphDraw uses the network

Only when you ask it to:

- Help, the project's pages and links you click open in your web
  browser, which then follows its own privacy policy.
- Send To > Mail hands a file to your mail program; you choose whether to
  send it.
- `graphdraw --mcp` talks over standard input and output with the
  program that started it, on your computer; it opens no network port.
- Options > Global > General > "Let AI agents on this computer edit the
  open drawing" (off unless you turn it on) opens a port on your
  computer only (`127.0.0.1`), guarded by a key kept in the preferences
  folder, for `graphdraw --mcp-window`; nothing outside your computer can
  connect.
- AI Drawing (Tools > AI Drawing), only when you press Draw: it sends
  your request, the earlier requests of the same conversation and a
  description of the active page (its size, layers and objects: types,
  positions, colours and text; no file names or pictures) to the AI
  service whose address you entered, with your API key. That service
  handles them under its own terms and privacy policy. GraphDraw has no
  AI service of its own and sends nothing to its authors.

## The browser version

The browser version at <https://geifison.github.io/GraphDRAW/> is a set of static
files served by GitHub Pages. GraphDraw itself sends nothing (AI Drawing
aside, as above, from your browser to the service you set up). When a
drawing's text needs a font of the page's font catalog, or the pointer
rests on one in a font list, the page fetches that font's files from
GitHub's file server (`raw.githubusercontent.com`). GitHub, as the host
of both, may log visits as described in the
[GitHub Privacy Statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).
Files you open in it stay in your browser.

## Contact

Questions about this policy: open an issue at
<https://github.com/geifison/GraphDRAW/issues>.

Last updated: 2026-10-12.
