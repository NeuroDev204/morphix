// Khay window: a drop zone and file picker that feed the batch flow, plus the last four jobs. Runs in its own webview.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { formatRelativeTime, truncateName } from "./format";
import { composeRecentRows, type HistoryEntry, type JobSnapshot, type RecentRow } from "./recent";
import { integrationViewFromDoctor, type IntegrationView } from "./integration";
import { composeSelectionView, type KhaySelection, type SelectionView } from "./selection";
import type { WheelPayload } from "./menu-types";
import { S } from "./strings";

// WHY 4: the khay is a fixed-height strip; the drop zone is the main action and history only offers a way back.
const RECENT_LIMIT: number = 4;
const SKELETON_ROWS: number = 3;
const NAME_MAX_CHARS: number = 36;
const JOB_UPDATE_EVENT: string = "job-update";
const JOB_FINISHED_EVENT: string = "job-finished";

type ListState =
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly rows: RecentRow[] }
  | { readonly kind: "error"; readonly reason: string };

type IntegrationState =
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly view: IntegrationView }
  | { readonly kind: "error"; readonly reason: string };

let selection: KhaySelection = { paths: [], error: null, revision: -1 };
let selectionView: SelectionView = { kind: "empty" };
let selectionToken: number = 0;
let startingAction: boolean = false;
let activeJob: JobSnapshot | null = null;
// WHY: đếm sự kiện việc để `refresh` biết ảnh chụp `active_job` nó đọc đã cũ hơn sự kiện vừa tới.
let jobEventCount: number = 0;
let history: HistoryEntry[] = [];
let listState: ListState = { kind: "loading" };
let integrationState: IntegrationState = { kind: "loading" };
let bannerMessage: string | null = null;
// WHY token: a slow refresh must not overwrite the list of a newer one that finished first.
let refreshToken: number = 0;

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string = "",
  text: string = "",
): HTMLElementTagNameMap[K] {
  const node: HTMLElementTagNameMap[K] = document.createElement(tag);
  if (className !== "") {
    node.className = className;
  }
  // WHY textContent: file names are user data and must never be parsed as HTML.
  node.textContent = text;
  return node;
}

function showBanner(message: string | null): void {
  bannerMessage = message;
  render();
}

function rowNode(row: RecentRow, nowUnix: number): HTMLLIElement {
  const item: HTMLLIElement = el("li", "row row-stack");
  if (row.kind === "running") {
    item.append(
      el("p", "row-title", truncateName(row.label, NAME_MAX_CHARS)),
      el("p", "row-detail", S.runningWithPercent(row.percent)),
    );
    const track: HTMLDivElement = el("div", "bar-track");
    const fill: HTMLDivElement = el("div", "bar-fill");
    fill.style.width = `${row.percent}%`;
    track.append(fill);
    item.append(track);
    return item;
  }
  const title: HTMLParagraphElement = el("p", "row-title", truncateName(row.title, NAME_MAX_CHARS));
  title.title = row.title;
  const detail: HTMLParagraphElement = el(
    "p",
    row.failed ? "row-detail is-failed" : "row-detail",
    row.detail,
  );
  const time: HTMLSpanElement = el(
    "span",
    "row-time",
    formatRelativeTime(nowUnix, row.finishedAtUnix),
  );
  item.append(title, detail, time);
  const outputPath: string | null = row.outputPath;
  if (outputPath !== null) {
    const openDirectory: HTMLButtonElement = el("button", "btn btn-ghost row-open-directory", S.openOutputDirectory);
    openDirectory.type = "button";
    openDirectory.addEventListener("click", (): void => { void openOutputDirectory(outputPath); });
    item.append(openDirectory);
  }
  return item;
}

