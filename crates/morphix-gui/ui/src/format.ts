import { S } from "./strings";
const KILOBYTE: number = 1024;
const MEGABYTE: number = 1024 * 1024;
const ELLIPSIS: string = "…";

// WHY: a hand-rolled format instead of toLocaleString, so the output (comma decimal)
// does not depend on the browser or OS locale.
export function formatSize(bytes: number): string {
  // WHY: below 1 KB, rounding to KB would print a false "0 KB" for small files.
  if (bytes < KILOBYTE) {
    return `${Math.round(bytes)} B`;
  }
  if (bytes < MEGABYTE) {
    return `${Math.round(bytes / KILOBYTE)} KB`;
  }
  const megabytes: number = bytes / MEGABYTE;
  // WHY: one decimal only below 10 MB keeps the figure short; larger sizes are already precise enough.
  const text: string = megabytes < 10 ? megabytes.toFixed(1) : Math.round(megabytes).toString();
  return `${text.replace(".", ",")} MB`;
}

export function summaryLine(count: number, bytes: number, noun: string): string {
  return `${count} ${noun} · ${formatSize(bytes)}`;
}

// WHY: clamped so a stale or out-of-order count never draws a ring past 100% or below 0%.
export function progressPercent(done: number, total: number): number {
  if (total <= 0) {
    return 0;
  }
  const ratio: number = Math.min(Math.max(done / total, 0), 1);
  return Math.round(ratio * 100);
}

// WHY: Array.from splits by code point, so a Vietnamese letter or emoji is never cut in half.
// WHY middle-cut: the head identifies the file and the tail (plus extension) tells versions apart.
export function truncateName(name: string, max: number): string {
  const chars: string[] = Array.from(name);
  if (chars.length <= max) {
    return name;
  }
  if (max <= 1) {
    return max === 1 ? ELLIPSIS : "";
  }

  // WHY dotIndex > 0: a leading dot marks a hidden file (".bashrc"), not an extension.
  const dotIndex: number = chars.lastIndexOf(".");
  const hasExtension: boolean = dotIndex > 0;
  const extension: string[] = hasExtension ? chars.slice(dotIndex) : [];
  // WHY: keep the extension only if at least one base char and the ellipsis still fit,
  // otherwise the extension would consume the whole budget.
  const keepExtension: boolean = hasExtension && extension.length + 2 <= max;
  const base: string[] = keepExtension ? chars.slice(0, dotIndex) : chars;
  const suffix: string[] = keepExtension ? extension : [];

  const budget: number = max - suffix.length - 1;
  const headLength: number = Math.ceil(budget / 2);
  const tailLength: number = budget - headLength;
  return [
    ...base.slice(0, headLength),
    ELLIPSIS,
    ...base.slice(base.length - tailLength),
    ...suffix,
  ].join("");
}

// WHY: the payload labels every file outside image and PDF as "other", so kinds cannot tell
// a video from an audio file. The extension is the only per-file signal the UI still has.
export function hasMixedExtensions(names: readonly string[]): boolean {
  const extensions: Set<string> = new Set(names.map(extensionOf));
  return extensions.size > 1;
}

function extensionOf(name: string): string {
  const dot: number = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

const MINUTE_SECONDS: number = 60;
const HOUR_SECONDS: number = 3600;
const DAY_SECONDS: number = 86400;

// WHY: relative wording ("2 phút", "Hôm qua") matches the khay wireframe; absolute dates would wrap in the 400 px window.
export function formatRelativeTime(nowUnix: number, thenUnix: number): string {
  // WHY max: a timestamp ahead of this clock (skew, restored backup) must not print a negative age.
  const ageSeconds: number = Math.max(0, nowUnix - thenUnix);
  if (ageSeconds < MINUTE_SECONDS) {
    return S.timeJustNow;
  }
  if (ageSeconds < HOUR_SECONDS) {
    return S.timeMinutesAgo(Math.floor(ageSeconds / MINUTE_SECONDS));
  }
  if (ageSeconds < DAY_SECONDS) {
    return S.timeHoursAgo(Math.floor(ageSeconds / HOUR_SECONDS));
  }
  if (ageSeconds < 2 * DAY_SECONDS) {
    return S.timeYesterday;
  }
  return S.timeDaysAgo(Math.floor(ageSeconds / DAY_SECONDS));
}
