import { extOf, groupOf, targetsFor, mimeOf, ffmpegArgs, MAX_BYTES } from "./catalog.ts";
import { convertPureText } from "./text.ts";

export type OnProgress = (ratio: number | null, note?: string) => void;

let avifSupportCache: boolean | null = null;

export async function canEncodeAvif(): Promise<boolean> {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return false;
  }
  if (avifSupportCache !== null) {
    return avifSupportCache;
  }
  try {
    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;
    const blob = await new Promise<Blob | null>((resolve) => {
      canvas.toBlob(resolve, "image/avif");
    });
    avifSupportCache = !!blob && blob.type === "image/avif";
  } catch {
    avifSupportCache = false;
  }
  return avifSupportCache;
}

let ffmpegInstance: any = null;
let ffmpegLoadingPromise: Promise<any> | null = null;

async function getFFmpeg(onProgress: OnProgress): Promise<any> {
  if (ffmpegInstance && ffmpegInstance.loaded) {
    return ffmpegInstance;
  }
  if (ffmpegLoadingPromise) {
    return ffmpegLoadingPromise;
  }

  ffmpegLoadingPromise = (async () => {
    onProgress(null, "Đang tải bộ chuyển đổi (khoảng 30 MB)");
    const { FFmpeg } = await import("@ffmpeg/ffmpeg");
    const { toBlobURL } = await import("@ffmpeg/util");

    const baseURL = "https://cdn.jsdelivr.net/npm/@ffmpeg/core@0.12.10/dist/umd";
    const ffmpeg = new FFmpeg();

    const coreURL = await toBlobURL(`${baseURL}/ffmpeg-core.js`, "text/javascript");
    const wasmURL = await toBlobURL(`${baseURL}/ffmpeg-core.wasm`, "application/wasm");

    await ffmpeg.load({
      coreURL,
      wasmURL,
    });

    ffmpegInstance = ffmpeg;
    return ffmpeg;
  })();

  try {
    return await ffmpegLoadingPromise;
  } finally {
    ffmpegLoadingPromise = null;
  }
}

export function resetEngines(): void {
  if (ffmpegInstance) {
    try {
      ffmpegInstance.terminate();
    } catch {}
    ffmpegInstance = null;
  }
  ffmpegLoadingPromise = null;
}

async function runFFmpeg(
  inputBlob: Blob,
  inExt: string,
  target: string,
  onProgress: OnProgress
): Promise<Blob> {
  const args = ffmpegArgs(target);
  if (!args) {
    throw new Error("Không hỗ trợ chuyển sang định dạng này");
  }

  const ffmpeg = await getFFmpeg(onProgress);

  const progressHandler = ({ progress }: { progress: number }) => {
    if (typeof progress === "number" && !isNaN(progress)) {
      const clamped = Math.max(0, Math.min(1, progress));
      onProgress(clamped, "Đang chuyển đổi...");
    }
  };

  ffmpeg.on("progress", progressHandler);

  const inVirtualName = `in_${Date.now()}_${Math.random().toString(36).slice(2, 7)}.${inExt}`;
  const outVirtualName = `out_${Date.now()}_${Math.random().toString(36).slice(2, 7)}.${target}`;

  try {
    const { fetchFile } = await import("@ffmpeg/util");
    onProgress(0.1, "Đang chuẩn bị file");
    const fileData = await fetchFile(inputBlob);
    await ffmpeg.writeFile(inVirtualName, fileData);

    onProgress(0.2, "Đang chuyển đổi...");
    const execArgs = ["-i", inVirtualName, ...args, outVirtualName];
    const exitCode = await ffmpeg.exec(execArgs);
    if (exitCode !== 0) {
      throw new Error("Chuyển đổi thất bại");
    }

    const outputData = await ffmpeg.readFile(outVirtualName);
    const resultBlob = new Blob([outputData], { type: mimeOf(target) });
    if (resultBlob.size === 0) {
      throw new Error("Kết quả rỗng");
    }
    onProgress(1);
    return resultBlob;
  } catch (err: any) {
    if (err && err.message && (err.message.includes("File quá lớn") || err.message.includes("Kết quả rỗng"))) {
      throw err;
    }
    throw new Error(err?.message || "Chuyển đổi thất bại");
  } finally {
    try {
      await ffmpeg.deleteFile(inVirtualName);
    } catch {}
    try {
      await ffmpeg.deleteFile(outVirtualName);
    } catch {}
    ffmpeg.off("progress", progressHandler);
  }
}

