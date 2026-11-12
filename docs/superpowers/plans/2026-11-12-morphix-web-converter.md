# Morphix Web Converter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Khối `#playground` trên website chuyển đổi thật nhiều file trong trình duyệt và cho tải kết quả về.

**Architecture:** Một module thuần (`catalog.ts`, `text.ts`) quyết định nhóm, định dạng đích, tên file ra, argv ffmpeg và các bộ chuyển văn bản tự viết; module này có test. `engines.ts` là lớp chạm trình duyệt: canvas cho ảnh, ffmpeg.wasm một luồng tải lười cho media, thư viện tải lười cho Markdown/HTML/DOCX. `DropzonePlayground.tsx` chỉ giữ state và vẽ giao diện.

**Tech Stack:** Next.js 15 (`output: "export"`), React 19, Tailwind 4, TypeScript strict. Thêm: `@ffmpeg/ffmpeg@0.12.15`, `@ffmpeg/util@0.12.2`, `marked`, `turndown` (+ `@types/turndown`), `mammoth`, `fflate`. Test bằng `node --test` (Node 22.23 tự bỏ kiểu TypeScript, không thêm framework).

**Spec:** `docs/superpowers/specs/2026-11-12-morphix-web-converter-design.md`

## Global Constraints

- Mọi lệnh chạy trong `website/`. Làm trên nhánh `feat/web-converter`, không commit thẳng vào `main`.
- Không sửa crate Rust nào.
- File người dùng không rời máy: không `fetch`/`XMLHttpRequest` nào mang nội dung file.
- ffmpeg và mọi thư viện chuyển đổi chỉ được nạp bằng `import()` động bên trong hàm chạy việc, không import tĩnh ở đầu file nào mà `page.tsx` kéo vào.
- ffmpeg dùng lõi một luồng `@ffmpeg/core@0.12.10` (không cần `SharedArrayBuffer`, không cần header COOP/COEP).
- Giới hạn `200 * 1024 * 1024` byte mỗi file.
- Chữ giao diện tiếng Việt; giữ lớp Tailwind tông teal của khối hiện tại.
- Không chèn HTML của người dùng vào trang đang chạy (`innerHTML`, `dangerouslySetInnerHTML`); chỉ phân tích bằng `DOMParser`.
- Kiểm tra cuối mỗi task: `npx tsc --noEmit` qua. `next lint` chưa cấu hình, không dùng.

## Review Focus

1. Đuôi viết hoa hoặc biến thể (`ANH.JPG`, `a.jpeg`, `b.tif`, `c.htm`) phải vào đúng nhóm; file không có đuôi là dòng "Chỉ có trong app". → test ở Task 1.
2. Hai file ra trùng tên (`a.png` và `a.jpg` cùng sang WebP) phải thành `a.webp` và `a (2).webp`, nếu không zip sẽ mất một file. → test ở Task 1.
3. CSV có ô chứa dấu phẩy, ngoặc kép, xuống dòng; SRT có BOM và CRLF. → test ở Task 2.
4. File 0 byte hoặc ảnh hỏng: dòng đó báo lỗi, các file sau vẫn chạy, không có dòng tải về rỗng. → kiểm tay ở Task 4, bước 6.
5. Bấm "Làm lại với file khác" hoặc thả thêm file khi đang chạy: không có kết quả cũ hiện lại sau khi xoá, nút chuyển bị khoá khi đang chạy. → kiểm tay ở Task 4, bước 6.

---

### Task 1: Bảng nhóm và định dạng đích (`catalog.ts`)

**Files:**
- Create: `website/src/lib/convert/catalog.ts`
- Test: `website/src/lib/convert/catalog.test.ts`
- Modify: `website/package.json` (script `test`), `website/tsconfig.json` (`"allowImportingTsExtensions": true`)

