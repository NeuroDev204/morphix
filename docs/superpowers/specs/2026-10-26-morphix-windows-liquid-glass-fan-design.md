# Morphix Windows Liquid Glass Fan

## Intent

Polish the Explorer format fan to match the approved mockup: a larger half-wheel beside the native context menu, with separate rounded glass sectors.

## Goals

- Keep the Explorer-only context menu, scroll selection, and existing conversion pipeline.
- Give each compatible format its own curved SVG sector with genuinely rounded corners, a subtle translucent tint, a soft highlight, and a visible shadow.
- Size the half-wheel close to the adjacent context-menu panel, accounting for Windows display scaling.
- Show format labels only inside sectors; keep the center empty.
- Hide the rectangular native submenu backing so the fan silhouette has no square outer frame.

## Non-goals

- No full circular wheel, flower layout, center label, new formats, or conversion behavior changes.
- No rewrite of the Shell extension, IPC, or Tauri window architecture.
- No dependency on desktop blur being available; translucent glass gradients remain the fallback.

## Approaches

1. **Extend the current SVG/Tauri fan (chosen).** Use curved sector paths, glass tints, highlights, and per-sector shadows. This reuses the working input and display path with the smallest implementation change.
2. **Replace it with a native Win32 renderer.** This may provide tighter control over Windows clipping and backdrop effects, but expands rendering and input handling.

The user chose the first approach after reviewing the Tangerine-inspired mockup.

## Constraints and implementation calls

- The Shell submenu currently owns a rectangular popup item. Clip or hide that rectangle without breaking Explorer menu tracking or wheel input.
- Rounded corners must come from sector geometry, not only from a thick stroke.
- Keep the selected sector legible with Morphix teal and high-contrast text; use restrained pale tints for inactive sectors.
- Keep current scroll cycling, selection, IPC session handling, and conversion semantics unchanged.
- Verify appearance at the user's Windows display scale and confirm the native menu remains usable.

## Approval state

The user approved the visual direction in `tangerine-inspired-fan-rounded-v9.html` and chose the current SVG/Tauri renderer. Implementation awaits approval of this design note.