function integrationNode(): HTMLElement {
  const panel: HTMLElement = el("section", "integration-panel");
  panel.append(el("p", "integration-title", S.integrationTitle));
  if (integrationState.kind === "loading") {
    panel.append(el("p", "integration-note", S.integrationLoading));
  } else if (integrationState.kind === "error") {
    panel.append(el("p", "integration-note is-failed", `${S.integrationError} ${integrationState.reason}`));
  } else {
    const rows: HTMLUListElement = el("ul", "integration-list");
    for (const row of integrationState.view.rows) {
      const item: HTMLLIElement = el("li", "integration-row");
      item.append(el("span", "integration-label", row.label));
      const status: HTMLSpanElement = el("span", "integration-status", row.statusLabel);
      status.dataset.status = row.status;
      item.append(status, el("p", "integration-message", row.message));
      rows.append(item);
    }
    panel.append(rows);
    if (integrationState.view.guidance !== null) {
      panel.append(el("p", "integration-guidance", integrationState.view.guidance));
    }
  }
  return panel;
}

function skeletonNode(): HTMLLIElement {
  const item: HTMLLIElement = el("li", "row row-stack");
  item.append(el("div", "skeleton-line skeleton-wide"), el("div", "skeleton-line skeleton-narrow"));
  return item;
}

function renderList(list: HTMLUListElement, clearButton: HTMLButtonElement): void {
  list.replaceChildren();
  clearButton.hidden = listState.kind !== "ready" || listState.rows.length === 0;
  if (listState.kind === "loading") {
    list.setAttribute("aria-busy", "true");
    for (let index = 0; index < SKELETON_ROWS; index += 1) {
      list.append(skeletonNode());
    }
    return;
  }
  list.removeAttribute("aria-busy");
  if (listState.kind === "error") {
    list.append(el("p", "empty-state", listState.reason));
    return;
  }
  if (listState.rows.length === 0) {
    list.append(el("p", "empty-state", S.emptyHistory));
    return;
  }
  const nowUnix: number = Math.floor(Date.now() / 1000);
  for (const row of listState.rows) {
    list.append(rowNode(row, nowUnix));
  }
}

// WHY full rebuild: the khay is four rows, so re-rendering is cheaper than tracking which node changed.
function render(): void {
  const app: HTMLElement | null = document.getElementById("app");
  if (app === null) {
    throw new Error("thiếu phần tử #app trong khay.html");
  }
  app.replaceChildren();
  app.className = "khay-shell";

  const header: HTMLDivElement = el("div", "khay-header");
  const brand: HTMLParagraphElement = el("p", "khay-brand", S.appName);
  const settingsButton: HTMLButtonElement = el("button", "btn btn-ghost", S.traySettings);
  settingsButton.type = "button";
  settingsButton.addEventListener("click", () => {
    void openSettings();
  });
  header.append(brand, settingsButton);

  const banner: HTMLDivElement = el("div", "khay-banner");
  const hasFailedRow: boolean =
    listState.kind === "ready" && listState.rows.some((row) => row.kind === "history" && row.failed);
  if (selectionView.kind === "error") {
    banner.append(el("p", "banner-error", selectionView.message));
  } else if (bannerMessage !== null) {
    banner.append(el("p", "banner-error", bannerMessage));
  } else if (hasFailedRow) {
    banner.append(el("p", "banner-error", S.errorBanner));
  }

  const dropZone: HTMLDivElement = el("div", "drop-zone khay-drop");
  dropZone.id = "drop-zone";
  const dropTitle: HTMLParagraphElement = el("p", "drop-title", S.dropTitle);
  const dropHint: HTMLParagraphElement = el("p", "drop-hint", S.dropHint);
  const chooseButton: HTMLButtonElement = el("button", "btn btn-outline", S.chooseFile);
  chooseButton.type = "button";
  chooseButton.addEventListener("click", () => {
    void chooseFiles();
  });
  dropZone.append(dropTitle, dropHint, chooseButton);

  if (selectionView.kind === "files") {
    dropZone.className = "khay-selection";
    dropZone.replaceChildren(el("p", "drop-title", selectionView.summary));
    for (const name of selectionView.names) {
      const line: HTMLParagraphElement = el("p", "selection-name", name);
      line.title = name;
      dropZone.append(line);
    }
    if (selectionView.extra > 0) dropZone.append(el("p", "drop-hint", S.moreFiles(selectionView.extra)));
    const discard: HTMLButtonElement = el("button", "btn btn-ghost", S.discardSelection);
    discard.type = "button";
    discard.addEventListener("click", (): void => { void discardSelection(); });
    dropZone.append(discard);
    if (selectionView.unsupported !== null) dropZone.append(el("p", "empty-state", selectionView.unsupported));
    const actions: HTMLDivElement = el("div", "selection-actions");
    for (const item of selectionView.items) {
      const button: HTMLButtonElement = el("button", item.is_tool ? "btn btn-outline" : "btn", item.label);
      button.type = "button";
      button.disabled = startingAction || activeJob !== null;
      button.addEventListener("click", (): void => { void runSelectionAction(item.id); });
      actions.append(button);
    }
    dropZone.append(actions);
  }

  const recent: HTMLDivElement = el("div", "khay-recent");
  const recentHead: HTMLDivElement = el("div", "khay-recent-head");
  const recentTitle: HTMLParagraphElement = el("p", "khay-recent-title", S.recentTitle);
  const clearButton: HTMLButtonElement = el("button", "btn btn-ghost", S.clearHistory);
  clearButton.type = "button";
  clearButton.addEventListener("click", () => {
    void clearHistory();
  });
  recentHead.append(recentTitle, clearButton);
  const list: HTMLUListElement = el("ul", "khay-list");
  renderList(list, clearButton);
  recent.append(recentHead, list);

  const footer: HTMLDivElement = el("div", "khay-footer");
  const privacy: HTMLParagraphElement = el("p", "khay-privacy", S.footerPrivacy);
  const quitButton: HTMLButtonElement = el("button", "btn btn-ghost", S.quit);
  quitButton.type = "button";
  quitButton.addEventListener("click", () => {
    void quitApp();
  });
  footer.append(privacy, quitButton);

  app.append(header, banner, integrationNode(), dropZone, recent, footer);
}

