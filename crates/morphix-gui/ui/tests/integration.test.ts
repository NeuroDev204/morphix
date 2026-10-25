import { describe, expect, it } from "vitest";
import { integrationViewFromDoctor } from "../src/integration";

describe("integrationViewFromDoctor", () => {
  it("shows package states and repair guidance without treating partial failures as healthy", () => {
    const view = integrationViewFromDoctor({
      integration: {
        components: [
          { id: "nautilus_extension", status: "installed", message: "Files" },
          { id: "desktop_context_menu", status: "conflict", message: "DING changed" },
          { id: "application_launcher", status: "installed", message: "Launcher" },
        ],
      },
    });
    expect(view.rows.map((row) => row.status)).toEqual(["installed", "conflict", "installed"]);
    expect(view.guidance).toContain("apt");
  });

  it("marks absent rows unknown and rejects malformed reports", () => {
    const view = integrationViewFromDoctor({ integration: { components: [] } });
    expect(view.rows.every((row) => row.status === "unknown")).toBe(true);
    expect(view.guidance).not.toBeNull();
    expect(() => integrationViewFromDoctor({ integration: null })).toThrow("không có danh sách");
  });
  it("repair guidance works for curl installs, not only apt", () => {
    const view = integrationViewFromDoctor({ integration: { components: [] } });
    expect(view.guidance).toContain("morphix install-integration");
  });
});
