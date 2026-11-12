# Morphix: Xây dựng Website giới thiệu & Tải về bằng Next.js (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dựng website chính thức cho Morphix bằng Next.js 15 (App Router, TypeScript, Tailwind CSS), triển khai chuẩn Phương án A (FxSound-style), tích hợp đủ 4 link tải v0.1.4, chuẩn hóa hành vi nút bấm (Windows tải trực tiếp, Linux cuộn mượt), và giữ trọn bộ các hiệu ứng tương tác cao cấp.

**Architecture:** Ứng dụng Next.js tĩnh (Static Site Generation / `output: 'export'`) chia module rõ ràng: Server Components cho phần tĩnh (SEO, metadata, layout, bảng so sánh) và Client Components cho tương tác (Hero Simulator chuột phải, Live Dropzone, Calculator, CLI Terminal, FAQ Accordion).

**Tech Stack:** Next.js 15, React 19, TypeScript 5, Tailwind CSS v4, Lucide React (`lucide-react`).

**Spec:** `docs/superpowers/specs/2026-11-12-morphix-nextjs-website-design.md`

## Global Constraints

- Xây dựng trong thư mục `website/` (thay thế bản HTML tạm thời).
- Không làm gián đoạn mã nguồn Rust hay Tauri trong `crates/`.
- Sau mỗi task, lệnh build phải vượt qua kiểm tra: `npm run build` (hoặc `npx tsc --noEmit`) không có lỗi type hay lint.
- Đúng 4 link tải phát hành v0.1.4 đã định trong Spec, không tự ý sửa đổi URL.
- Nút "Tải cho Windows (exe)": trỏ trực tiếp tải tệp `MorphixSetup.exe`.
- Nút "Cài đặt cho Linux": gọi hàm cuộn mượt xuống phần tử `#tai-ve-linux` và kích hoạt hiệu ứng chớp sáng viền.

## Review Focus

1. **Hành vi tải về & điều hướng:** Bấm nút Windows phải kích hoạt tải file `MorphixSetup.exe`; bấm nút Linux phải cuộn đúng tâm màn hình vào khối `#tai-ve-linux`.
2. **Trải nghiệm tương tác Client:** Khung giả lập chuột phải, khu vực kéo thả dropzone, thanh trượt tính dung lượng và terminal CLI phải chạy trơn tru không lỗi hydrate.
3. **Responsive & Mobile Drawer:** Kiểm tra hiển thị sạch đẹp từ mobile (375px) đến desktop (1920px), menu mobile đóng/mở mượt mà.
4. **Static Export:** Lệnh `npm run build` phải xuất ra thư mục tĩnh `out/` sẵn sàng deploy lên GitHub Pages hoặc Vercel.

---

## Các Cột mốc & Nhiệm vụ

### Task 1: Khởi tạo dự án Next.js 15 trong `website/`

**Files:**
- Create: `website/package.json`
- Create: `website/tsconfig.json`
- Create: `website/next.config.ts`
- Create: `website/src/app/layout.tsx`
- Create: `website/src/app/globals.css`
- Create: `website/src/constants/downloads.ts`

- [x] **Step 1: Dọn dẹp thư mục `website/` và tạo `package.json`** với Next.js 15, React 19, `lucide-react`, Tailwind CSS v4.
- [x] **Step 2: Cài đặt dependencies** bằng `npm install` trong thư mục `website/`.
- [x] **Step 3: Cấu hình `next.config.ts`** với `output: 'export'` và `images: { unoptimized: true }`.
- [x] **Step 4: Tạo `src/constants/downloads.ts`** định nghĩa 4 đường dẫn tải v0.1.4 và lệnh curl Linux.
- [x] **Step 5: Tạo `src/app/globals.css`** cấu hình Tailwind CSS, biến CSS token màu brand `#0f766e`, animation shimmer và ambient glow.
- [x] **Step 6: Tạo `src/app/layout.tsx`** thiết lập font *Plus Jakarta Sans*, *JetBrains Mono* và SEO Metadata chuẩn.
- [x] **Step 7: Kiểm thử:** Chạy `npm run build` trong `website/` xác nhận build thành công.

---

### Task 2: Xây dựng Header & Hero Component (với nút Windows tải trực tiếp & Linux cuộn mượt)

**Files:**
- Create: `website/src/components/Header.tsx`
- Create: `website/src/components/Hero.tsx`
- Create: `website/src/components/HeroSimulator.tsx`

- [x] **Step 1: Tạo `Header.tsx`** (sticky frosted glass, logo Morphix, menu desktop và mobile drawer toggle).
- [x] **Step 2: Tạo `HeroSimulator.tsx`** (`"use client"`) mô phỏng menu chuột phải tương tác với 3 tab file (`Anh.png`, `Video.mov`, `BaoCao.docx`), thanh progress bar động và thông báo toast kết quả.
- [x] **Step 3: Tạo `Hero.tsx`** (`"use client"`):
  - Nút **"Tải cho Windows (exe) · v0.1.4"**: thẻ `<a>` tải file exe trực tiếp kèm hiệu ứng shimmer.
  - Nút **"Cài đặt cho Linux"**: thẻ `<button>` gắn sự kiện `onClick` gọi `document.getElementById('tai-ve-linux')?.scrollIntoView({ behavior: 'smooth', block: 'center' })` và chớp sáng viền thẻ Linux.
  - Logic tự động nhận diện hệ điều hành (Smart OS Detection) từ `navigator.userAgent`.