async function submitPaths(paths: string[]): Promise<void> {
  if (paths.length === 0) {
    return;
  }
  try {
    await invoke<void>("submit_menu_request", { paths });
    showBanner(null);
  } catch (error: unknown) {
    console.error("không gửi được file từ Khay", error);
    showBanner(describeError(error));
  }
}

async function chooseFiles(): Promise<void> {
  try {
    const picked: string[] | null = await open({ multiple: true, directory: false });
    await submitPaths(picked ?? []);
  } catch (error: unknown) {
    console.error("không mở được hộp thoại chọn file", error);
    showBanner(describeError(error));
  }
}

async function clearHistory(): Promise<void> {
  try {
    await invoke<void>("clear_history");
  } catch (error: unknown) {
    console.error("không xoá được lịch sử", error);
    showBanner(describeError(error));
    return;
  }
  await refresh();
}

async function openSettings(): Promise<void> {
  try {
    await invoke<void>("open_settings");
  } catch (error: unknown) {
    console.error("không mở được cửa sổ Cài đặt", error);
    showBanner(describeError(error));
  }
}

async function openOutputDirectory(path: string): Promise<void> {
  try {
    await invoke<void>("open_output_directory", { path });
    showBanner(null);
  } catch (error: unknown) {
    console.error("không mở được thư mục đầu ra", error);
    showBanner(describeError(error));
  }
}

async function quitApp(): Promise<void> {
  try {
    await invoke<void>("quit");
  } catch (error: unknown) {
    console.error("không thoát được Morphix", error);
    showBanner(describeError(error));
  }
}

