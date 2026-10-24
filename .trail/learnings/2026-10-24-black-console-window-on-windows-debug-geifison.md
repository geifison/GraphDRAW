---
title: Black console window on Windows debug builds
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 6, tokens: 50475 }
touches:
  - AGENTS.md
  - ATTRIBUTION.md
  - apps/graphdraw/assets/icons/color/align-bottom.svg
  - apps/graphdraw/assets/icons/color/align-center-h.svg
  - apps/graphdraw/assets/icons/color/align-center-v.svg
  - apps/graphdraw/assets/icons/color/align-left.svg
  - apps/graphdraw/assets/icons/color/align-right.svg
  - apps/graphdraw/assets/icons/color/align-top.svg
  - apps/graphdraw/assets/icons/color/dist-bottom.svg
  - apps/graphdraw/assets/icons/color/dist-center-h.svg
---

## Decided
apps/graphdraw/src/main.rs uses `#![windows_subsystem = "windows"]` unconditionally (it was only for release, so debug builds opened a console next to the splash). `attach_parent_console` (kernel32 AttachConsole via a local extern block, no new dependency) borrows the parent shell's console only when STD_ERROR_HANDLE is null, so logs still show when started from cmd.

## Tried and ruled out
Adding windows-sys just for two kernel32 calls: unnecessary.

## Watch out
Inherited handles (cargo run, pipes for `--mcp`) are left alone; do not call AttachConsole unconditionally or piped MCP stdout could be disturbed. Verify with `objdump -p target/debug/graphdraw.exe | grep Subsystem` (expect Windows GUI).
