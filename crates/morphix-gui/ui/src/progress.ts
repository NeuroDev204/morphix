// Progress popup: a small always-on-top window that follows jobs started from the context menu.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { truncateName } from "./format";
import { isStale, progressView, type ProgressView } from "./progress-view";
import type { HistoryEntry, JobSnapshot } from "./recent";

const JOB_UPDATE_EVENT: string = "job-update";
const JOB_FINISHED_EVENT: string = "job-finished";
const PROGRESS_SHOWN_EVENT: string = "progress-shown";
const NAME_MAX_CHARS: number = 40;
const POLL_MS: number = 500;

let current: { snapshot: JobSnapshot; view: ProgressView; latest: HistoryEntry | null } | null = null;
let closeTimer: number | undefined;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text?: string): HTMLElementTagNameMap[K] {
  const node: HTMLElementTagNameMap[K] = document.createElement(tag);
  node.className = className;
  if (text !== undefined) {
    node.textContent = text;
  }
  return node;
}

function button(label: string, onClick: () => void): HTMLButtonElement {
  const node: HTMLButtonElement = el("button", "rounded-md px-3 py-1 text-xs font-medium cursor-pointer", label);
  node.style.cssText = "background: var(--secondary); color: var(--foreground)";
  node.addEventListener("click", onClick);
  return node;
}

function hide(): void {
  clearTimeout(closeTimer);
  void invoke("hide_progress");
}

function scheduleClose(): void {
  clearTimeout(closeTimer);
  // WHY: nếu con trỏ đang nằm trên popup (không có mouseenter mới khi nội dung dựng lại) thì để nguyên, mouseleave sẽ đặt lại giờ đóng.
  if (document.querySelector(":hover") !== null && document.querySelector("#app :hover") !== null) {
    return;
  }
  if (current?.view.closeAfterMs != null) {
    closeTimer = window.setTimeout(hide, current.view.closeAfterMs);
  }
}

function render(): void {
  const app: HTMLElement = document.getElementById("app") as HTMLElement;
  app.replaceChildren();
  if (current === null) {
    return;
  }
  const { snapshot, view, latest } = current;
  const card: HTMLDivElement = el("div", "flex flex-col gap-1.5 p-3 h-screen box-border");
  card.style.cssText = "background: var(--surface); color: var(--foreground); font-family: var(--app-font)";
  card.setAttribute("data-tauri-drag-region", "");

  const header: HTMLDivElement = el("div", "flex items-center justify-between gap-2");
  header.append(el("div", "text-sm font-semibold truncate", view.title));
  const close: HTMLButtonElement = el("button", "text-xs cursor-pointer", "✕");
  close.title = "Đóng";
  close.addEventListener("click", hide);
  header.append(close);

  const file: HTMLDivElement = el("div", "text-xs truncate", truncateName(view.fileLine, NAME_MAX_CHARS));
  file.style.color = "var(--muted)";

  const track: HTMLDivElement = el("div", "h-2 rounded-full overflow-hidden relative");
  track.style.background = "var(--secondary)";
  const bar: HTMLDivElement = el("div", "h-full rounded-full");
  bar.style.background = view.status === "failed" ? "var(--danger)" : "var(--primary)";
  if (view.percent === null && view.status === "running") {
    bar.style.cssText += "; width: 35%; position: absolute; animation: progress-slide 1.2s ease-in-out infinite alternate";
  } else {
    bar.style.width = `${view.status === "done" ? 100 : (view.percent ?? 0)}%`;
  }
  track.append(bar);

  const footer: HTMLDivElement = el("div", "flex items-center justify-between gap-2");
  const detail: HTMLDivElement = el("div", "text-xs truncate", view.detail);
  if (view.status === "failed") {
    detail.style.color = "var(--danger)";
  }
  footer.append(detail);
  const actions: HTMLDivElement = el("div", "flex gap-2");
  if (view.showCancel) {
    actions.append(button("Huỷ", () => void invoke("cancel_job", { id: snapshot.id })));
  }
  if (view.showOpenFolder && latest !== null) {
    actions.append(button("Mở thư mục", () => void invoke("open_output_directory", { path: latest.path })));
  }
  footer.append(actions);

  card.append(header, file, track, footer);
  card.addEventListener("mouseenter", () => clearTimeout(closeTimer));
  card.addEventListener("mouseleave", scheduleClose);
  app.append(card);
}

async function onSnapshot(snapshot: JobSnapshot, finished: boolean): Promise<void> {
  // WHY bỏ qua việc không từ menu: việc chạy từ Khay đã có tiến trình ngay trong Khay.
  if (!snapshot.from_menu || isStale(current, snapshot)) {
    return;
  }
  let latest: HistoryEntry | null = null;
  if (finished) {
    latest = await lastHistoryEntry();
  }
  show(snapshot, latest);
}

async function lastHistoryEntry(): Promise<HistoryEntry | null> {
  try {
    return (await invoke<HistoryEntry[]>("recent_history", { limit: 1 }))[0] ?? null;
  } catch (error: unknown) {
    console.error("không đọc được lịch sử", error);
    return null;
  }
}

function show(snapshot: JobSnapshot, latest: HistoryEntry | null): void {
  clearTimeout(closeTimer);
  current = { snapshot, view: progressView(snapshot, latest), latest };
  render();
  scheduleClose();
}

// WHY hỏi lại thay vì chỉ nghe sự kiện: sự kiện chỉ tới khi một file xong (không có % trong file) và có thể trễ hoặc mất;
// `active_job` luôn đúng. Việc biến mất khỏi `active_job` nghĩa là đã xong: lấy kết quả từ lịch sử.
async function poll(): Promise<void> {
  const job: JobSnapshot | null = await invoke<JobSnapshot | null>("active_job");
  if (job !== null) {
    if (job.from_menu) {
      show(job, null);
    }
    return;
  }
  if (current === null || current.view.status !== "running") {
    return;
  }
  const latest: HistoryEntry | null = await lastHistoryEntry();
  const failed: boolean = latest?.status === "failed";
  const cancelled: boolean = latest?.status === "cancelled";
  const done: JobSnapshot = {
    ...current.snapshot,
    done: current.snapshot.total,
    fraction: 0,
    state: cancelled ? { kind: "cancelled" } : { kind: "finished", ok: failed ? 0 : current.snapshot.total, failed: failed ? 1 : 0 },
  };
  show(done, latest);
}

async function start(): Promise<void> {
  await listen<JobSnapshot>(JOB_UPDATE_EVENT, (event) => void onSnapshot(event.payload, false));
  await listen<JobSnapshot>(JOB_FINISHED_EVENT, (event) => void onSnapshot(event.payload, true));
  // WHY kéo ảnh chụp khi cửa sổ vừa hiện: lõi chỉ báo khi một mục xong, nên việc một file lớn chưa có sự kiện nào.
  await listen(PROGRESS_SHOWN_EVENT, () => void pullActiveJob());
  await pullActiveJob();
  window.setInterval(() => void poll().catch((error: unknown) => console.error("không hỏi được tiến trình", error)), POLL_MS);
}

async function pullActiveJob(): Promise<void> {
  const job: JobSnapshot | null = await invoke<JobSnapshot | null>("active_job");
  if (job !== null) {
    await onSnapshot(job, false);
  }
}

start().catch((error: unknown) => console.error("không khởi động được popup tiến trình", error));