// WHY đọc active_job: việc đã chạy trước khi Khay mở vẫn phải hiện dù Khay lỡ mất sự kiện của nó.
async function refresh(): Promise<void> {
  const token: number = ++refreshToken;
  const eventsBefore: number = jobEventCount;
  listState = { kind: "loading" };
  render();
  try {
    const [job, entries] = await Promise.all([
      invoke<JobSnapshot | null>("active_job"),
      invoke<HistoryEntry[]>("recent_history", { limit: RECENT_LIMIT }),
    ]);
    if (token !== refreshToken) {
      return;
    }
    // WHY: sự kiện tới trong lúc chờ mới hơn ảnh chụp vừa đọc; ghi đè sẽ kéo thanh tiến độ lùi lại.
    if (jobEventCount === eventsBefore) {
      activeJob = job;
    }
    history = entries;
    listState = { kind: "ready", rows: composeRecentRows(activeJob, history, RECENT_LIMIT) };
  } catch (error: unknown) {
    if (token !== refreshToken) {
      return;
    }
    console.error("không tải được lịch sử Khay", error);
    listState = { kind: "error", reason: describeError(error) };
  }
  render();
}

async function refreshIntegration(): Promise<void> {
  integrationState = { kind: "loading" };
  render();
  try {
    const report: unknown = await invoke<unknown>("integration_status");
    integrationState = { kind: "ready", view: integrationViewFromDoctor(report) };
  } catch (error: unknown) {
    console.error("không đọc được trạng thái tích hợp Morphix", error);
    integrationState = { kind: "error", reason: describeError(error) };
  }
  render();
}

function applyJobUpdate(snapshot: JobSnapshot): void {
  jobEventCount += 1;
  activeJob = snapshot;
  if (listState.kind === "ready") {
    listState = { kind: "ready", rows: composeRecentRows(activeJob, history, RECENT_LIMIT) };
    render();
  }
}

function wireDropZone(): void {
  void getCurrentWebview().onDragDropEvent((event) => {
    const zone: HTMLElement | null = document.getElementById("drop-zone");
    if (event.payload.type === "drop") {
      zone?.classList.remove("is-over");
      void submitPaths(event.payload.paths);
      return;
    }
    zone?.classList.toggle("is-over", event.payload.type === "enter" || event.payload.type === "over");
  });
}

async function start(): Promise<void> {
  // WHY đăng ký trước: việc xong trong lúc tải lần đầu vẫn phải vào danh sách.
  await listen<JobSnapshot>(JOB_UPDATE_EVENT, (event) => applyJobUpdate(event.payload));
  await listen<JobSnapshot>(JOB_FINISHED_EVENT, () => {
    jobEventCount += 1;
    activeJob = null;
    void refresh();
  });
  await listen<KhaySelection>("khay-selection", (event): void => { void applySelection(event.payload); });
  await applySelection(await invoke<KhaySelection>("khay_selection"));
  void refreshIntegration();
  wireDropZone();
  await refresh();
}

start().catch((error: unknown) => {
  console.error("không khởi động được Khay", error);
  showBanner(describeError(error));
});


async function applySelection(next: KhaySelection): Promise<void> {
  if (next.revision <= selection.revision) return;
  selection = next;
  const token: number = ++selectionToken;
  selectionView = composeSelectionView(next, null, null);
  render();
  if (next.paths.length === 0 || next.error !== null) return;
  try {
    const payload: WheelPayload = await invoke<WheelPayload>("menu_payload", { paths: next.paths });
    if (token !== selectionToken) return;
    selectionView = composeSelectionView(next, payload, null);
  } catch (error: unknown) {
    if (token !== selectionToken) return;
    console.error("không tải được thao tác cho vùng chọn", error);
    selectionView = composeSelectionView(next, null, describeError(error));
  }
  render();
}

async function discardSelection(): Promise<void> {
  try {
    await invoke<void>("clear_khay_selection");
    showBanner(null);
  } catch (error: unknown) {
    console.error("không bỏ được vùng chọn", error);
    showBanner(describeError(error));
  }
}

async function runSelectionAction(id: string): Promise<void> {
  startingAction = true;
  render();
  try {
    await invoke<number>("run_action", { id, paths: selection.paths });
    showBanner(null);
    await refresh();
  } catch (error: unknown) {
    console.error("không chạy được thao tác vùng chọn", error);
    showBanner(describeError(error));
  } finally {
    startingAction = false;
    render();
  }
}