export async function convertFile(
  file: File,
  target: string,
  onProgress: OnProgress
): Promise<Blob> {
  if (file.size > MAX_BYTES) {
    throw new Error("File quá lớn cho trình duyệt, hãy dùng app");
  }
  if (file.size === 0) {
    throw new Error("File rỗng");
  }

  const inExt = extOf(file.name);
  const supported = targetsFor(inExt);
  if (!supported.includes(target)) {
    throw new Error("Không hỗ trợ chuyển sang định dạng này");
  }

  const group = groupOf(inExt);
  if (!group) {
    throw new Error("Không hỗ trợ chuyển sang định dạng này");
  }

  // 1. Text & Doc formats
  if (group === "plain" || group === "subtitle" || group === "table" || group === "doc") {
    onProgress(0.2, "Đang xử lý văn bản");
    if (inExt === "docx") {
      const buffer = await file.arrayBuffer();
      const mammoth = await import("mammoth");
      if (target === "html") {
        const { value: html } = await mammoth.convertToHtml({ arrayBuffer: buffer });
        onProgress(1);
        const res = new Blob([html], { type: mimeOf(target) });
        if (res.size === 0) throw new Error("Kết quả rỗng");
        return res;
      } else if (target === "txt") {
        const { value: text } = await mammoth.extractRawText({ arrayBuffer: buffer });
        onProgress(1);
        const res = new Blob([text], { type: mimeOf(target) });
        if (res.size === 0) throw new Error("Kết quả rỗng");
        return res;
      } else if (target === "md") {
        const { value: html } = await mammoth.convertToHtml({ arrayBuffer: buffer });
        const TurndownService = (await import("turndown")).default;
        const td = new TurndownService();
        const md = td.turndown(html);
        onProgress(1);
        const res = new Blob([md], { type: mimeOf(target) });
        if (res.size === 0) throw new Error("Kết quả rỗng");
        return res;
      }
    }

    const textContent = await file.text();
    const pure = convertPureText(textContent, inExt, target);
    if (pure !== null) {
      onProgress(1);
      const res = new Blob([pure], { type: mimeOf(target) });
      if (res.size === 0) throw new Error("Kết quả rỗng");
      return res;
    }

    if (inExt === "md" && target === "html") {
      const { marked } = await import("marked");
      const parsed = await marked.parse(textContent);
      const fullHtml = `<!doctype html><meta charset="utf-8"><title>${file.name}</title>\n${parsed}`;
      onProgress(1);
      const res = new Blob([fullHtml], { type: mimeOf("html") });
      if (res.size === 0) throw new Error("Kết quả rỗng");
      return res;
    }

    if (inExt === "md" && target === "txt") {
      const { marked } = await import("marked");
      const parsed = await marked.parse(textContent);
      const parser = new DOMParser();
      const doc = parser.parseFromString(parsed, "text/html");
      const plain = doc.body.textContent || "";
      onProgress(1);
      const res = new Blob([plain], { type: mimeOf("txt") });
      if (res.size === 0) throw new Error("Kết quả rỗng");
      return res;
    }

    if (inExt === "html" && target === "md") {
      const TurndownService = (await import("turndown")).default;
      const td = new TurndownService();
      const md = td.turndown(textContent);
      onProgress(1);
      const res = new Blob([md], { type: mimeOf("md") });
      if (res.size === 0) throw new Error("Kết quả rỗng");
      return res;
    }

    if (inExt === "html" && target === "txt") {
      const parser = new DOMParser();
      const doc = parser.parseFromString(textContent, "text/html");
      const plain = doc.body.textContent || "";
      onProgress(1);
      const res = new Blob([plain], { type: mimeOf("txt") });
      if (res.size === 0) throw new Error("Kết quả rỗng");
      return res;
    }
  }

  // 2. Image formats
  if (group === "image") {
    const canvasTargets = ["jpg", "png", "webp", "avif"];
    let decodedBitmap: ImageBitmap | null = null;
    let decodedCanvas: HTMLCanvasElement | null = null;

    if (typeof createImageBitmap !== "undefined") {
      try {
        decodedBitmap = await createImageBitmap(file);
      } catch {
        decodedBitmap = null;
      }
    }

    if (decodedBitmap && typeof document !== "undefined") {
      const canvas = document.createElement("canvas");
      canvas.width = decodedBitmap.width;
      canvas.height = decodedBitmap.height;
      const ctx = canvas.getContext("2d");
      if (ctx) {
        if (target === "jpg") {
          ctx.fillStyle = "#ffffff";
          ctx.fillRect(0, 0, canvas.width, canvas.height);
        }
        ctx.drawImage(decodedBitmap, 0, 0);
        decodedCanvas = canvas;
      }
      decodedBitmap.close?.();
    }

    if (decodedCanvas && canvasTargets.includes(target)) {
      let canUseCanvas = true;
      if (target === "avif") {
        canUseCanvas = await canEncodeAvif();
      }

      if (canUseCanvas) {
        onProgress(0.5, "Đang xử lý ảnh");
        const mime = mimeOf(target);
        const blob = await new Promise<Blob | null>((resolve) => {
          decodedCanvas!.toBlob(resolve, mime, 0.9);
        });
        if (blob && blob.size > 0 && blob.type === mime) {
          onProgress(1);
          return blob;
        }
      }
    }

    // Other image targets or canvas fallback -> ffmpeg
    let ffmpegInputBlob: Blob = file;
    let ffmpegInputExt = inExt;

    if (decodedCanvas) {
      const pngBlob = await new Promise<Blob | null>((resolve) => {
        decodedCanvas!.toBlob(resolve, "image/png");
      });
      if (pngBlob && pngBlob.size > 0) {
        ffmpegInputBlob = pngBlob;
        ffmpegInputExt = "png";
      }
    }

    return await runFFmpeg(ffmpegInputBlob, ffmpegInputExt, target, onProgress);
  }

  // 3. Audio & Video
  return await runFFmpeg(file, inExt, target, onProgress);
}

export async function zipResults(items: { name: string; blob: Blob }[]): Promise<Blob> {
  const { zipSync } = await import("fflate");
  const zipData: Record<string, [Uint8Array, { level: 0 }]> = {};

  for (const item of items) {
    const buffer = await item.blob.arrayBuffer();
    zipData[item.name] = [new Uint8Array(buffer), { level: 0 }];
  }

  const zipped = zipSync(zipData);
  return new Blob([zipped], { type: "application/zip" });
}
