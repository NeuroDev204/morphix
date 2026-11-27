import { describe, expect, it } from "vitest";
import { isStale, progressView } from "../src/progress-view";
import type { HistoryEntry, JobSnapshot } from "../src/recent";

const snap = (over: Partial<JobSnapshot> = {}): JobSnapshot => ({
  id: 1,
  label: "Đổi sang MP4",
  done: 2,
  total: 5,
  state: { kind: "running" },
  from_menu: true,
  first_name: "du-lich.mov",
  fraction: 0,
  ...over,
});
const entry = (over: Partial<HistoryEntry> = {}): HistoryEntry => ({
  path: "/a/du-lich.mp4",
  title: "Đổi sang MP4",
  detail: "Hoàn tất 5 file",
  status: "done",
  finished_at_unix: 1,
  ...over,
});

describe("progressView", () => {
  it("running multi file shows count and percent", () => {
    expect(progressView(snap(), null)).toMatchObject({
      title: "Morphix · Đổi sang MP4",
      fileLine: "du-lich.mov (2/5 file)",
      percent: 40,
      status: "running",
      showCancel: true,
      closeAfterMs: null,
      showOpenFolder: false,
    });
  });

  it("a single big file shows the percent ffmpeg reports", () => {
    const view = progressView(snap({ done: 0, total: 1, fraction: 0.42 }), null);
    expect(view.percent).toBe(42);
    expect(view.detail).toBe("42%");
  });

  it("percent counts finished files plus the fraction of the current one", () => {
    expect(progressView(snap({ done: 1, total: 4, fraction: 0.5 }), null).percent).toBe(38);
  });

  it("single file is indeterminate", () => {
    const view = progressView(snap({ done: 0, total: 1 }), null);
    expect(view.percent).toBeNull();
    expect(view.fileLine).toBe("du-lich.mov");
    expect(view.detail).toBe("Đang xử lý…");
  });

  it("success closes after 4s with open folder", () => {
    const view = progressView(snap({ done: 5, state: { kind: "finished", ok: 5, failed: 0 } }), entry());
    expect(view).toMatchObject({ status: "done", detail: "Xong", closeAfterMs: 4000, showOpenFolder: true, showCancel: false });
  });

  it("failure stays with the reason", () => {
    const view = progressView(
      snap({ done: 5, state: { kind: "finished", ok: 4, failed: 1 } }),
      entry({ status: "failed", detail: "Lỗi: file hỏng" }),
    );
    expect(view).toMatchObject({ status: "failed", detail: "Lỗi: file hỏng", closeAfterMs: null, showOpenFolder: false });
  });

  it("cancel closes after 2s", () => {
    const view = progressView(snap({ state: { kind: "cancelled" } }), null);
    expect(view).toMatchObject({ status: "cancelled", detail: "Đã huỷ", closeAfterMs: 2000, showCancel: false });
  });
});

describe("isStale", () => {
  it("a running snapshot never overrides a finished one of the same job", () => {
    const finished = progressView(snap({ state: { kind: "finished", ok: 5, failed: 0 } }), entry());
    expect(isStale({ snapshot: snap({ state: { kind: "finished", ok: 5, failed: 0 } }), view: finished }, snap())).toBe(true);
    expect(isStale({ snapshot: snap({ id: 1, state: { kind: "finished", ok: 5, failed: 0 } }), view: finished }, snap({ id: 2 }))).toBe(false);
    expect(isStale(null, snap())).toBe(false);
  });
});
