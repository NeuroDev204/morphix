// Quick menu window: a circular wheel of actions for the current file batch, driven by the commands and events in commands.rs and windows.rs.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { shouldHideOnBlur } from "./focus";
import { hasMixedExtensions, progressPercent, summaryLine, truncateName } from "./format";
import { actionForKey, isNavigationKey, nextIndex, type NavigationKey } from "./keys";
import { S } from "./strings";
import { WHEEL, labelPosition, sectorPath, wrapLabel } from "./wheel";

import type { WheelItem, WheelPayload, FileSummary } from "./menu-types";

interface MenuRequestState {
  readonly paths: string[];
  readonly collecting: boolean;
  readonly revision: number;
}

type JobOutcome =
  | { kind: "running" }
  | { kind: "finished"; ok: number; failed: number }
  | { kind: "cancelled" };

interface JobSnapshot {
  readonly id: number;
  readonly label: string;
  readonly done: number;
  readonly total: number;
  readonly state: JobOutcome;
}

type Mode = "idle" | "collecting" | "ready" | "running" | "error";

const TOOLS_TAB: string = "cong-cu";
const BASIC_TAB: string = "chuyen";
const SVG_NS = "http://www.w3.org/2000/svg";
// WHY: 600 ms lets the user see the ring reach 100% before the window disappears.
const CLOSE_DELAY_MS: number = 600;
// WHY: X11 can report focus loss before it dispatches the click that starts a conversion.
const BLUR_CLICK_DELAY_MS: number = 180;
const LABEL_MAX_CHARS: number = 10;
const NAME_PREVIEW_COUNT: number = 2;
const NAME_MAX_CHARS: number = 14;
const RING_SIZE: number = 96;
const RING_RADIUS: number = 42;
const RING_CIRCUMFERENCE: number = 2 * Math.PI * RING_RADIUS;
const RING_STROKE: string = "6";

const appWindow = getCurrentWindow();

let mode: Mode = "idle";
let tab: string = BASIC_TAB;
let request: MenuRequestState | null = null;
let latestRevision: number = 0;
let payload: WheelPayload | null = null;
let job: { id: number | null; done: number; total: number } = { id: null, done: 0, total: 0 };
let errorReason: string = "";
let banner: string | null = null;
let closeTimer: ReturnType<typeof setTimeout> | undefined;
let altHeld: boolean = false;
// WHY: Rust hiện cửa sổ ngay trước khi gửi lô, nên lúc nhận lô là mốc "vừa hiện" để bỏ qua blur sớm.
let shownAtMs: number | null = null;
// Parallel arrays: the sector at index i is the item at index i of the visible list.
let visibleItems: WheelItem[] = [];
let sectorNodes: SVGPathElement[] = [];

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string = "",
  text: string = "",
): HTMLElementTagNameMap[K] {
  const node: HTMLElementTagNameMap[K] = document.createElement(tag);
  if (className !== "") {
    node.className = className;
  }
  if (text !== "") {
    node.textContent = text;
  }
  return node;
}

function svgElement<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attributes: Record<string, string>,
): SVGElementTagNameMap[K] {
  const node: SVGElementTagNameMap[K] = document.createElementNS(SVG_NS, tag);
  for (const [name, value] of Object.entries(attributes)) {
    node.setAttribute(name, value);
  }
  return node;
}

// WHY: Tauri rejects with the Rust error string; anything else still gets a visible reason.
function reasonOf(error: unknown): string {
  if (typeof error === "string" && error.trim() !== "") {
    return error;
  }
  if (error instanceof Error && error.message !== "") {
    return error.message;
  }
  return S.unknownError;
}

function visibleFor(items: readonly WheelItem[], currentTab: string): WheelItem[] {
  const wantTools: boolean = currentTab === TOOLS_TAB;
  return items.filter((item) => item.is_tool === wantTools);
}

// WHY: a name list in the hole stays short; the full names live in the file list, not here.
function shortNames(files: readonly FileSummary[]): string {
  const shown: string[] = files
    .slice(0, NAME_PREVIEW_COUNT)
    .map((file) => truncateName(file.name, NAME_MAX_CHARS));
  const hidden: number = files.length - shown.length;
  return hidden > 0 ? `${shown.join(", ")} +${hidden}` : shown.join(", ");
}

