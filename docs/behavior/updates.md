# Updates

Desktop only (the browser build is always the latest; there Help >
Updates opens the releases page).

- Help > Updates asks GitHub for the project's latest release
  (`api.github.com/repos/geifison/GraphDRAW/releases/latest`) and says
  whether it is newer than this version (numbers compared as major,
  minor, patch; drafts, pre-releases and the nightly build never count).
- A newer release offers Download (the file for this system: the
  Windows installer, else the portable zip; the macOS disk image for the
  Mac's processor; the Linux AppImage, else the tar.gz; the release page
  when none fits), Release notes, Skip this version and Close. Download
  opens the link; the system's installer does the installing.
- Install in place (Linux AppImage only): when GraphDraw runs as an
  AppImage (the AppImage runtime names its file in `APPIMAGE`) and the
  release's AppImage has a SHA-256 digest (GitHub gives one for every
  uploaded file), the dialog offers Install first. It downloads the
  AppImage next to the running one (`.<name>.update`), showing the
  megabytes done, checks its size and digest, gives it the running
  file's permissions and moves it over the running file. A short, long
  or mismatched download is deleted and the running file is left as it
  was; the dialog says why and offers Download. The new version runs
  from the next start. Windows, macOS, the `.deb` and the tar.gz keep
  Download and the system's installer.
- Options > Global > General, Check for updates at start (on by
  default): once a day, the same check runs in the background at start;
  only a newer release that was not skipped is shown, nothing otherwise.
- A failed check (no connection) says why and offers the releases page.

## Checks

- `updates::tests::versions_compare_by_number`
- `updates::tests::the_download_fits_the_system`
- `updates::tests::the_same_or_older_drafts_and_pre_releases_are_not_offered`
- `ui::dialogs::tests::every_dialog_draws_and_accepts_enter` (the dialog)
- Checked against GitHub from the app: 0.4.0 reads as the latest version.
- `updates::tests::the_appimage_comes_with_its_digest`
- `updates::tests::installing_replaces_the_file_only_when_it_checks_out`
- `ui::dialogs::tests::the_updates_dialog_draws_every_state`
- `updates::tests::live_install_from_the_latest_release` (run by hand,
  needs the network): the 0.4.0 AppImage downloaded through GitHub's
  redirect, its digest checked and the file put in place.
