# Adsterra Social Bar + Popunder: kế hoạch làm

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thay banner AdSense bằng 2 script Adsterra (Social Bar, Popunder) tải ngay khi mở mọi trang.

**Architecture:** Xoá toàn bộ phần AdSense đã làm trên nhánh; `layout.tsx` render `next/script` cho mỗi URL khác rỗng trong `ADSTERRA`.

**Tech Stack:** Next.js 15 (`output: "export"`), `next/script`.

**Spec:** `docs/superpowers/specs/2026-11-27-morphix-website-ads-design.md` (mục "Sửa đổi 2026-11-27: chuyển sang Adsterra")

## Global Constraints

- `strategy="afterInteractive"` cho cả 2 script. URL rỗng → không render.
- Không quảng cáo 18+, Smartlink, Native Banner, banner cookie, `ads.txt`.
- Không còn chuỗi `googlesyndication` hay `google-adsense-account` trong `website/src`.

## Review Focus

1. URL rỗng (chưa dán mã): không có thẻ script lạ nào trong HTML. (Task 1, bước 4)
2. Popunder ăn cú bấm đầu: khối thả file vẫn mở hộp chọn file và đổi/tải file vẫn chạy khi script bị chặn hoặc lỗi. (Task 1, bước 5)
3. Trang chính sách cũng có 2 script (mọi trang), và nói đúng hành vi popunder. (Task 1 bước 4, Task 2)

---

### Task 1: Gỡ AdSense, gắn script Adsterra

**Files:**
- Delete: `website/src/components/AdSlot.tsx`, `website/src/lib/ads.ts`, `website/src/lib/ads.test.ts`
- Modify: `website/src/components/DropzonePlayground.tsx` (bỏ 2 import và khối `shouldShowAd(...) && <AdSlot />`)
- Modify: `website/package.json` (script `test` về `node --test "src/lib/convert/*.test.ts"`)
- Modify: `website/src/constants/ads.ts` → `export const ADSTERRA = { socialBar: "", popunder: "" }`
- Modify: `website/src/app/layout.tsx` (bỏ meta `other`, bỏ import `ADSENSE`; thêm script)

- [ ] **Step 1: Kiểm tra đỏ**: tạm đặt URL giả `https://example.invalid/sb.js` và `https://example.invalid/pu.js` (không commit), `npm run build`, `grep -c example.invalid out/index.html` → `0` (chưa có code gắn).
- [ ] **Step 2: Gỡ AdSense và gắn script** theo Files. Trong `<body>` sau `{children}`: `{ADSTERRA.socialBar && <Script src={ADSTERRA.socialBar} strategy="afterInteractive" />}`, tương tự `popunder`.
- [ ] **Step 3: Kiểm tra xanh**: build lại với URL giả → `out/index.html` và `out/chinh-sach-rieng-tu.html` đều chứa cả `sb.js` và `pu.js`.
- [ ] **Step 4: URL rỗng**: trả `ADSTERRA` về `""`, build → `grep -cE "example.invalid|googlesyndication|google-adsense-account" out/index.html` → `0`; `npm test` pass; `rg -n "googlesyndication|AdSlot|shouldShowAd" src` → không có kết quả.
- [ ] **Step 5: Khi script hỏng**: với URL giả (trỏ tới host không tồn tại), `next dev` + Playwright: thả 1 PNG, đổi sang JPG, bấm "Tải về" → tải được `a.jpg`, không có pageerror.
- [ ] **Step 6: Commit** `feat(website): switch ads to Adsterra social bar and popunder`.

### Task 2: Sửa chữ chính sách và FAQ

**Files:**
- Modify: `website/src/app/chinh-sach-rieng-tu/page.tsx` (mục "Quảng cáo trên website")
- Modify: `website/src/components/FaqAccordion.tsx` ("quảng cáo của Google" → "quảng cáo")

- [ ] **Step 1: Kiểm tra đỏ**: `rg -n "Google" src/app/chinh-sach-rieng-tu/page.tsx src/components/FaqAccordion.tsx` → có kết quả.
- [ ] **Step 2: Viết lại mục quảng cáo**: "Website này hiển thị quảng cáo của Adsterra: một thanh thông báo nổi ở góc màn hình, và một tab quảng cáo có thể mở ra khi bạn bấm vào trang. Adsterra và các đối tác có thể dùng cookie để hiển thị quảng cáo." + link `https://adsterra.com/privacy-policy/`. Bỏ 2 link Google.
- [ ] **Step 3: Kiểm tra xanh**: lệnh `rg` ở bước 1 → không có kết quả; `npm run build` qua.
- [ ] **Step 4: Commit** `docs(website): describe Adsterra ads in the privacy policy`.