**Interfaces:**
- Produces:
  ```ts
  export type Group = "image" | "audio" | "video" | "plain" | "subtitle" | "table" | "doc";
  export const MAX_BYTES: number;                       // 200 * 1024 * 1024
  export function extOf(name: string): string;          // chữ thường, đã chuẩn hoá; "" nếu không có đuôi
  export function groupOf(ext: string): Group | null;   // null = không hỗ trợ
  export function targetsFor(ext: string): string[];    // đuôi đích, không gồm chính nó; [] nếu không hỗ trợ
  export function sharedTargets(exts: string[]): string[] | null;
  export function outputName(name: string, target: string, taken: Set<string>): string; // thêm tên vào taken
  export function mimeOf(ext: string): string;
  export function ffmpegArgs(target: string): string[] | null; // phần argv giữa input và output
  ```
- Chuẩn hoá đuôi: `jpeg→jpg`, `tif→tiff`, `htm→html`, `markdown→md`, `aif→aiff`.
- Nhóm theo đuôi đầu vào:
  - image: `jpg png webp avif gif tiff bmp`; đích theo thứ tự `jpg png webp avif gif tiff bmp`
  - audio: `mp3 m4a wav flac ogg opus aiff wma`; đích `mp3 m4a wav flac ogg opus`
  - video: `mp4 mov mkv webm avi wmv`; đích `mp4 mkv webm mov avi gif mp3`
  - plain: `txt→[md]`, `md→[txt, html]`
  - subtitle: `srt→[vtt, txt]`, `vtt→[srt, txt]`
  - table: `csv→[tsv, md]`, `tsv→[csv, md]`
  - doc: `html→[md, txt]`, `docx→[md, html, txt]`
- `sharedTargets`: `null` khi danh sách rỗng, có đuôi không hỗ trợ, hoặc các file khác nhóm. Ngược lại trả hợp các `targetsFor`, giữ thứ tự của nhóm.
- `ffmpegArgs`: chép giá trị từ `crates/morphix-core/src/ops/media.rs:24-96`, với ba khác biệt có chủ ý vì wasm một luồng chậm: `-preset ultrafast` thay `medium`; WebM thêm `-deadline realtime -cpu-used 8` và bỏ `-row-mt 1`; đích âm thanh (`mp3 m4a wav flac ogg opus`) thêm `-vn` ở đầu. Thêm `tiff` và `bmp` trả `[]`. Ghi một dòng `// shortcut:` nêu lý do lệch so với app. Đích khác trả `null`.

- [x] **Step 1: Thêm script và cờ tsconfig**

`package.json` scripts: `"test": "node --test \"src/lib/convert/*.test.ts\""`. `tsconfig.json` compilerOptions: `"allowImportingTsExtensions": true`. File test import bằng đuôi `.ts` (`from "./catalog.ts"`).

- [x] **Step 2: Viết test thất bại** (`catalog.test.ts`, dùng `node:test` và `node:assert/strict`)

