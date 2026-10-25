# Ubuntu 24.04 desktop baseline

Date: 2026-10-20. Validation ran in `<worktree>`, branch `codex/morphix-ubuntu-complete`, based on `36ecc3b`.

## Host snapshot

- OS: Ubuntu 24.04.5 LTS (Noble)
- GNOME Shell: 46.0; active session mode: `ubuntu`; session type: X11
- Files/Nautilus: 46.4 (`1:46.4-0ubuntu0.2`)
- DING package: `gnome-shell-extension-desktop-icons-ng 46+really47.0.9-1ubuntu5`; enabled and active before and after the isolated probe
- `python3-nautilus`: not installed. Noble apt metadata offers `python3-nautilus 4.0-1build4`; the integration package must declare this dependency if the provider is installed system-wide.
- `python3-gi`: 3.48.2-1

## DING source snapshot

Installed directory: `/usr/share/gnome-shell/extensions/ding@rastersoft.com`. Package copyright declares GPL-3.

| File | SHA-256 |
| --- | --- |
| `app/fileItemMenu.js` | `f3784240a959187747ab8b153f2e9a4c03d1120bc56849e99aec17d4eb994111` |
| `app/desktopManager.js` | `022c43d8587f2b41c4e60e9ec4ff6d1fcf059688d6ec66fce25c642d896bce50` |
| `metadata.json` | `f29cedcb83a8e1ac8ea4c205a48f5342d196e3930fe566023a49eba7066179e7` |
| `extension.js` | `bdc6c9460d4688e0b410417e664a715c409d5c9591bd8227adc544fdcde5b25f` |

## Isolated user-copy probe

A temporary DING copy was placed under `/tmp/morphix-ding-probe.68lcIo/data/gnome-shell/extensions/ding@rastersoft.com`, with a log marker added only to the temporary `extension.js`. GNOME Shell ran nested under a separate D-Bus session with temporary HOME, XDG data/config/cache/state directories. The system installation and active desktop files were not edited.

- `GNOME_SHELL_SESSION_MODE=user`: nested Shell logged that the user extension was installed and system copy would not be loaded; the temporary import and enable markers appeared. This proves the per-user override works in generic GNOME user mode.
- `GNOME_SHELL_SESSION_MODE=ubuntu`: nested Shell logged `Found user extension ding@rastersoft.com, but not loading from ... directory as part of session mode`; neither temporary marker appeared. Ubuntu's session mode prevents the user-copy approach from loading. The user later approved the package-managed DING patch with `dpkg-divert`; that design is tracked in the implementation plan.
- The nested Shell intentionally ran until timeout after 8 seconds to collect startup logs. The extension source was a temporary copy and is not installed.
- During the first isolated probe, the separately activated `dconf` service inherited the host HOME despite process environment overrides. `org.gnome.shell enabled-extensions` was restored immediately to the exact five IDs recorded before the probe and verified with `gsettings` and `gnome-extensions list --enabled`. No packages or active extensions were installed/removed. Future isolated probes set HOME/XDG variables on `dbus-run-session` itself.

The probe validates source precedence only; it does not validate the modified menus in an actual interactive desktop. The package-managed DING patch avoids the rejected per-user override and remains subject to Task 8's desktop acceptance.

## Baseline checks

- `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo test --workspace --locked`: pass after making the existing local PDFium library available in the worktree; 0 failures, 2 X11-session tests ignored by their existing annotations. Without the local library, two PDF CLI tests fail because the ignored/untracked 7.6 MB `third_party/pdfium/lib/libpdfium.so` is absent from a new worktree.
- UI: `npm run check` pass; `npm test` pass (7 files, 77 tests); `npm run build` pass.
- `cargo fmt --all -- --check`: fails on existing formatting differences in many untouched Rust files (first reported `crates/morphix-gui/src/history.rs`, followed by jobs, integration and other modules). No workspace-wide formatting was applied to avoid an unrelated bulk diff.
- No Wayland interactive session was available; this host is X11. The nested Wayland Shell probe above was not a real desktop session acceptance test.

## Result

The baseline is suitable for Files/app work. The user-copy DING design is incompatible with Ubuntu 24.04's active `ubuntu` session mode; package-level `dpkg-divert` integration was approved afterward. Interactive acceptance on Ubuntu GNOME, including Wayland and X11, is still required before claiming all four Files/Desktop menu locations work.

## Conversion audit — 2026-10-20

