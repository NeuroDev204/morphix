# Quảng cáo trên website: thiết kế (bản ngắn)

Ngày: 2026-11-27. Phạm vi: `website/` (Next.js 15, `output: "export"`, deploy Vercel tại `https://morphix.neuro.io.vn`).

## Ý định

Chủ dự án muốn có thu nhập từ website bằng quảng cáo. Lời hứa "không theo dõi" chỉ áp dụng cho **app**,
không áp dụng cho website.

## Mục tiêu

1. Một ô quảng cáo Google AdSense trong khối "thử đổi file" (`DropzonePlayground.tsx`).
2. Ô hiện khi người dùng bấm đổi file (`isRunning` thành true) và **ở lại** đến khi danh sách bị xoá
   (`setResults([])`, dòng ~132). Hiện cả với file đổi nhanh dưới 1 giây.
3. Chữ trên web nói rõ **app** không theo dõi, website có quảng cáo.

## Ngoài phạm vi / không làm

- Auto ads, quảng cáo bật lên, banner rải khắp trang.
- Bắt chờ hay đếm ngược trước khi tải file.
- Thư viện React quảng cáo (khoảng 20 dòng tự viết là đủ).
- Thay đổi app hay crate Rust.

## Điều làm hỏng tính năng dù chạy được

- Trang chậm đi cho người chỉ xem: script AdSense **không** được tải khi mở trang.
- Quảng cáo che hoặc đẩy lệch nút tải, danh sách kết quả.
- Vi phạm chính sách AdSense (tự làm mới quảng cáo, đặt để bấm nhầm).

## Cách làm đã chọn

| Cách | Đánh đổi | Chọn |
|---|---|---|
| A. Tải script AdSense chỉ khi ô quảng cáo hiện lần đầu | Trang mở nhanh như cũ; khách không đổi file thì không bị đặt cookie | **Chủ dự án chọn** |
| B. Gắn script trong `layout.tsx` cho mọi trang | Dễ hơn chút, nhưng trang nặng thêm 100–300 KB và mọi khách bị đặt cookie | Bỏ |

Thời điểm hiện ô: chủ dự án chọn "hiện lúc bấm đổi, giữ đến khi xoá danh sách" thay vì "chỉ lúc đang đổi"
(file nhanh gần như 0 đồng) hay "bắt chờ 5 giây" (người dùng ghét, rủi ro chính sách).

## Thiết kế

1. **`website/src/components/AdSlot.tsx`** (mới): lần đầu mount thì chèn
   `<script async src="https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=<PUB_ID>" crossorigin="anonymous">`
   nếu chưa có, render `<ins class="adsbygoogle" data-ad-client data-ad-slot ...>`, rồi
   `(window.adsbygoogle ||= []).push({})` một lần cho mỗi lần mount. Có nhãn nhỏ "Quảng cáo".
   Khi chưa có mã publisher (dev): hiện khung xám giữ chỗ, không tải gì.
2. **`DropzonePlayground.tsx`**: render `<AdSlot />` dưới danh sách kết quả khi `isRunning || results.length > 0`.
   Ô có chiều cao cố định để không làm nhảy bố cục.
3. **`layout.tsx`**: thêm `metadata.other = { "google-adsense-account": "<PUB_ID>" }` để Google xác minh web
   mà không cần script trên trang.
4. **Trang `/chinh-sach-rieng-tu`** (`website/src/app/chinh-sach-rieng-tu/page.tsx`): nói rõ website dùng
   Google AdSense và cookie, app thì không; link tới chính sách quảng cáo của Google. Thêm link ở `Footer.tsx`.
5. **Sửa chữ**: `FaqAccordion.tsx:29` và `FeaturesZPattern.tsx:116` nói rõ là **app** không theo dõi.
   Câu "không gửi 1 byte" ở `Hero.tsx:60` giữ nguyên (đúng cả với web: file không rời máy).
6. Mã publisher (`ca-pub-…`) và mã ô (`data-ad-slot`) để thành hằng số ở một chỗ (cạnh `DOWNLOADS`).

## Việc chủ dự án tự làm (ngoài repo)

- Đăng ký AdSense với tên miền gốc `neuro.io.vn` (AdSense không nhận riêng tên miền con).
- Thêm `ads.txt` vào project Vercel của `neuro.io.vn` (hiện `/ads.txt` trả 404).
- Bật thông báo đồng ý cookie có sẵn của Google trong AdSense (Quyền riêng tư & thông báo) cho khách châu Âu.
  *Gợi ý của Claude: không tự viết banner cookie.*

## Để lại cho người làm

- Kích thước ô (gợi ý: responsive, cao tối thiểu 250 px).
- Kiểm tra: `npm run build` qua; mở trang thấy không có request tới `googlesyndication`
  cho đến khi bấm đổi file.

---

## Sửa đổi 2026-11-27: chuyển sang Adsterra (Social Bar + Popunder)

Chủ dự án bỏ Google AdSense, dùng Adsterra với 2 loại **Social Bar** và **Popunder**, không quảng cáo 18+.
Phần AdSense ở trên **không còn hiệu lực**; phần dưới đây thay thế nó.

**Chủ dự án chọn:** tải 2 script **ngay khi mở trang**, trên mọi trang (thay vì chỉ sau khi đổi file):
nhiều tiền nhất, chấp nhận ấn tượng đầu kém hơn.

**Cách làm (một cách hợp lý):** `next/script` với `strategy="afterInteractive"` trong `layout.tsx`.
Bỏ: lazy-load (trượt cú bấm đầu, nơi popunder ăn tiền); script thô trong `<head>` (chặn lần vẽ đầu).

**Thiết kế:**
1. Xoá phần banner: `AdSlot.tsx`, `lib/ads.ts`, `lib/ads.test.ts`, ô quảng cáo trong `DropzonePlayground.tsx`,
   meta `google-adsense-account`; trả script `test` trong `package.json` về như cũ.
2. `constants/ads.ts`: `ADSTERRA = { socialBar: "", popunder: "" }` (URL script). Rỗng → không render script đó.
3. `layout.tsx`: mỗi URL khác rỗng → `<Script src strategy="afterInteractive" />`.
4. Trang chính sách: thay đoạn Google bằng Adsterra (thanh thông báo nổi, tab quảng cáo mở khi bấm, cookie,
   link https://adsterra.com/privacy-policy/). FAQ: "quảng cáo của Google" → "quảng cáo".
5. Giữ: các câu "ứng dụng không quảng cáo", link chính sách ở footer.

**Không làm:** quảng cáo 18+, Smartlink, Native Banner, banner cookie tự viết, `ads.txt`.

**Kiểm tra:** build với URL giả → `out/index.html` chứa cả 2 URL; URL rỗng → không chứa.
