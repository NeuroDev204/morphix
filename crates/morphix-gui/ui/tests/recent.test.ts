import { describe, expect, it } from "vitest";
import { composeRecentRows, type HistoryEntry, type JobSnapshot } from "../src/recent";

const finishedEntry = (n: number): HistoryEntry => ({
  path: `/a/file-${n}.jpg`,
  title: `việc ${n}`,
  detail: "Hoàn tất 1 file",
  status: "done",
  finished_at_unix: n,
});

const runningJob: JobSnapshot = {
  id: 7,
  label: "Đổi sang JPG",
  done: 1,
  total: 4,
  state: { kind: "running" },
};

describe("composeRecentRows", () => {
  it("shows at most four rows and puts the running job first", () => {
    const history: HistoryEntry[] = [0, 1, 2, 3, 4].map(finishedEntry);
    const rows = composeRecentRows(runningJob, history, 4);
    expect(rows).toHaveLength(4);
    expect(rows[0]).toEqual({ kind: "running", label: "Đổi sang JPG", percent: 25 });
    expect(rows[1]).toMatchObject({ kind: "history", title: "việc 0" });
  });

  it("fills all four slots from history when nothing is running", () => {
    const history: HistoryEntry[] = [0, 1, 2, 3, 4].map(finishedEntry);
    expect(composeRecentRows(null, history, 4)).toHaveLength(4);
  });

  it("marks failed entries so the list can show them", () => {
    const failed: HistoryEntry = { ...finishedEntry(1), status: "failed" };
    expect(composeRecentRows(null, [failed], 4)[0]).toMatchObject({
      kind: "history",
      failed: true,
      outputPath: null,
    });
  });

  it("keeps output paths only for completed jobs", () => {
    const done: HistoryEntry = finishedEntry(1);
    const cancelled: HistoryEntry = { ...finishedEntry(2), status: "cancelled" };
    expect(composeRecentRows(null, [done, cancelled], 4).map((row) =>
      row.kind === "history" ? row.outputPath : null,
    )).toEqual([done.path, null]);
  });

  it("returns an empty list when there is no job and no history", () => {
    expect(composeRecentRows(null, [], 4)).toEqual([]);
  });
});
