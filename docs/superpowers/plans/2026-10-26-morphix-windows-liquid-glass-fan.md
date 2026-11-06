# Morphix Windows Liquid Glass Fan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update the Explorer fan to the approved larger, rounded liquid-glass half-wheel without changing conversion or wheel behavior.

**Architecture:** Keep the existing SVG/Tauri overlay and Shell extension. Generate rounded SVG sectors, resize the owner-drawn submenu to match, and clip its native popup backing to the fan silhouette while preserving menu tracking.

**Tech Stack:** TypeScript, SVG/CSS, Vitest, Rust Windows Shell extension, Tauri/WebView2.

**Spec:** `docs/superpowers/specs/2026-10-26-morphix-windows-liquid-glass-fan-design.md`

## Global Constraints

- Keep the Explorer-only, half-wheel interaction and existing conversion pipeline.
- Keep format labels inside sectors and leave the center without a format label.
- Use pale translucent sectors, a Morphix teal selected sector, rounded geometry, and soft shadows.
- Do not add dependencies or rewrite IPC, Shell integration, or Tauri window ownership.
- Keep the fan beside the context menu and test the native popup after clipping.

## Review Focus

- **One or few compatible formats:** sectors and labels remain valid when count is 1–5. Test in Task 1 with counts 1, 2, and 5.
- **Rounded corners and gaps:** each sector has four rounded joins and adjacent sectors remain visibly separate. Test in Task 1 by asserting rounded path commands and geometry bounds.
- **Windows display scaling:** larger menu dimensions still align with the fan. Test in Task 2 with physical anchor dimensions and an Explorer smoke test.
- **Pointer travel through transparent areas:** clipped popup remains open while hovering and scrolling the Tauri overlay. Test in Task 2 manually in Explorer.
- **Other menu presentation:** the existing full-size wheel remains unchanged. Test in Task 1 with current UI suite/build and in Task 2 by opening the normal Morphix menu.

---

### Task 1: Rounded liquid-glass SVG fan

**Files:**
- Modify: `crates/morphix-gui/ui/src/wheel.ts`
- Modify: `crates/morphix-gui/ui/src/main.ts`
- Modify: `crates/morphix-gui/ui/src/styles.css`
- Test: `crates/morphix-gui/ui/tests/wheel.test.ts`

**Interfaces:**
- Keep `fanSectorPath(index: number, count: number): string` and `fanLabelPosition(index: number, count: number): Point` as the existing callers' interfaces.
- Set fan geometry to `width: 150`, `height: 300`, `outer: 145`, `inner: 56`, `centerX: 0`, `centerY: 150` so the SVG and native item share one physical layout.
- Use a 4-degree angular gap and 6 px corner radius, clamped so a sector remains non-degenerate for counts 1–5.

- [x] **Step 1: Add `fan_sector_path_rounds_all_four_corners` and `fan_sector_path_stays_within_fan_bounds`** in `wheel.test.ts` for counts 1, 2, and 5. Assert two `A` commands, four `Q` commands, a closing `Z`, bounds within 150 × 300, and labels at each sector's radial midpoint.
- [x] **Step 2: Run the focused UI test** from `crates/morphix-gui/ui` with `npm test -- --run tests/wheel.test.ts`; confirm the new geometry assertions fail against the current paths.
- [x] **Step 3: Implement rounded sectors** in `wheel.ts` using arc insets, quadratic corner curves, the 4-degree gap, and the 6 px corner radius; update `FAN_WHEEL` to the stated dimensions.
- [x] **Step 4: Apply glass styling** in `styles.css`: translucent light fill, restrained edge highlight, per-sector soft drop shadow, stronger selected shadow, and high-contrast labels. Keep the center hidden. Update `main.ts` only if needed to expose a stable sector index/class for styling.
- [x] **Step 5: Run UI verification** from `crates/morphix-gui/ui`: `npm test`, `npm run check`, and `npm run build`; confirm all pass.

### Task 2: Match and clip the native Explorer popup

**Files:**
- Modify: `crates/morphix-shell/src/windows.rs`
- Modify: `crates/morphix-gui/src/windows.rs` only if sizing must be adjusted beyond the existing `anchor.height / 2` by `anchor.height` rule.
- Test: `crates/morphix-shell/src/windows.rs` unit tests, plus manual Explorer verification.

**Interfaces:**
- Keep `popup_rect(process_id: u32, menu_id: usize) -> Option<(i32, i32, u32, u32)>` for CLI and Shell callers.
- Set the owner-drawn item to `150 × 300` physical pixels so the overlay window and SVG fan use the same bounds.

- [x] **Step 1: Add `native_fan_measurement_matches_overlay`** in the Windows Shell tests. Call the real `WM_MEASUREITEM` handler and assert the owner-drawn item measures 150 × 300, matching the Tauri fan bounds.
- [x] **Step 2: Run `cargo test --locked -p morphix-shell`** from the workspace; confirm the measurement assertions fail against the current 110 × 220 item.
- [x] **Step 3: Update native menu measurement and clipping** so the visible popup backing follows a right-facing half-disc computed from the measured item rectangle. Use `SetWindowRgn` on the matching popup HWND; keep the blank owner-draw row and existing asynchronous IPC path.
- [x] **Step 3a: Clip the Explorer popup, Tauri fan HWND, and WebView2 child surfaces to the same semi-ring** after screenshots and live HWND inspection showed the native submenu still painted behind the center; clear the Explorer popup region when its submenu closes and clear GUI regions before the full-size wheel opens.
- [x] **Step 4: Run `cargo test --locked -p morphix-shell`** and `cargo test --locked -p morphix-gui`; confirm both pass.
- [x] **Step 5: Build and deploy versioned Windows binaries** with the embedded UI; verify `doctor --json` reports the Explorer handler installed and the running GUI points to the new version.
- [x] **Step 6: Verify in Explorer** on the existing image fixture: the native menu appears without a square backing, the rounded fan aligns beside it, wheel scrolling changes the active sector while the menu stays open, and choosing a format converts the fixture. Also open the normal full-size Morphix menu and confirm it is unchanged.

**Ruling:** The native popup region itself is verified through Explorer, not a synthetic popup unit test, because Explorer owns its menu HWND and tracking lifecycle. The measurement contract has a unit test; clipping and continued scrolling remain an explicit physical acceptance check.

## Completion check

- `git diff --check -- .` reports no whitespace errors.
- The UI, Shell, and GUI checks above pass.
- Final review confirms only the approved visual scope changed; report any unverified DPI or format cases plainly.