const app: HTMLElement = document.getElementById("app") ?? failMissing("app");
const wheel: SVGSVGElement = svgElement("svg", {
  viewBox: `0 0 ${WHEEL.size} ${WHEEL.size}`,
  class: "wheel-svg",
});
const center: HTMLDivElement = element("div", "wheel-center");
const wheelBox: HTMLDivElement = element("div", "wheel");
const bannerBox: HTMLDivElement = element("div", "banner-error");
const tabBar: HTMLDivElement = element("div", "segmented");
const hint: HTMLParagraphElement = element("p", "menu-hint", S.escHint);

function failMissing(id: string): never {
  throw new Error(`missing #${id} element in index.html`);
}

function buildSkeleton(): void {
  wheelBox.append(wheel, center);
  tabBar.setAttribute("role", "tablist");
  for (const option of S.menuTabs) {
    const button: HTMLButtonElement = element("button", "", option.label);
    button.setAttribute("role", "tab");
    button.dataset.value = option.value;
    button.addEventListener("click", () => setTab(option.value));
    tabBar.append(button);
  }
  app.className = "menu-shell";
  app.replaceChildren(wheelBox, bannerBox, tabBar, hint);
}

function renderBanner(): void {
  bannerBox.textContent = banner ?? "";
  bannerBox.hidden = banner === null;
}

function renderTabs(): void {
  for (const button of tabBar.querySelectorAll("button")) {
    button.setAttribute("aria-selected", String(button.dataset.value === tab));
  }
}

function drawSectors(items: WheelItem[]): void {
  visibleItems = items;
  sectorNodes = [];
  const count: number = items.length;
  items.forEach((item, index) => {
    const sector: SVGPathElement = svgElement("path", {
      d: sectorPath(index, count),
      class: item.is_suggested ? "wheel-sector is-suggested" : "wheel-sector",
      role: "menuitem",
      tabindex: "0",
      "aria-label": item.label,
    });
    sector.addEventListener("click", () => void runItem(item));
    sectorNodes.push(sector);
    wheel.append(sector);

    const anchor = labelPosition(index, count);
    const lines: string[] = wrapLabel(item.label, LABEL_MAX_CHARS);
    const text: SVGTextElement = svgElement("text", {
      class: item.is_suggested ? "wheel-label is-suggested" : "wheel-label",
      "text-anchor": "middle",
    });
    lines.forEach((line, lineIndex) => {
      // WHY: the first line is lifted by (lines - 1) half-steps so the block stays centred on the anchor.
      const dy: string = lineIndex === 0 ? `${-(lines.length - 1) * 0.55}em` : "1.1em";
      const span: SVGTSpanElement = svgElement("tspan", {
        x: String(anchor.x),
        dy,
      });
      span.textContent = line;
      text.append(span);
    });
    text.setAttribute("x", String(anchor.x));
    text.setAttribute("y", String(anchor.y));
    wheel.append(text);
  });
}

function summaryBlock(current: WheelPayload): HTMLElement[] {
  return [
    element("p", "wheel-title", summaryLine(current.files.length, current.total_bytes, S.fileNoun)),
    element("p", "wheel-sub", shortNames(current.files)),
  ];
}

function emptyBlock(current: WheelPayload): HTMLElement[] {
  const message: string = hasMixedExtensions(current.files.map((file) => file.name))
    ? S.emptyMixed
    : S.emptyUnsupported;
  return [element("p", "wheel-title", message), element("p", "wheel-sub", shortNames(current.files))];
}

function progressBlock(): HTMLElement[] {
  const percent: number = progressPercent(job.done, job.total);
  const ring: SVGSVGElement = svgElement("svg", {
    viewBox: `0 0 ${RING_SIZE} ${RING_SIZE}`,
    width: String(RING_SIZE),
    height: String(RING_SIZE),
  });
  const mid: string = String(RING_SIZE / 2);
  ring.append(
    svgElement("circle", {
      class: "progress-track",
      cx: mid,
      cy: mid,
      r: String(RING_RADIUS),
      "stroke-width": RING_STROKE,
    }),
    svgElement("circle", {
      class: "progress-bar",
      cx: mid,
      cy: mid,
      r: String(RING_RADIUS),
      "stroke-width": RING_STROKE,
      "stroke-dasharray": String(RING_CIRCUMFERENCE),
      "stroke-dashoffset": String(RING_CIRCUMFERENCE * (1 - percent / 100)),
      transform: `rotate(-90 ${mid} ${mid})`,
    }),
  );
  const ringBox: HTMLDivElement = element("div", "progress");
  const value: HTMLDivElement = element("div", "progress-value", `${percent}%`);
  ringBox.append(ring, value);
  const cancel: HTMLButtonElement = element("button", "btn btn-outline", S.cancel);
  cancel.addEventListener("click", () => void cancelRunningJob());
  return [ringBox, element("p", "wheel-sub", S.runningTitle), cancel];
}

