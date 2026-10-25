import { describe, expect, it } from "vitest";
import { ownsJobEvent, shouldHideOnBlur } from "../src/focus";

describe("shouldHideOnBlur", () => {
  it("ignores a blur right after the menu is shown", () => {
    expect(shouldHideOnBlur(1100, 1000, false)).toBe(false);
  });
  it("hides on a blur after the grace period", () => {
    expect(shouldHideOnBlur(1700, 1000, false)).toBe(true);
  });
  it("keeps the window visible while work or action feedback needs attention", () => {
    expect(shouldHideOnBlur(1100, 1000, true)).toBe(false);
    expect(shouldHideOnBlur(9000, 1000, true)).toBe(false);
    expect(shouldHideOnBlur(9000, null, true)).toBe(false);
  });
  it("hides when no show time is known (old behaviour)", () => {
    expect(shouldHideOnBlur(1100, null, false)).toBe(true);
  });
});

describe("ownsJobEvent", () => {
  it("ignores events of a job started elsewhere (Khay)", () => {
    expect(ownsJobEvent(7, 3, true)).toBe(false);
    expect(ownsJobEvent(7, null, false)).toBe(false);
  });
  it("accepts its own job, even before run_action replies with the id", () => {
    expect(ownsJobEvent(3, 3, false)).toBe(true);
    expect(ownsJobEvent(3, null, true)).toBe(true);
  });
});
