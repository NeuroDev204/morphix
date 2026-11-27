// Pure view model of the progress popup: what to show for a job snapshot, and when the popup closes by itself.
import { progressPercent } from "./format";
import type { HistoryEntry, JobSnapshot } from "./recent";

export interface ProgressView {
  readonly title: string;
  readonly fileLine: string;
  // WHY null: the core reports progress per item, so a single file (one video) has no honest percentage.
  readonly percent: number | null;
  readonly status: "running" | "done" | "failed" | "cancelled";
  readonly detail: string;
  readonly closeAfterMs: number | null;
  readonly showOpenFolder: boolean;
  readonly showCancel: boolean;
}

const CLOSE_AFTER_DONE_MS: number = 4000;
const CLOSE_AFTER_CANCEL_MS: number = 2000;

export function progressView(snapshot: JobSnapshot, latest: HistoryEntry | null): ProgressView {
  const count: string = snapshot.total > 1 ? ` (${snapshot.done}/${snapshot.total} file)` : "";
  const percent: number | null = snapshot.total <= 1 ? null : progressPercent(snapshot.done, snapshot.total);
  const base = {
    title: `Morphix · ${snapshot.label}`,
    fileLine: `${snapshot.first_name}${count}`,
    percent,
  };
  switch (snapshot.state.kind) {
    case "running":
      return {
        ...base,
        status: "running",
        detail: percent === null ? "Đang xử lý…" : `${percent}%`,
        closeAfterMs: null,
        showOpenFolder: false,
        showCancel: true,
      };
    case "cancelled":
      return { ...base, status: "cancelled", detail: "Đã huỷ", closeAfterMs: CLOSE_AFTER_CANCEL_MS, showOpenFolder: false, showCancel: false };
    case "finished":
      if (snapshot.state.failed > 0) {
        return {
          ...base,
          status: "failed",
          detail: latest?.detail ?? `${snapshot.state.failed} file lỗi`,
          closeAfterMs: null,
          showOpenFolder: false,
          showCancel: false,
        };
      }
      return {
        ...base,
        status: "done",
        detail: "Xong",
        closeAfterMs: CLOSE_AFTER_DONE_MS,
        showOpenFolder: latest?.status === "done" && latest.path !== "",
        showCancel: false,
      };
  }
}
