// Keyboard model for the wheel. Pure logic, no DOM, so vitest can run it in node.

export type NavigationKey = "ArrowRight" | "ArrowLeft" | "ArrowDown" | "ArrowUp" | "Tab";
const NAVIGATION_KEYS: readonly NavigationKey[] = ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp", "Tab"];
export type KeyAction = "close" | "activate" | "tools";

// Shift only reverses Tab. Arrow keys already encode a direction, so holding Shift
// with an arrow should not flip it.
export function nextIndex(
  current: number,
  count: number,
  key: NavigationKey,
  shift: boolean,
): number {
  // Empty wheel: there is no item to move to. 0 is returned rather than NaN (from % 0)
  // so the value is always a finite number; callers must still treat count 0 as no selection.
  if (count < 1) {
    return 0;
  }
  const isBackward: boolean =
    key === "ArrowLeft" || key === "ArrowUp" || (key === "Tab" && shift);
  const direction: number = isBackward ? -1 : 1;
  // Double modulo keeps the result non-negative when stepping back from index 0.
  return (((current + direction) % count) + count) % count;
}

export function isNavigationKey(key: string): key is NavigationKey {
  return (NAVIGATION_KEYS as readonly string[]).includes(key);
}

export function actionForKey(key: string): KeyAction | null {
  switch (key) {
    case "Escape":
      return "close";
    case "Enter":
    case " ":
      return "activate";
    case "Alt":
      // Alt alone opens the Tools tab; the spec treats it as a hold-to-reveal modifier.
      return "tools";
    default:
      return null;
  }
}
