// Pure composition of the khay "Gần đây" list: the running job first, then finished history, capped at one shared limit.
import { progressPercent } from "./format";

export interface HistoryEntry {
  readonly path: string;
  readonly title: string;
  readonly detail: string;
  readonly status: "done" | "failed" | "cancelled";
  readonly finished_at_unix: number;
}

export type JobState =
  | { readonly kind: "running" }
  | { readonly kind: "finished"; readonly ok: number; readonly failed: number }
  | { readonly kind: "cancelled" };

export interface JobSnapshot {
  readonly id: number;
  readonly label: string;
  readonly done: number;
  readonly total: number;
  readonly state: JobState;
}

export type RecentRow =
  | { readonly kind: "running"; readonly label: string; readonly percent: number }
  | {
      readonly kind: "history";
      readonly title: string;
      readonly detail: string;
      readonly failed: boolean;
      readonly outputPath: string | null;
      readonly finishedAtUnix: number;
    };

// WHY one limit: the running row takes a slot of the same four, so the khay never grows past its height.
export function composeRecentRows(
  active: JobSnapshot | null,
  history: readonly HistoryEntry[],
  limit: number,
): RecentRow[] {
  const rows: RecentRow[] = [];
  if (active !== null) {
    rows.push({
      kind: "running",
      label: active.label,
      percent: progressPercent(active.done, active.total),
    });
  }
  const remaining: number = Math.max(0, limit - rows.length);
  for (const entry of history.slice(0, remaining)) {
    rows.push({
      kind: "history",
      title: entry.title,
      detail: entry.detail,
      failed: entry.status === "failed",
      outputPath: entry.status === "done" && entry.path.trim() !== "" ? entry.path : null,
      finishedAtUnix: entry.finished_at_unix,
    });
  }
  return rows;
}