```ts
test("extOf chuẩn hoá", () => {
  assert.equal(extOf("ANH.JPG"), "jpg");
  assert.equal(extOf("a.jpeg"), "jpg");
  assert.equal(extOf("b.tif"), "tiff");
  assert.equal(extOf("c.htm"), "html");
  assert.equal(extOf("README"), "");
  assert.equal(extOf("x.tar.gz"), "gz");
});
test("groupOf", () => {
  assert.equal(groupOf("png"), "image");
  assert.equal(groupOf("wma"), "audio");
  assert.equal(groupOf("wmv"), "video");
  assert.equal(groupOf("docx"), "doc");
  assert.equal(groupOf("pdf"), null);
  assert.equal(groupOf(""), null);
});
test("targetsFor bỏ chính nó", () => {
  assert.deepEqual(targetsFor("png"), ["jpg", "webp", "avif", "gif", "tiff", "bmp"]);
  assert.deepEqual(targetsFor("mp4"), ["mkv", "webm", "mov", "avi", "gif", "mp3"]);
  assert.deepEqual(targetsFor("wma"), ["mp3", "m4a", "wav", "flac", "ogg", "opus"]);
  assert.deepEqual(targetsFor("md"), ["txt", "html"]);
  assert.deepEqual(targetsFor("srt"), ["vtt", "txt"]);
  assert.deepEqual(targetsFor("7z"), []);
});
test("sharedTargets", () => {
  assert.deepEqual(sharedTargets(["png", "jpg"]), ["jpg", "png", "webp", "avif", "gif", "tiff", "bmp"]);
  assert.deepEqual(sharedTargets(["txt", "md"]), ["txt", "md", "html"]);
  assert.equal(sharedTargets(["png", "mp4"]), null);
  assert.equal(sharedTargets(["png", "pdf"]), null);
  assert.equal(sharedTargets(["csv", "srt"]), null);
  assert.equal(sharedTargets([]), null);
});
test("outputName không trùng", () => {
  const taken = new Set<string>();
  assert.equal(outputName("a.png", "webp", taken), "a.webp");
  assert.equal(outputName("a.jpg", "webp", taken), "a (2).webp");
  assert.equal(outputName("a.gif", "webp", taken), "a (3).webp");
  assert.equal(outputName("ghi chu.final.md", "html", taken), "ghi chu.final.html");
});
test("ffmpegArgs", () => {
  assert.deepEqual(ffmpegArgs("mp3"), ["-vn", "-c:a", "libmp3lame", "-b:a", "192k"]);
  assert.deepEqual(ffmpegArgs("gif"), ["-vf", "fps=10,scale=480:-1:flags=lanczos"]);
  assert.ok(ffmpegArgs("mp4")!.includes("ultrafast"));
  assert.deepEqual(ffmpegArgs("bmp"), []);
  assert.equal(ffmpegArgs("wmv"), null);
});
test("mimeOf", () => {
  assert.equal(mimeOf("webp"), "image/webp");
  assert.equal(mimeOf("mp3"), "audio/mpeg");
  assert.equal(mimeOf("md"), "text/markdown");
  assert.equal(mimeOf("zzz"), "application/octet-stream");
});
```

- [x] **Step 3: Chạy, xác nhận thất bại.** Run: `npm test`. Expected: FAIL, không tìm thấy `./catalog.ts`.
- [x] **Step 4: Viết `catalog.ts`** theo khối Interfaces. Một bảng dữ liệu `Record<Group, ...>` là đủ; không lớp, không enum.
- [x] **Step 5: Chạy lại.** Run: `npm test && npx tsc --noEmit`. Expected: 7 test PASS, tsc không lỗi.
- [x] **Step 6: Commit.** `git add website/package.json website/tsconfig.json website/src/lib/convert && git commit -m "feat(website): add conversion catalog for the web playground"`

---

### Task 2: Bộ chuyển văn bản tự viết (`text.ts`)

**Files:**
- Create: `website/src/lib/convert/text.ts`
- Test: `website/src/lib/convert/text.test.ts`

**Interfaces:**
- Produces:
  ```ts
  // Trả chuỗi kết quả cho cặp không cần thư viện; null nếu cặp cần thư viện (engines.ts lo).
  export function convertPureText(input: string, from: string, to: string): string | null;
  ```
- Cặp xử lý ở đây: `txt→md` (giữ nguyên), `srt↔vtt`, `srt|vtt→txt`, `csv↔tsv`, `csv|tsv→md`. Mọi cặp khác trả `null`.
- Quy tắc: bỏ BOM đầu chuỗi; đọc được cả CRLF, ghi ra LF. VTT ra bắt đầu bằng `WEBVTT\n\n`, mốc giờ dùng dấu chấm; SRT ra đánh số lại từ 1, mốc giờ dùng dấu phẩy. Phụ đề sang TXT bỏ số thứ tự, dòng mốc giờ và đầu `WEBVTT`, mỗi cue một dòng trống ngăn cách. CSV/TSV đọc theo RFC 4180 (ô trong ngoặc kép, `""` là một ngoặc kép, xuống dòng trong ô); khi ghi chỉ bọc ngoặc kép ô nào chứa dấu phân cách, ngoặc kép hoặc xuống dòng. Bảng Markdown: dòng đầu là tiêu đề, `|` trong ô thành `\|`, xuống dòng trong ô thành `<br>`.

- [x] **Step 1: Viết test thất bại**