function errorBlock(): HTMLElement[] {
  const actions: HTMLDivElement = element("div", "wheel-actions");
  const retry: HTMLButtonElement = element("button", "btn btn-outline", S.retry);
  retry.addEventListener("click", () => retryLoad());
  const skip: HTMLButtonElement = element("button", "btn btn-ghost", S.skip);
  skip.addEventListener("click", () => void discardRequest());
  actions.append(retry, skip);
  return [
    element("span", "warn-mark", "!"),
    element("p", "wheel-title", S.errorTitle),
    element("p", "wheel-sub", errorReason),
    actions,
  ];
}

function renderWheel(): void {
  wheel.replaceChildren();
  sectorNodes = [];
  visibleItems = [];
  let blocks: HTMLElement[];
  if (mode === "ready" && payload !== null) {
    if (payload.items.length === 0) {
      blocks = emptyBlock(payload);
    } else {
      drawSectors(visibleFor(payload.items, tab));
      blocks = summaryBlock(payload);
    }
  } else if (mode === "running") {
    blocks = progressBlock();
  } else if (mode === "error") {
    blocks = errorBlock();
  } else if (mode === "collecting") {
    blocks = [element("p", "wheel-title", S.collectingFiles)];
  } else {
    blocks = [element("p", "wheel-title", S.dropTitle), element("p", "wheel-sub", S.dropHint)];
  }
  center.replaceChildren(...blocks);
}

function render(): void {
  renderBanner();
  renderTabs();
  renderWheel();
}

function setTab(value: string): void {
  tab = value;
  render();
}

function focusedSectorIndex(): number | null {
  const focused: Element | null = document.activeElement;
  if (!(focused instanceof SVGPathElement)) {
    return null;
  }
  const index: number = sectorNodes.indexOf(focused);
  return index === -1 ? null : index;
}

function moveFocus(key: NavigationKey, shift: boolean): void {
  if (sectorNodes.length === 0) {
    return;
  }
  const current: number = focusedSectorIndex() ?? -1;
  sectorNodes[nextIndex(current, sectorNodes.length, key, shift)]?.focus();
}

// WHY: the request is stored before the first load, so a later batch can tell stale results apart by revision.
function acceptRequest(next: MenuRequestState): void {
  if (next.revision < latestRevision) {
    return;
  }
  latestRevision = next.revision;
  request = next;
  shownAtMs = Date.now();
  banner = null;
  clearTimeout(closeTimer);
  if (next.collecting) {
    mode = "collecting";
    render();
    return;
  }
  void loadPayload(next);
}

async function loadPayload(current: MenuRequestState): Promise<void> {
  try {
    const loaded: WheelPayload = await invoke<WheelPayload>("menu_payload", {
      paths: current.paths,
    });
    if (current.revision !== latestRevision) {
      return;
    }
    payload = loaded;
    mode = "ready";
    // WHY: a job may still be running from before this window was opened; show its progress, not the wheel.
    const active: JobSnapshot | null = await invoke<JobSnapshot | null>("active_job");
    if (current.revision !== latestRevision) {
      return;
    }
    if (active !== null) {
      adoptRunningJob(active);
    }
  } catch (error: unknown) {
    if (current.revision !== latestRevision) {
      return;
    }
    errorReason = reasonOf(error);
    mode = "error";
  }
  render();
}

function adoptRunningJob(snapshot: JobSnapshot): void {
  mode = "running";
  job = { id: snapshot.id, done: snapshot.done, total: snapshot.total };
}

function retryLoad(): void {
  if (request !== null) {
    void loadPayload(request);
  }
}

async function runItem(item: WheelItem): Promise<void> {
  if (mode !== "ready" || request === null) {
    return;
  }
  const paths: string[] = request.paths;
  mode = "running";
  job = { id: null, done: 0, total: 0 };
  banner = null;
  render();
  try {
    const id: number = await invoke<number>("run_action", { id: item.id, paths });
    // WHY: job events can arrive before this reply, so the id is only filled in when still unknown.
    if (job.id === null) {
      job.id = id;
    }
  } catch (error: unknown) {
    mode = "ready";
    banner = reasonOf(error);
    render();
  }
}