- [x] **Step 4: Kiểm thử:** Ghép Header & Hero vào `src/app/page.tsx`, kiểm tra cuộn mượt và tải tệp.

---

### Task 3: Xây dựng Marquee, Trust Metrics, Z-Pattern Features & Format Matrix

**Files:**
- Create: `website/src/components/MarqueeTicker.tsx`
- Create: `website/src/components/TrustMetrics.tsx`
- Create: `website/src/components/FeaturesZPattern.tsx`
- Create: `website/src/components/FormatFilterGrid.tsx`

- [x] **Step 1: Tạo `MarqueeTicker.tsx`** dải chạy chữ vô tận hiển thị các cặp định dạng (`PNG sang WebP`, `MP4 sang MP3`, `PDF sang Markdown`...).
- [x] **Step 2: Tạo `TrustMetrics.tsx`** 4 chỉ số (0 Byte Cloud, 30+ Định dạng, <15ms Rust, GPL-3.0).
- [x] **Step 3: Tạo `FeaturesZPattern.tsx`** 4 khối tính năng xen kẽ giải thích cơ chế chuột phải, bảo mật offline, kho định dạng và động cơ Rust.
- [x] **Step 4: Tạo `FormatFilterGrid.tsx`** (`"use client"`) danh mục hơn 30 định dạng kèm các nút bấm lọc nhóm tương tác (*Tất cả*, *Hình ảnh*, *Video*, *Văn bản*, *Nén*).
- [x] **Step 5: Kiểm thử:** Xác nhận bộ lọc hoạt động không reload trang và layout responsive chuẩn.

---

### Task 4: Xây dựng Live Dropzone, Storage Calculator, CLI Doctor & FAQ

**Files:**
- Create: `website/src/components/DropzonePlayground.tsx`
- Create: `website/src/components/StorageCalculator.tsx`
- Create: `website/src/components/CliDoctorTerminal.tsx`
- Create: `website/src/components/ComparisonTable.tsx`
- Create: `website/src/components/FaqAccordion.tsx`

- [x] **Step 1: Tạo `DropzonePlayground.tsx`** (`"use client"`) hỗ trợ kéo thả file thật từ máy tính để mô phỏng tác vụ on-device tức thì.
- [x] **Step 2: Tạo `StorageCalculator.tsx`** (`"use client"`) thanh trượt tính dung lượng tiết kiệm (1 GB - 100 GB) nhảy số thời gian thực.
- [x] **Step 3: Tạo `CliDoctorTerminal.tsx`** (`"use client"`) cửa sổ terminal với 3 tab lệnh (`morphix doctor`, `convert --to webp`, `extract --audio`).
- [x] **Step 4: Tạo `ComparisonTable.tsx`** bảng so sánh 5 tiêu chí Morphix (On-device) vs Web Online (CloudConvert, Convertio).
- [x] **Step 5: Tạo `FaqAccordion.tsx`** (`"use client"`) danh sách câu hỏi thường gặp có hiệu ứng gập/mở mượt mà.

---

### Task 5: Xây dựng Download Matrix Hub & Footer

**Files:**
- Create: `website/src/components/DownloadMatrix.tsx`
- Create: `website/src/components/Footer.tsx`
- Modify: `website/src/app/page.tsx`

- [x] **Step 1: Tạo `DownloadMatrix.tsx`** (`"use client"`):
  - Cột Windows: nút tải trực tiếp `MorphixSetup.exe`, lưu ý SmartScreen và link SHA-256.
  - Cột Linux: gắn định danh `id="tai-ve-linux"`, khối lệnh `curl` kèm nút copy 1-click, các nút tải gói `.deb` và `.tar.gz`.
- [x] **Step 2: Tạo `Footer.tsx`** chân trang với liên kết điều hướng, giấy phép GPL-3.0 và GitHub repo.
- [x] **Step 3: Hoàn thiện `src/app/page.tsx`** ghép toàn bộ 13 component theo đúng trình tự phân cấp của Phương án A.
- [x] **Step 4: Kiểm thử liên kết:** Bấm nút Linux ở Hero kiểm tra trình duyệt cuộn mượt đến đúng vị trí `#tai-ve-linux`.

---

### Task 6: Kiểm thử toàn diện & Xuất bản Static Build (`out/`)

- [x] **Step 1: Chạy type check:** `npm run build` trong `website/`.
- [x] **Step 2: Kiểm tra thư mục xuất bản:** Xác nhận thư mục `website/out/` được sinh ra với đầy đủ HTML, CSS, JS tĩnh.
- [x] **Step 3: Chạy máy chủ xem thử:** Khởi động web server nội bộ trên thư mục `website/out/` (cổng `5180`).
- [x] **Step 4: Kiểm thử chức năng trên trình duyệt:**
  - Kiểm tra nút Windows tải file exe v0.1.4.
  - Kiểm tra nút Linux cuộn mượt và highlight viền.
  - Kiểm tra bộ lọc định dạng, dropzone, slider dung lượng, CLI terminal.
  - Kiểm tra responsive trên mobile (375px) và desktop (1280px+).
