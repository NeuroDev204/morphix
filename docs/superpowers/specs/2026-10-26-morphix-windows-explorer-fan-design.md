# Morphix Windows Explorer fan

Status: native implementation built and registered for the current user; integrated fan acceptance is pending.

## Goal

Convert selected files from Windows Explorer or the Desktop through a compact half-wheel beside the native context menu.

## User decisions

- The feature applies only to files selected in Explorer or on the Desktop.
- The context menu contains a Morphix entry. Opening it shows a half-circle fan beside the menu.
- The fan is about the height of the native context menu. Format labels appear in the fan; there is no format label in the center.
- The mouse wheel moves the active format. Choosing a sector runs the existing conversion for every selected file.
- Keep the native context menu open while the fan is being used.
- Use the existing format compatibility and conversion pipeline; do not create a second conversion path.

## Proposed first release scope

- Target the classic Shell context menu available through Show more options. The existing Windows integration is a per-user registry verb and does not have package identity.
- Keep the Windows 11 modern context menu as a later option. Microsoft requires an IExplorerCommand extension with packaged or sparse-package identity for that menu.
- Handle right-click fan activation first. Windows Alt+C selection capture is a separate subsystem; the current GUI reports that selection capture is Linux-only.
- Preserve the existing full-wheel presentation for other triggers and preserve Linux behavior.

## Evidence and remaining check

- A Windows overlay probe received physical WM_MOUSEWHEEL messages while the pointer was over it.
- The user confirmed that the physical-wheel probe changed its label while the native menu stayed open.
- The GUI now renders a compact half-wheel and handles accumulated wheel input through the existing compatible-format and conversion APIs.
- The integrated fan still needs physical scrolling, conversion, Escape, Desktop, and multi-selection acceptance checks.

## Implementation verification

- UI: 85 tests and production build passed.
- GUI: 83 tests passed. IPC: 14 integration tests passed. Windows registry: 10 tests passed.
- CLI: 38 tests passed; two PDF extraction tests require the missing PDFium runtime.
- The full integration suite has 23 Windows failures in existing Linux-oriented fixtures and expectations; the Windows registry subset passes.
- Rust formatting passes for the changed files using workspace edition 2021. The whole-workspace formatting check reports existing differences in `crates/morphix-gui/tests/jobs.rs`.
- Current trial artifacts are in `target/morphix-native-v2`; the GUI was built with `custom-protocol` to embed `ui/dist`.
- A separate CLI process initially lost its request on exit because local-socket `flush()` is a no-op. The Windows sender now drains the underlying named pipe; the regression failed before the fix and passes afterward. Direct fan display and session-matched close were confirmed, and the 14 IPC tests still pass.
- A native popup probe confirmed that the initial menu notifications precede final positioning. The Shell bridge now sends its process ID and submenu handle; CLI waits up to 500 ms for the matching visible popup before resolving its physical rectangle. A timed native-menu probe opened the fan at the measured popup anchor and closed it by session.

## Constraints

- Install and remove integration for the current Windows user only; do not require administrator access.
- Do not modify source files or overwrite outputs during conversion.
- Do not activate or focus the overlay if that closes the native menu.
- Keep the shell extension's menu callbacks fast; conversion work remains out of process.
