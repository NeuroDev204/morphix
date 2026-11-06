# Windows Explorer fan Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:executing-plans. Implement task-by-task and verify each deliverable.

**Goal:** Open a compact half-wheel from a Windows Explorer or Desktop context menu, scroll through compatible formats, and run Morphix's existing conversion pipeline while the native menu stays open.

**Architecture:** Add a Windows Shell bridge that forwards the selected paths and native menu anchor to the existing Tauri GUI without taking activation. Carry an ExplorerFan presentation mode through the existing local IPC, render the half-wheel in the existing menu UI, and call the current menu_payload/run_action path. The first release uses the classic menu under Show more options; modern Windows 11 integration requires packaged or sparse-package identity and is excluded here.

**Tech Stack:** Rust workspace, existing Tauri 2 GUI and local IPC, TypeScript/Vite UI, Windows Shell COM APIs, existing morphix-core conversions.

**Spec:** docs/superpowers/specs/2026-10-26-morphix-windows-explorer-fan-design.md, supplementing docs/superpowers/specs/2026-10-19-morphix-design.md.

## Global Constraints

- Only selected files in Explorer and on the Desktop activate the fan.
- Install and remove Shell integration per-user through the existing install-integration command.
- All conversions go through the current compatibility and job APIs.
- The overlay must not take focus in a way that dismisses the native context menu.
- Leave current full-wheel behavior and Linux behavior unchanged.
- Windows Alt+C selection capture remains a separate plan.

## Review Focus

- Shell menu notifications may not arrive when expected: the Windows prototype must prove activation and native-menu persistence before building the full UI.
- High-resolution wheel input may send partial or multiple deltas: accumulate and move sectors predictably in either direction.
- Multi-select, Unicode paths, and filenames with spaces must survive Shell-to-CLI-to-GUI IPC unchanged.
- HiDPI, multiple monitors, and screen edges must keep the fan beside the correct menu without clipping.
- Escape, conversion failure, and successful conversion must not leave the Shell extension or overlay stuck.

---

### Task 1: Prove the Windows Shell bridge

**Files:**
- Create: crates/morphix-shell/Cargo.toml and crates/morphix-shell/src/lib.rs
- Modify: Cargo.toml workspace members and crates/morphix-cli/src/integration/windows.rs
- Test: crates/morphix-shell Windows tests and an Explorer manual acceptance check

**Interfaces:**
- Consumes: selected Shell items and the context-menu row rectangle.
- Produces: a per-user Morphix submenu whose menu-open notification launches or contacts the GUI without taking focus.

- [ ] Add a Windows-only COM context-menu handler that inserts Morphix and records selected file paths.
- [ ] Add a minimal Windows prototype that opens a no-activate window beside the Morphix row and proves the native menu remains visible while the window receives physical wheel input.
- [ ] Add a failing ownership test for install, inspect, and uninstall of the handler registration.
- [ ] Run the focused Windows build and manual Explorer check; do not continue to the UI until the parent menu remains open.

### Task 2: Carry Explorer presentation through IPC

**Files:**
- Modify: crates/morphix-ipc/src/lib.rs
- Modify: crates/morphix-cli/src/main.rs
- Modify: crates/morphix-gui/src/state.rs and crates/morphix-gui/src/windows.rs
- Modify: crates/morphix-gui/ui/src/menu-types.ts and crates/morphix-gui/ui/src/main.ts
- Test: crates/morphix-ipc/tests/ipc.rs and crates/morphix-gui/tests/windows.rs

**Interfaces:**
- Consumes: selected paths and physical screen coordinates from Task 1.
- Produces: MenuPresentation::Full or MenuPresentation::ExplorerFan, plus an optional physical-pixel anchor for the fan.

- [ ] Add serialized presentation and anchor fields with defaults so old Full-wheel requests keep working.
- [ ] Route ExplorerFan requests through the existing GUI request and single-instance flow.
- [ ] Add tests proving presentation and anchor survive encode/decode and missing fields default to Full.
- [ ] Run the IPC and focused GUI tests.

### Task 3: Render and operate the half-wheel

**Files:**
- Modify: crates/morphix-gui/ui/src/wheel.ts
- Modify: crates/morphix-gui/ui/src/main.ts
- Modify: crates/morphix-gui/ui/src/styles.css
- Modify: crates/morphix-gui/tauri.conf.json and crates/morphix-gui/src/windows.rs
- Test: crates/morphix-gui/ui/tests/wheel.test.ts

**Interfaces:**
- Consumes: MenuPresentation::ExplorerFan, the native anchor, and the existing WheelPayload.
- Produces: a half-wheel sized to the menu, labels only in the fan, and the currently selected WheelItem after wheel input.

- [ ] Add pure half-wheel geometry and tests for sector labels and hit locations.
- [ ] Add wheel-delta handling that updates the focused sector and preserves the full-wheel click/keyboard behavior.
- [ ] Hide the center summary only in ExplorerFan mode.
- [ ] Position and size the Tauri window from the supplied anchor without activating it.
- [ ] Run the UI test and build commands from crates/morphix-gui/ui/package.json.

### Task 4: End-to-end Windows acceptance

**Files:**
- Modify: existing Windows integration and acceptance-test documentation only as needed.

**Interfaces:**
- Consumes: registered Shell bridge, ExplorerFan IPC request, and existing conversion actions.
- Produces: verified Explorer and Desktop conversion flow.

- [ ] Test a single selected file and a multi-selection from Explorer and Desktop.
- [ ] Confirm every offered format is compatible, both wheel directions change the active item, and selecting it uses the existing core action.
- [ ] Confirm the native context menu remains open during physical scrolling and conversion.
- [ ] Confirm Escape closes only the fan and install/uninstall leaves unrelated registry data untouched.
