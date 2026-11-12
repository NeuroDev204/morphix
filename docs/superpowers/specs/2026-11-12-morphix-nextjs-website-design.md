# Morphix: Website Giới thiệu & Tải về sử dụng Next.js (Design Spec)

Status: Chờ duyệt thực thi (2026-11-12). Kế hoạch thực thi tại `docs/superpowers/plans/2026-11-12-morphix-nextjs-website.md`.

Ký hiệu: **[ND]** là điều người dùng đã yêu cầu. **[TK]** là lựa chọn kỹ thuật để đáp ứng yêu cầu.

---

## 1. Mục tiêu & Phạm vi

### Mục tiêu [ND]
- Xây dựng website chính thức cho Morphix dựa trên wireframe Phương án A đã chọn (mẫu tham chiếu: FxSound) [ND].
- Sử dụng framework **Next.js** (App Router, React 19, TypeScript, Tailwind CSS) [ND].
- Quy định hành vi nút bấm tải về ở Hero [ND]:
  - Bấm **"Tải cho Windows"**: tải trực tiếp tệp `MorphixSetup.exe` (v0.1.4).
  - Bấm **"Cài đặt cho Linux"**: cuộn mượt (smooth scroll) xuống khu vực cài đặt Linux và làm sáng nổi bật khu vực này.
- Đầy đủ 4 tùy chọn tải về phiên bản **v0.1.4** [ND]:
  1. Windows: `https://github.com/NeuroDev204/morphix/releases/download/v0.1.4/MorphixSetup.exe`
  2. Linux, .deb: `https://github.com/NeuroDev204/morphix/releases/download/v0.1.4/morphix_0.1.4-1_amd64.deb`
  3. Linux, tarball: `https://github.com/NeuroDev204/morphix/releases/download/v0.1.4/morphix-x86_64-linux.tar.gz`
  4. Lệnh curl tự động: `curl -fsSL https://github.com/NeuroDev204/morphix/releases/latest/download/install.sh | sh`
- Giữ trọn bộ các khối hiệu ứng tương tác cao cấp: Hero Simulator chuột phải, Live Dropzone thử file thật, Storage Calculator ước tính dung lượng, CLI Doctor và FAQ Accordion.

### Ngoài phạm vi
- Chưa kết nối backend cơ sở dữ liệu (website thuần frontend tĩnh / SSG để deploy miễn phí lên GitHub Pages hoặc Vercel).
- Không thêm các đường dẫn tải về ngoài v0.1.4 đã định.

---

## 2. Kiến trúc & Công nghệ [TK]

- **Framework:** Next.js 15+ (App Router).
- **Ngôn ngữ:** TypeScript 5, quy chuẩn kiểu nghiêm ngặt (`strict: true`).
- **Styling:** Tailwind CSS v4, đồng bộ bảng màu brand `#0f766e` từ `crates/morphix-gui/ui/src/styles.css`.
- **Icon:** `lucide-react`.
- **Phân tách Server & Client Components:**
  - *Server Components (RSC):* Layout, SEO Metadata, Header tĩnh, Footer, Bảng so sánh (tối ưu index Google).
  - *Client Components (`"use client"`):* HeroSimulator, DropzonePlayground, StorageCalculator, CliDoctorTerminal, FAQAccordion, Nút cuộn Linux.
- **Static Export:** Cấu hình `output: 'export'` trong `next.config.ts` để sinh thư mục `out/` tĩnh.

---

## 3. Đặc tả Thành phần & Tương tác

### 3.1. Hero CTA Behavior [ND]
- Nút Windows:
  - Thẻ `<a>` trỏ thẳng tới `MorphixSetup.exe`.
  - Hiệu ứng shimmer quét sáng kim loại (`btn-shimmer`).
- Nút Linux:
  - Thẻ `<button>` với sự kiện `onClick`.
  - Kích hoạt `document.getElementById('tai-ve-linux')?.scrollIntoView({ behavior: 'smooth', block: 'center' })`.
  - Thêm class `ring-4 ring-teal-400 scale-[1.02]` vào `#card-linux` trong 1.8 giây để người dùng định vị ngay lệnh curl.

### 3.2. Client-side OS Detection [TK]
- Đọc `navigator.userAgent` ở client:
  - Nếu là Windows: hiển thị badge "Phát hiện Windows", viền xanh thẻ Windows.
  - Nếu là Linux: hiển thị badge "Phát hiện Linux", viền xanh thẻ Linux.

---

## 4. Danh sách Components chính

1. `Header`: Thanh điều hướng mờ (frosted glass) ghim đỉnh trang.
2. `Hero`: Tiêu đề, nút tải Windows, nút cuộn Linux, OS detection banner.
3. `HeroSimulator`: Client component mô phỏng chuột phải với 3 file mẫu (`Anh.png`, `Video.mov`, `BaoCao.docx`).
4. `MarqueeTicker`: Dải chữ chạy vô tận các định dạng (`PNG sang WebP`, `MP4 sang MP3`, `PDF sang Markdown`...).
5. `TrustMetrics`: 4 số liệu cốt lõi (0 Byte Cloud, 30+ Định dạng, <15ms Rust, GPL-3.0).
6. `FeaturesZPattern`: 4 khối tính năng xen kẽ giải thích chuột phải, bảo mật offline, kho định dạng, động cơ Rust.
7. `DropzonePlayground`: Kéo thả file thật từ máy tính để mô phỏng tức thì.
8. `StorageCalculator`: Thanh trượt tính dung lượng tiết kiệm (1GB - 100GB).
9. `CliDoctorTerminal`: Cửa sổ dòng lệnh mô phỏng 3 tab lệnh (`morphix doctor`, `convert`, `extract`).
10. `ComparisonTable`: Bảng so sánh Morphix vs Web Online (CloudConvert, Convertio).
11. `DownloadMatrix`: Khối tải về Windows và khối Linux (`id="tai-ve-linux"`).
12. `FaqAccordion`: Danh sách câu hỏi thường gặp.
13. `Footer`: Thông tin bản quyền GPL-3.0 và link GitHub.
