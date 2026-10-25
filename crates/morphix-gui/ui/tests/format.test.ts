import { describe, expect, it } from "vitest";
import { formatRelativeTime, formatSize, hasMixedExtensions, progressPercent, summaryLine, truncateName } from "../src/format";

const MEGABYTE: number = 1024 * 1024;

describe("formatSize", () => {
  it("uses comma decimals and matches the wireframe", () => {
    expect(formatSize(8.4 * MEGABYTE)).toBe("8,4 MB");
    expect(formatSize(412 * MEGABYTE)).toBe("412 MB");
    expect(formatSize(512 * 1024)).toBe("512 KB");
  });
  it("shows sizes under 1 KB in bytes instead of a false 0 KB", () => {
    expect(formatSize(0)).toBe("0 B");
    expect(formatSize(300)).toBe("300 B");
    expect(formatSize(1023)).toBe("1023 B");
  });
  it("switches to KB at exactly 1024 bytes", () => {
    expect(formatSize(1024)).toBe("1 KB");
  });
  it("drops the decimal from 10 MB upward", () => {
    expect(formatSize(45 * MEGABYTE)).toBe("45 MB");
  });
});

describe("summaryLine", () => {
  it("joins count, noun and size", () => {
    expect(summaryLine(3, 8.4 * MEGABYTE, "ảnh")).toBe("3 ảnh · 8,4 MB");
  });
  it("shows two small files in bytes, not a false zero kilobytes", () => {
    expect(summaryLine(2, 400, "ảnh")).toBe("2 ảnh · 400 B");
  });
});

describe("truncateName", () => {
  it("keeps short names whole", () => {
    expect(truncateName("ngan.jpg", 20)).toBe("ngan.jpg");
  });
  it("keeps the extension of long names", () => {
    const cut: string = truncateName("scan-hop-dong-thue-nha-ban-cuoi-cung-final-v2.png", 24);
    expect(cut).toBe("scan-hop-d…-final-v2.png");
    expect(Array.from(cut).length).toBe(24);
  });
  it("does not split Vietnamese characters", () => {
    expect(truncateName("Ảnh chụp màn hình rất dài.jpg", 12)).toBe("Ảnh …dài.jpg");
  });
  it("counts code points, so an emoji is never cut in half", () => {
    expect(truncateName("😀😀😀😀😀😀.png", 6)).toBe("😀….png");
  });
  it("cuts a name without an extension from the middle", () => {
    expect(truncateName("bao-cao-quy-III-phien-ban-cuoi-cung", 16)).toBe("bao-cao-…oi-cung");
  });
  it("treats a leading dot as a hidden file, not an extension", () => {
    expect(truncateName(".bashrc-backup-long-name", 12)).toBe(".bashr…-name");
  });
  it("drops an extension longer than the budget and cuts from the middle", () => {
    expect(truncateName("file.verylongextension", 5)).toBe("fi…on");
  });
  it("returns only an ellipsis or nothing when max is 1 or less", () => {
    expect(truncateName("abc", 1)).toBe("…");
    expect(truncateName("abc", 0)).toBe("");
    expect(truncateName("a", 1)).toBe("a");
  });
});

describe("progressPercent", () => {
  it("rounds done over total to a whole percent", () => {
    expect(progressPercent(1, 2)).toBe(50);
    expect(progressPercent(1, 3)).toBe(33);
  });
  it("is 0 for an empty job and clamps out-of-range counts", () => {
    expect(progressPercent(0, 0)).toBe(0);
    expect(progressPercent(-1, 2)).toBe(0);
    expect(progressPercent(5, 2)).toBe(100);
  });
});

describe("hasMixedExtensions", () => {
  it("is true for a video and an audio file, which share the \"other\" kind", () => {
    expect(hasMixedExtensions(["clip.mp4", "song.mp3"])).toBe(true);
  });
  it("is false for one unsupported type, ignoring case", () => {
    expect(hasMixedExtensions(["a.MP4", "b.mp4"])).toBe(false);
  });
  it("treats files without an extension as one type", () => {
    expect(hasMixedExtensions(["notes", ".hidden"])).toBe(false);
  });
  it("is false for an empty batch", () => {
    expect(hasMixedExtensions([])).toBe(false);
  });
});

describe("formatRelativeTime", () => {
  const NOW: number = 1_000_000;
  it("describes recent finishes in the wireframe's words", () => {
    expect(formatRelativeTime(NOW, NOW)).toBe("Vừa xong");
    expect(formatRelativeTime(NOW, NOW - 120)).toBe("2 phút");
    expect(formatRelativeTime(NOW, NOW - 3 * 3600)).toBe("3 giờ");
    expect(formatRelativeTime(NOW, NOW - 25 * 3600)).toBe("Hôm qua");
    expect(formatRelativeTime(NOW, NOW - 3 * 86400)).toBe("3 ngày");
  });
  it("treats a timestamp in the future (clock skew) as just finished", () => {
    expect(formatRelativeTime(NOW, NOW + 500)).toBe("Vừa xong");
  });
});