- `cargo test -p morphix-core --quiet`: 166 passed, 0 failed, 0 ignored. The suite exercised PDF text extraction with the local PDFium library and archive traversal/symlink, collision, source-preservation, and cancellation regressions.
- Isolated CLI smoke fixtures: image→PNG, FLAC→WAV, MKV→MP4, and CSV→TSV all completed; `ffprobe` recognized WAV/MP4 containers, PNG signature matched, and all source hashes stayed unchanged. The TSV writer correctly omitted quotes around a cell that contains neither delimiter nor quote; an initial smoke assertion expecting redundant quotes was corrected, with no product change.
- No reproducible conversion defect was found, so no core code was changed.

## Final package and session checks — 2026-10-20

- Final artifact: /tmp/morphix-ubuntu-final-r2-613558a/morphix_0.1.0-2_amd64.deb. SHA-256: dba9e4fd322111c78fb9ef9edbed142bc562dcff16c396d73b0c396a56a36cbf. Source includes GTK/Nautilus GI version pinning (cce0e18) and Debian revision 2 (613558a). The DING package was rebuilt from its diverted original files after verifying their manifest hashes.
- scripts/check-deb.sh passed on this artifact: 563 package entries, desktop file validation, Nautilus provider syntax, DING source/patch hashes, exact DING Depends, maintainer script syntax, dynamic library closure, and FFmpeg GPL/libx264 configuration (6.1.1-3ubuntu5, GLIBC 2.39 floor).
- APT simulation predicted an upgrade from morphix 0.1.0-1 to 0.1.0-2, with no other package changes. APT installed 0.1.0-2 successfully. The initial installation added python3-nautilus and gir1.2-nautilus-4.0; the revision upgrade added or removed no dependencies.
- Post-install dpkg -V morphix reported no modified package files. morphix doctor reports the Nautilus extension, Desktop DING menu, app launcher, and tray support installed. DING was reloaded and is ACTIVE; diverted original hashes and patched-file hashes match the manifest.
- Removed the unowned old ~/.local/bin/morphix binary. morphix uninstall-integration --json removed the old Nautilus Scripts entry, Dolphin menu, autostart entry, Nautilus accelerator, and Alt+C shortcut. Morphix user data under ~/.local/share/morphix was preserved. No Snap or Flatpak Morphix install was present.
- The installed Nautilus provider imported under /usr/bin/python3 with G_DEBUG=fatal-warnings; GTK 4 and Nautilus 4 were selected without PyGI warnings. All 8 Nautilus provider tests passed.
- Earlier full checks passed: Clippy; Cargo workspace tests (437 passed, 0 failed, 3 ignored; the two read-only X11 tests then passed explicitly); UI checks/build and 80 tests; 15 maintainer/DING Python tests; GJS argv test; and package checks.
- cargo fmt --all -- --check still reports existing formatting differences in untouched Rust files, including history.rs, jobs.rs, settings.rs, and IPC tests. No workspace-wide formatting was applied. DING reload emitted non-fatal warnings from DING itself (GLib.unix_signal_add deprecation and keybinding replacement); DING returned to ACTIVE with no Morphix exception.

### Conversion click follow-up — 2026-10-20

- Fixed a blur/click race in the quick menu: defer blur dismissal briefly so X11 can dispatch the selected format, and keep errors visible for the user. Updated the pure blur-policy test and made the DING source directory overridable during package builds so revision upgrades can use verified, clean diverted originals.
- Built and installed `morphix 0.1.0-3` on the active session. Package checker passed; `dpkg -V morphix` was clean and `morphix doctor --json` reported integration success.
- Opened the installed GUI wheel with a byte-for-byte copy of the reported Desktop JPEG, selected PNG, and observed `/tmp/morphix-actual-jpeg-gui-20261020.png` beside the copy. The image is 1024×1024, the GUI history records `1 thành công, 0 lỗi`, and the output is 942,298 bytes. The original Desktop directory was left untouched.
- UI `npm run check` and `npm test` passed (8 files, 80 tests); `git diff --check` passed.

### Still unverified before calling desktop acceptance complete

- Installation was done on the active Ubuntu 24.04.5 / GNOME 46 / X11 machine, not a clean VM.
- The four right-click menus were not visually opened (Files on a file/background and Desktop on a file/background). doctor, DING state/hash checks, provider import, and provider tests confirm installation and loadability, not visible menu interaction.
- The follow-up conversion used the installed GUI wheel opened through `morphix menu`, not a literal right-click. File picker/drop, progress/cancel, and conversion initiated from each right-click route remain unverified after installation.
- A live package removal and DING restoration were not performed; maintainer lifecycle was tested earlier with real dpkg-divert in isolated temporary admin databases. No Wayland session was available.
- Public redistribution remains out of scope: Morphix copyright still says its license is unselected, and GPL source compliance requires a separate release review.