```ts
const SRT = "﻿1\r\n00:00:01,000 --> 00:00:02,500\r\nXin chào\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nDòng một\r\nDòng hai\r\n";
const VTT = "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nXin chào\n\n00:00:03.000 --> 00:00:04.000\nDòng một\nDòng hai\n";

test("srt sang vtt", () => assert.equal(convertPureText(SRT, "srt", "vtt"), VTT));
test("vtt sang srt", () => assert.equal(
  convertPureText(VTT, "vtt", "srt"),
  "1\n00:00:01,000 --> 00:00:02,500\nXin chào\n\n2\n00:00:03,000 --> 00:00:04,000\nDòng một\nDòng hai\n"));
test("phụ đề sang txt", () => {
  assert.equal(convertPureText(SRT, "srt", "txt"), "Xin chào\n\nDòng một\nDòng hai\n");
  assert.equal(convertPureText(VTT, "vtt", "txt"), "Xin chào\n\nDòng một\nDòng hai\n");
});
test("csv sang tsv giữ ô có ngoặc", () => assert.equal(
  convertPureText('ten,ghi chu\r\n"An, Binh","noi ""a"""\r\n', "csv", "tsv"),
  'ten\tghi chu\nAn, Binh\t"noi ""a"""\n'));
test("tsv sang csv", () => assert.equal(
  convertPureText("a\tb\nx,y\tz\n", "tsv", "csv"), 'a,b\n"x,y",z\n'));
test("csv sang bảng markdown", () => assert.equal(
  convertPureText('a,b\n"1|2","dong 1\ndong 2"\n', "csv", "md"),
  "| a | b |\n| --- | --- |\n| 1\\|2 | dong 1<br>dong 2 |\n"));
test("txt sang md giữ nguyên", () => assert.equal(convertPureText("abc\n", "txt", "md"), "abc\n"));
test("cặp cần thư viện trả null", () => {
  assert.equal(convertPureText("# a", "md", "html"), null);
  assert.equal(convertPureText("<p>a</p>", "html", "md"), null);
});
```

- [x] **Step 2: Chạy, xác nhận thất bại.** Run: `npm test`. Expected: FAIL, không tìm thấy `./text.ts`.
- [x] **Step 3: Viết `text.ts`.** Một hàm đọc cue dùng chung cho SRT và VTT, một hàm đọc bảng nhận dấu phân cách. Không import gì.
- [x] **Step 4: Chạy lại.** Run: `npm test && npx tsc --noEmit`. Expected: tất cả PASS.
- [x] **Step 5: Commit.** `git commit -am "feat(website): add subtitle and table text converters"` (sau `git add website/src/lib/convert`)

---

### Task 3: Lớp chạy trong trình duyệt (`engines.ts`)

**Files:**
- Create: `website/src/lib/convert/engines.ts`
- Modify: `website/package.json`, `website/package-lock.json` (cài thư viện)

**Interfaces:**
- Consumes: mọi export của `catalog.ts`; `convertPureText` của `text.ts`.
- Produces:
  ```ts
  export type OnProgress = (ratio: number | null, note?: string) => void; // ratio 0..1, null = chưa biết
  export function convertFile(file: File, target: string, onProgress: OnProgress): Promise<Blob>;
  export function canEncodeAvif(): Promise<boolean>;
  export function zipResults(items: { name: string; blob: Blob }[]): Promise<Blob>;
  export function resetEngines(): void; // dừng ffmpeg đang chạy, lần sau nạp lại
  ```
- `convertFile` ném `Error` có thông báo tiếng Việt khi: file lớn hơn `MAX_BYTES` ("File quá lớn cho trình duyệt, hãy dùng app"), file 0 byte, cặp không có trong `targetsFor`, ffmpeg trả mã khác 0, hoặc kết quả 0 byte. Blob trả về có `type` là `mimeOf(target)`.

