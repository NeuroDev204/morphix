export type Group = "image" | "audio" | "video" | "plain" | "subtitle" | "table" | "doc";

export const MAX_BYTES = 200 * 1024 * 1024; // 200 MB

const ALIASES: Record<string, string> = {
  jpeg: "jpg",
  tif: "tiff",
  htm: "html",
  markdown: "md",
  aif: "aiff",
};

export function extOf(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot === -1 || dot === 0) return "";
  const raw = name.slice(dot + 1).toLowerCase();
  return ALIASES[raw] || raw;
}

const GROUPS: Record<Group, { inputs: string[]; targets: string[] }> = {
  image: {
    inputs: ["jpg", "png", "webp", "avif", "gif", "tiff", "bmp"],
    targets: ["jpg", "png", "webp", "avif", "gif", "tiff", "bmp"],
  },
  audio: {
    inputs: ["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"],
    targets: ["mp3", "m4a", "wav", "flac", "ogg", "opus"],
  },
  video: {
    inputs: ["mp4", "mov", "mkv", "webm", "avi", "wmv"],
    targets: ["mp4", "mkv", "webm", "mov", "avi", "gif", "mp3"],
  },
  plain: {
    inputs: ["txt", "md"],
    targets: ["txt", "md", "html"],
  },
  subtitle: {
    inputs: ["srt", "vtt"],
    targets: ["srt", "vtt", "txt"],
  },
  table: {
    inputs: ["csv", "tsv"],
    targets: ["csv", "tsv", "md"],
  },
  doc: {
    inputs: ["html", "docx"],
    targets: ["md", "html", "txt"],
  },
};

const SPECIFIC_TARGETS: Record<string, string[]> = {
  txt: ["md"],
  md: ["txt", "html"],
  srt: ["vtt", "txt"],
  vtt: ["srt", "txt"],
  csv: ["tsv", "md"],
  tsv: ["csv", "md"],
  html: ["md", "txt"],
  docx: ["md", "html", "txt"],
};

export function groupOf(ext: string): Group | null {
  const normalized = ALIASES[ext.toLowerCase()] || ext.toLowerCase();
  for (const [group, conf] of Object.entries(GROUPS) as [Group, { inputs: string[] }][]) {
    if (conf.inputs.includes(normalized)) {
      return group;
    }
  }
  return null;
}

export function targetsFor(ext: string): string[] {
  const normalized = ALIASES[ext.toLowerCase()] || ext.toLowerCase();
  const group = groupOf(normalized);
  if (!group) return [];

  if (SPECIFIC_TARGETS[normalized]) {
    return SPECIFIC_TARGETS[normalized];
  }

  return GROUPS[group].targets.filter((t) => t !== normalized);
}

export function sharedTargets(exts: string[]): string[] | null {
  if (exts.length === 0) return null;

  const normalizedList = exts.map((e) => ALIASES[e.toLowerCase()] || e.toLowerCase());
  const firstGroup = groupOf(normalizedList[0]);
  if (!firstGroup) return null;

  for (let i = 1; i < normalizedList.length; i++) {
    const g = groupOf(normalizedList[i]);
    if (!g || g !== firstGroup) return null;
  }

  const union = new Set<string>();
  for (const ext of normalizedList) {
    for (const t of targetsFor(ext)) {
      union.add(t);
    }
  }

  return GROUPS[firstGroup].targets.filter((t) => union.has(t));
}

export function outputName(name: string, target: string, taken: Set<string>): string {
  const dot = name.lastIndexOf(".");
  const base = dot === -1 ? name : name.slice(0, dot);

  let candidate = `${base}.${target}`;
  let counter = 2;
  while (taken.has(candidate)) {
    candidate = `${base} (${counter}).${target}`;
    counter++;
  }

  taken.add(candidate);
  return candidate;
}

// shortcut: Wasm đơn luồng chạy chậm nên dùng preset ultrafast thay vì medium;
// WebM dùng realtime cpu-used 8 thay vì row-mt 1;
// Các đích âm thanh thêm -vn để bỏ qua video stream khi trích xuất hoặc chuyển đổi.
export function ffmpegArgs(target: string): string[] | null {
  switch (target) {
    case "mp4":
    case "mov":
      return [
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "23",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "128k",
        "-movflags",
        "+faststart",
      ];
    case "mkv":
      return [
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "23",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "libopus",
        "-b:a",
        "128k",
      ];
    case "webm":
      return [
        "-c:v",
        "libvpx-vp9",
        "-crf",
        "32",
        "-b:v",
        "0",
        "-deadline",
        "realtime",
        "-cpu-used",
        "8",
        "-c:a",
        "libopus",
        "-b:a",
        "128k",
      ];
    case "avi":
      return [
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "23",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "libmp3lame",
        "-b:a",
        "192k",
      ];
    case "gif":
      return ["-vf", "fps=10,scale=480:-1:flags=lanczos"];
    case "mp3":
      return ["-vn", "-c:a", "libmp3lame", "-b:a", "192k"];
    case "m4a":
      return ["-vn", "-c:a", "aac", "-b:a", "192k"];
    case "wav":
      return ["-vn", "-c:a", "pcm_s16le"];
    case "flac":
      return ["-vn", "-c:a", "flac"];
    case "ogg":
      return ["-vn", "-c:a", "libvorbis", "-q:a", "5"];
    case "opus":
      return ["-vn", "-c:a", "libopus", "-b:a", "128k"];
    case "tiff":
    case "bmp":
      return [];
    default:
      return null;
  }
}

const MIME_MAP: Record<string, string> = {
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  png: "image/png",
  webp: "image/webp",
  avif: "image/avif",
  gif: "image/gif",
  tiff: "image/tiff",
  bmp: "image/bmp",
  mp3: "audio/mpeg",
  m4a: "audio/mp4",
  wav: "audio/wav",
  flac: "audio/flac",
  ogg: "audio/ogg",
  opus: "audio/opus",
  aiff: "audio/aiff",
  wma: "audio/x-ms-wma",
  mp4: "video/mp4",
  mov: "video/quicktime",
  mkv: "video/x-matroska",
  webm: "video/webm",
  avi: "video/x-msvideo",
  wmv: "video/x-ms-wmv",
  txt: "text/plain",
  md: "text/markdown",
  html: "text/html",
  srt: "text/plain",
  vtt: "text/vtt",
  csv: "text/csv",
  tsv: "text/tab-separated-values",
  docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  zip: "application/zip",
};

export function mimeOf(ext: string): string {
  const normalized = ALIASES[ext.toLowerCase()] || ext.toLowerCase();
  return MIME_MAP[normalized] || "application/octet-stream";
}