async function cancelRunningJob(): Promise<void> {
  const id: number | null = job.id;
  if (id === null) {
    return;
  }
  try {
    await invoke<boolean>("cancel_job", { id });
  } catch (error: unknown) {
    banner = reasonOf(error);
    render();
  }
}

function onJobUpdate(snapshot: JobSnapshot): void {
  mode = "running";
  job = { id: snapshot.id, done: snapshot.done, total: snapshot.total };
  render();
}

function onJobFinished(snapshot: JobSnapshot): void {
  const outcome: JobOutcome = snapshot.state;
  // WHY: a failed batch returns to the wheel with the error, so the user can retry or pick another action.
  if (outcome.kind === "finished" && outcome.failed > 0) {
    mode = "ready";
    banner = S.failedFiles(outcome.failed);
    render();
    return;
  }
  // WHY: success and cancel both close the window; only a failure needs the user to read a message.
  mode = "running";
  job = { id: snapshot.id, done: snapshot.done, total: snapshot.total };
  render();
  closeTimer = setTimeout(() => void appWindow.hide(), CLOSE_DELAY_MS);
}

async function discardRequest(): Promise<void> {
  if (request === null) {
    await appWindow.hide();
    return;
  }
  try {
    await invoke("discard_pending", { paths: request.paths, revision: request.revision });
    request = null;
    payload = null;
    mode = "idle";
    await appWindow.hide();
  } catch (error: unknown) {
    banner = reasonOf(error);
    render();
  }
}

// WHY: Alt is hold-to-reveal. Releasing it must always return to the basic tab, even if the
// keyup is lost (for example Alt+Tab moves focus away), so blur also releases it.
function releaseAlt(): void {
  if (altHeld) {
    altHeld = false;
    setTab(BASIC_TAB);
  }
}

function onKeyDown(event: KeyboardEvent): void {
  const action = actionForKey(event.key);
  if (action === "close") {
    void appWindow.hide();
    return;
  }
  if (action === "tools") {
    // WHY: an auto-repeat keydown would re-enter the same state, so only the first press counts.
    if (!event.repeat && !altHeld) {
      altHeld = true;
      setTab(TOOLS_TAB);
    }
    return;
  }
  if (action === "activate") {
    const index: number | null = focusedSectorIndex();
    const item: WheelItem | undefined = index === null ? undefined : visibleItems[index];
    if (item !== undefined) {
      event.preventDefault();
      void runItem(item);
    }
    return;
  }
  const key: string = event.key;
  if (isNavigationKey(key)) {
    event.preventDefault();
    moveFocus(key, event.shiftKey);
  }
}

function onKeyUp(event: KeyboardEvent): void {
  if (actionForKey(event.key) === "tools") {
    releaseAlt();
  }
}

// WHY: the window hides on blur only when no job is running; a running job must keep its progress visible.
async function hideUnlessRunning(): Promise<void> {
  // WHY: lấy giờ trước khi chờ invoke, để độ trễ IPC không đẩy blur sớm ra ngoài khoảng ân hạn.
  const blurAtMs: number = Date.now();
  try {
    await new Promise<void>((resolve: () => void): void => {
      window.setTimeout(resolve, BLUR_CLICK_DELAY_MS);
    });
    const active: JobSnapshot | null = await invoke<JobSnapshot | null>("active_job");
    const feedbackVisible: boolean = mode === "running" || mode === "error" || banner !== null;
    if (shouldHideOnBlur(blurAtMs, shownAtMs, active !== null || feedbackVisible)) {
      await appWindow.hide();
    }
  } catch (error: unknown) {
    banner = reasonOf(error);
    render();
  }
}

async function start(): Promise<void> {
  // WHY: listeners go first, so no event is lost between the initial read and registration.
  await listen<MenuRequestState>("menu-request", (event) => acceptRequest(event.payload));
  await listen<JobSnapshot>("job-update", (event) => onJobUpdate(event.payload));
  await listen<JobSnapshot>("job-finished", (event) => onJobFinished(event.payload));
  const pending: MenuRequestState = await invoke<MenuRequestState>("pending_request");
  if (pending.paths.length > 0) {
    acceptRequest(pending);
  }
}

buildSkeleton();
render();
window.addEventListener("keydown", onKeyDown);
window.addEventListener("keyup", onKeyUp);
window.addEventListener("blur", () => {
  releaseAlt();
  void hideUnlessRunning();
});
start().catch((error: unknown) => {
  errorReason = reasonOf(error);
  mode = "error";
  render();
});