- [x] **Step 1: Cài thư viện.** Run: `npm install @ffmpeg/ffmpeg@0.12.15 @ffmpeg/util@0.12.2 marked turndown mammoth fflate && npm install -D @types/turndown`
- [x] **Step 2: Đối chiếu tài liệu trước khi viết.** Mở README của `@ffmpeg/ffmpeg` 0.12 và ví dụ Next.js chính thức trong repo `ffmpegwasm/ffmpeg.wasm`; xác nhận các lời gọi `new FFmpeg()`, `load({ coreURL, wasmURL })`, `on("progress")`, `writeFile`, `exec`, `readFile`, `deleteFile`, `terminate`, và `toBlobURL` của `@ffmpeg/util`. Nếu tên nào khác, theo tài liệu.
- [x] **Step 3: Viết đường ảnh.** Thử `createImageBitmap(file)`. Đích `jpg png webp avif` và giải mã được: vẽ lên canvas, `canvas.toBlob(mime, 0.9)`; đích `jpg` tô nền trắng trước khi vẽ. Các trường hợp còn lại đi ffmpeg: đầu vào là PNG xuất từ canvas nếu giải mã được, nếu không thì chính file gốc. `canEncodeAvif` thử `toBlob("image/avif")` trên canvas 1×1 và kiểm `blob.type === "image/avif"`.
- [x] **Step 4: Viết đường ffmpeg.** Một thể hiện `FFmpeg` dùng chung, nạp lần đầu bằng `import("@ffmpeg/ffmpeg")` rồi `load` với `toBlobURL` trỏ tới `https://cdn.jsdelivr.net/npm/@ffmpeg/core@0.12.10/dist/umd/ffmpeg-core.js` và `.wasm`; trong lúc nạp gọi `onProgress(null, "Đang tải bộ chuyển đổi (khoảng 30 MB)")`. Chạy `exec(["-i", in, ...ffmpegArgs(target), out])`, tên `in`/`out` cố định kèm đuôi đúng, xoá cả hai file ảo sau khi đọc. `resetEngines` gọi `terminate()` và bỏ thể hiện.
- [x] **Step 5: Viết đường văn bản.** Gọi `convertPureText` trước. `null` thì: `md→html` dùng `marked` bọc trong `<!doctype html><meta charset="utf-8"><title>tên file</title>`; `md→txt` là `marked` rồi `DOMParser` lấy `body.textContent`; `html→md` dùng `turndown`; `html→txt` dùng `DOMParser`; `docx→html` và `docx→txt` dùng `mammoth` bản trình duyệt (`convertToHtml`, `extractRawText` với `arrayBuffer`); `docx→md` là `docx→html` rồi `turndown`. Mỗi thư viện nạp bằng `import()` ngay tại nhánh dùng nó.
- [x] **Step 6: Viết `zipResults`** bằng `fflate` (`zipSync`, nạp động), mức nén 0 vì đa số đầu ra đã nén sẵn.
- [x] **Step 7: Kiểm.** Run: `npx tsc --noEmit && npm test && npm run build`. Expected: không lỗi. Ghi lại số "First Load JS" của `/` trong bảng của `next build` để so ở Task 4.
- [x] **Step 8: Commit.** `git add website && git commit -m "feat(website): add in-browser conversion engines"`

---

### Task 4: Giao diện nhiều file (`DropzonePlayground.tsx`)

**Files:**
- Modify: `website/src/components/DropzonePlayground.tsx` (viết lại phần thân, giữ `export default` và `id="playground"`)
- Modify: `.gitignore` (thêm `.superpowers/`)

**Interfaces:**
- Consumes: `extOf`, `groupOf`, `targetsFor`, `sharedTargets`, `outputName`, `MAX_BYTES` từ `@/lib/convert/catalog`; `convertFile`, `canEncodeAvif`, `zipResults`, `resetEngines` từ `@/lib/convert/engines`.
- Mockup đã duyệt (phương án B): `.superpowers/brainstorm/239419-1794452572/content/multi-file-layout.html`. Bám bố cục đó.

