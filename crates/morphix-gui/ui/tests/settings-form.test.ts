import { describe, expect, it } from "vitest";
import { formFromSettings, settingsFromForm, type Settings, type SettingsForm } from "../src/settings-form";
import { S } from "../src/strings";

const defaults: Settings = { quality: "balanced", output_dir: null, default_resize_percent: 50 };

const formWith = (overrides: Partial<SettingsForm>): SettingsForm => ({
  quality: "balanced",
  outputDir: "",
  resizePercent: "50",
  ...overrides,
});

describe("formFromSettings", () => {
  it("shows the default folder as an empty field and the percent as text", () => {
    expect(formFromSettings(defaults)).toEqual({ quality: "balanced", outputDir: "", resizePercent: "50" });
  });

  it("keeps a chosen output folder", () => {
    const settings: Settings = { ...defaults, output_dir: "/a/Ảnh xuất" };
    expect(formFromSettings(settings).outputDir).toBe("/a/Ảnh xuất");
  });
});

describe("settingsFromForm", () => {
  it("maps a complete form to settings, empty folder becomes null", () => {
    const parsed = settingsFromForm(formWith({ quality: "high", resizePercent: "30" }));
    expect(parsed).toEqual({
      ok: true,
      settings: { quality: "high", output_dir: null, default_resize_percent: 30 },
    });
  });

  it("treats a blank folder as the default and keeps a real one", () => {
    expect(settingsFromForm(formWith({ outputDir: "   " }))).toMatchObject({
      ok: true,
      settings: { output_dir: null },
    });
    expect(settingsFromForm(formWith({ outputDir: "/out" }))).toMatchObject({
      ok: true,
      settings: { output_dir: "/out" },
    });
  });

  it("accepts the bounds 1 and 100 and trims spaces around the number", () => {
    expect(settingsFromForm(formWith({ resizePercent: "1" }))).toMatchObject({
      ok: true,
      settings: { default_resize_percent: 1 },
    });
    expect(settingsFromForm(formWith({ resizePercent: " 100 " }))).toMatchObject({
      ok: true,
      settings: { default_resize_percent: 100 },
    });
  });

  it.each(["0", "101", "", "abc", "12.5", "-3", "+5"])("rejects %j with the range message", (value: string) => {
    expect(settingsFromForm(formWith({ resizePercent: value }))).toEqual({
      ok: false,
      message: S.resizeRangeError,
    });
  });
});
