import { describe, expect, it } from "vitest";
import { actionForKey, isNavigationKey, nextIndex } from "../src/keys";

describe("keyboard", () => {
  it("arrows wrap around the wheel", () => {
    expect(nextIndex(7, 8, "ArrowRight", false)).toBe(0);
    expect(nextIndex(0, 8, "ArrowLeft", false)).toBe(7);
  });
  it("tab moves forward and shift-tab backward", () => {
    expect(nextIndex(2, 5, "Tab", false)).toBe(3);
    expect(nextIndex(2, 5, "Tab", true)).toBe(1);
  });
  it("escape closes, enter and space activate, alt opens tools", () => {
    expect(actionForKey("Escape")).toBe("close");
    expect(actionForKey("Enter")).toBe("activate");
    expect(actionForKey(" ")).toBe("activate");
    expect(actionForKey("Alt")).toBe("tools");
    expect(actionForKey("a")).toBeNull();
  });
});

describe("empty wheel", () => {
  it("nextIndex returns 0 instead of NaN when there are no items", () => {
    expect(nextIndex(0, 0, "ArrowRight", false)).toBe(0);
    expect(nextIndex(0, 0, "Tab", true)).toBe(0);
  });
});

describe("navigation keys", () => {
  it("accepts arrows and Tab only", () => {
    expect(isNavigationKey("ArrowLeft")).toBe(true);
    expect(isNavigationKey("Tab")).toBe(true);
    expect(isNavigationKey("Enter")).toBe(false);
  });
});