- [x] **Step 1: State và nạp file.** Input thêm `multiple`, bỏ `accept`. State: danh sách file gốc (mỗi mục: `File`, đuôi, đích đang chọn, trạng thái `idle | running | error | skipped`, tiến trình, ghi chú), danh sách kết quả (tên, blob URL, kích thước, kích thước gốc), cờ đang chạy, cờ AVIF (gọi `canEncodeAvif` một lần khi mount; `false` thì lọc `avif` khỏi mọi danh sách đích). Thả hoặc chọn file **nối thêm** vào danh sách. Khi đã có file, bấm vào vùng thả không mở hộp chọn; có liên kết "Thêm file" riêng.
- [x] **Step 2: Vẽ danh sách gốc.** Mỗi dòng: huy hiệu đuôi, tên, dung lượng. File `groupOf === null`: ghi "Chỉ có trong app" và liên kết `#tai-ve`. File lớn hơn `MAX_BYTES`: ghi "File quá lớn cho trình duyệt, hãy dùng app". Hai loại này không có dropdown và không tính vào N.
- [x] **Step 3: Hai chế độ.** `sharedTargets(đuôi của mọi file)` khác `null`: hàng nút dưới nhãn "Chuyển tất cả sang", bấm là chạy ngay; file không có đích đó trong `targetsFor` thì đánh dấu `skipped` với ghi chú "Bỏ qua: không chuyển được sang X". Bằng `null`: mỗi dòng hỗ trợ có `<select>` bên phải (mặc định là đích đầu tiên, có `aria-label` chứa tên file) và một nút "Chuyển đổi N file".
- [x] **Step 4: Chạy việc.** Lần lượt từng file, không song song. Dòng đang chạy hiện thanh tiến trình và ghi chú từ `onProgress`. Lỗi: dòng đó hiện thông báo lỗi, vòng lặp chạy tiếp. Thành công: thêm một dòng vào "Đã xong: tải về" với tên từ `outputName` (một `Set` tên đã dùng sống suốt phiên), dung lượng, "giảm X%" khi nhỏ hơn file gốc, nút tải là `<a download>`. Khi đang chạy: khoá mọi nút chuyển và dropdown. Mỗi lượt chạy giữ một số phiên trong `useRef`; kết quả về sau khi số phiên đã đổi thì bỏ và thu hồi URL.
- [x] **Step 5: Tải tất cả, làm lại, chữ.** "Tải tất cả (.zip)" hiện khi có từ 2 kết quả, tải `morphix.zip` từ `zipResults`. "Làm lại với file khác": tăng số phiên, `resetEngines()`, `URL.revokeObjectURL` mọi kết quả, xoá cả hai danh sách và `Set` tên. Đổi đoạn mô tả dưới tiêu đề thành: "Trình duyệt của bạn chuyển đổi thật ngay trên máy. File không rời khỏi thiết bị." Thêm `.superpowers/` vào `.gitignore`.
- [ ] **Step 6: Kiểm tay trên trình duyệt.** Run: `npm run dev`, mở `http://localhost:3000/#playground`, mở tab Network. Từng mục phải đúng:
  1. Thả 3 ảnh PNG/JPG, bấm WebP: 3 dòng tải về, tab Network không có request tới `cdn.jsdelivr.net`; file tải về mở được bằng trình xem ảnh.
  2. Thả `a.png` và `a.jpg`, bấm WebP: ra `a.webp` và `a (2).webp`; "Tải tất cả" cho zip chứa đủ 2 file.
  3. Thả 1 ảnh + 1 mp4 ngắn + 1 srt: hiện dropdown từng dòng; chọn WebP, MP3, VTT, bấm "Chuyển đổi 3 file": có dòng "Đang tải bộ chuyển đổi", đủ 3 kết quả, MP3 phát được.
  4. Thả một file 0 byte đổi tên thành `hong.png` cùng một ảnh thật: dòng hỏng báo lỗi, ảnh thật vẫn ra kết quả, không có dòng tải về 0 byte.
  5. Thả `tai-lieu.pdf`: dòng "Chỉ có trong app", liên kết cuộn tới mục tải về.
  6. Bấm "Làm lại với file khác" khi mp4 đang chạy: cả hai danh sách trống và không có kết quả nào hiện lại sau 30 giây.
  7. Tab Network trong suốt quá trình: không có request POST/PUT nào.
- [x] **Step 7: Kiểm bản tĩnh.** Run: `npx tsc --noEmit && npm test && npm run build && npx serve out`. Lặp lại mục 1 và 3 của bước 6 trên bản tĩnh. Expected: kết quả như trên. "First Load JS" của `/` tăng không quá 10 kB so với số ghi ở Task 3; tăng nhiều hơn nghĩa là có thư viện bị import tĩnh.
- [x] **Step 8: Commit.** `git add .gitignore website && git commit -m "feat(website): convert files for real in the playground"`
