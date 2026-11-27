# Quảng cáo trên website: kế hoạch làm

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Một ô Google AdSense trong khối thử đổi file, hiện từ lúc bấm đổi đến lúc xoá danh sách, script chỉ tải khi ô hiện lần đầu.

**Architecture:** Hàm thuần trong `website/src/lib/ads.ts` (có test), hằng số mã AdSense trong `website/src/constants/ads.ts`, component `AdSlot.tsx` tự chèn script khi mount. Trang chính sách tĩnh mới, sửa chữ FAQ/Features.

**Tech Stack:** Next.js 15 (`output: "export"`), React 19, Tailwind 4, `node --test` (Node 22, chạy thẳng `.ts`).

**Spec:** `docs/superpowers/specs/2026-11-27-morphix-website-ads-design.md`

## Global Constraints

- Không thêm dependency. Không Auto ads, không popup, không đếm ngược.
- Script AdSense không có trong HTML tĩnh: `out/index.html` không được chứa chuỗi `googlesyndication`.
- URL script: `https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=<client>`, `async`, `crossOrigin="anonymous"`.
- Chưa có mã (`client === ""`): không tải gì, hiện khung xám giữ chỗ.
- Ô quảng cáo cao tối thiểu 250 px, có nhãn "Quảng cáo".
- Chữ trên web viết tiếng Việt, cùng giọng với các file hiện có.

## Review Focus

1. React StrictMode (dev) mount 2 lần: `adsbygoogle.push` lần 2 ném lỗi "already have ads" → phải nuốt lỗi, không làm hỏng khối đổi file. (Task 2, bước kiểm tra tay)
2. Bấm "Làm lại với file khác" rồi đổi tiếp: ô mount lại, push lại, **không** chèn script lần 2. (Task 2, bước kiểm tra tay)
3. Trình chặn quảng cáo chặn script: ô vẫn giữ 250 px trống, đổi file và tải về vẫn chạy. (Task 2, bước kiểm tra tay)
4. Đang đổi (results còn rỗng) ô đã phải hiện; đổi xong vẫn hiện. (Task 1, test `shouldShowAd`)
5. Mã chưa cấu hình: không có request tới `googlesyndication`. (Task 1, test `isAdConfigured`; Task 2, kiểm tra tay)

---

### Task 1: Hàm thuần cho quảng cáo

**Files:**
- Create: `website/src/constants/ads.ts`
- Create: `website/src/lib/ads.ts`
- Test: `website/src/lib/ads.test.ts`
- Modify: `website/package.json` (script `test`)

**Interfaces:**
- Produces:
  - `export const ADSENSE: { client: string; slot: string }` (giá trị `""` cả hai; chủ dự án điền `ca-pub-…` và mã ô sau).
  - `export function shouldShowAd(isRunning: boolean, resultCount: number): boolean`
  - `export function isAdConfigured(cfg: { client: string; slot: string }): boolean` — true khi cả hai khác rỗng sau `trim()`.
  - `export function adScriptSrc(client: string): string`

- [ ] **Step 1: Viết test** `website/src/lib/ads.test.ts` (cùng kiểu `catalog.test.ts`):

```ts
test("shouldShowAd", () => {
  assert.equal(shouldShowAd(false, 0), false);
  assert.equal(shouldShowAd(true, 0), true);
  assert.equal(shouldShowAd(false, 2), true);
  assert.equal(shouldShowAd(true, 1), true);
});
test("isAdConfigured", () => {
  assert.equal(isAdConfigured({ client: "", slot: "" }), false);
  assert.equal(isAdConfigured({ client: "ca-pub-1", slot: "" }), false);
  assert.equal(isAdConfigured({ client: "  ", slot: "123" }), false);
  assert.equal(isAdConfigured({ client: "ca-pub-1", slot: "123" }), true);
});
test("adScriptSrc", () => {
  assert.equal(
    adScriptSrc("ca-pub-1"),
    "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=ca-pub-1",
  );
});
```

- [ ] **Step 2: Sửa script test** trong `website/package.json` thành `node --test "src/lib/*.test.ts" "src/lib/convert/*.test.ts"`.

- [ ] **Step 3: Chạy test, phải FAIL**: `cd website && npm test` → lỗi không tìm thấy `./ads.ts`.

- [ ] **Step 4: Viết `constants/ads.ts` và `lib/ads.ts`** theo Interfaces. `adScriptSrc` dùng `encodeURIComponent(client)`.

- [ ] **Step 5: Chạy test, phải PASS**: `cd website && npm test` → mọi test pass, kể cả test cũ.

- [ ] **Step 6: Commit**: `git add website/src/lib/ads.ts website/src/lib/ads.test.ts website/src/constants/ads.ts website/package.json && git commit -m "feat(website): add ad display helpers"`

### Task 2: Ô quảng cáo trong khối thử đổi file

**Files:**
- Create: `website/src/components/AdSlot.tsx`
- Modify: `website/src/components/DropzonePlayground.tsx` (sau khối `{/* Download Results Section */}`, trước `{/* Footer Controls */}`, ~dòng 532)
- Modify: `website/src/app/layout.tsx` (`metadata`)

**Interfaces:**
- Consumes: `ADSENSE`, `isAdConfigured`, `adScriptSrc`, `shouldShowAd` từ Task 1.
- Produces: `export default function AdSlot(): JSX.Element` (`"use client"`, không prop).

- [ ] **Step 1: Viết `AdSlot.tsx`**:
  - Khung ngoài: `min-h-[250px]`, viền nhẹ, nhãn nhỏ "Quảng cáo" phía trên.
  - Chưa cấu hình → chỉ khung xám, chữ "Chỗ đặt quảng cáo". Không `useEffect` tải gì.
  - Đã cấu hình → render `<ins className="adsbygoogle" style={{display:"block"}} data-ad-client={ADSENSE.client} data-ad-slot={ADSENSE.slot} data-ad-format="auto" data-full-width-responsive="true" />`.
  - `useEffect` khi mount: nếu chưa có `script[src="<adScriptSrc>"]` trong `document` thì tạo script (`async`, `crossOrigin="anonymous"`) gắn vào `document.head`; sau đó `try { ((window as any).adsbygoogle ||= []).push({}) } catch {}` (nuốt lỗi "already have ads" khi StrictMode mount 2 lần).

- [ ] **Step 2: Gắn vào playground**: `{shouldShowAd(isRunning, results.length) && <div className="pt-4"><AdSlot /></div>}`.

- [ ] **Step 3: Meta xác minh**: trong `layout.tsx` thêm `other: isAdConfigured(ADSENSE) ? { "google-adsense-account": ADSENSE.client } : undefined` vào `metadata`.

- [ ] **Step 4: Build**: `cd website && npm run build` → thành công; `grep -c googlesyndication out/index.html` → `0`.

- [ ] **Step 5: Kiểm tra tay chưa có mã** (`npm run dev`, DevTools tab Network, lọc `googlesyndication`):
  - Mở trang: 0 request, không thấy ô.
  - Thả 1 ảnh, bấm đổi: khung xám "Quảng cáo" hiện, vẫn 0 request.
  - Bấm "Làm lại với file khác": ô biến mất.

- [ ] **Step 6: Kiểm tra tay với mã giả** (tạm đặt `client: "ca-pub-0000000000000000", slot: "0000000000"`, **không commit**):
  - Bấm đổi lần 1: đúng 1 request `adsbygoogle.js`; console không có lỗi chưa bắt.
  - Làm lại rồi đổi lần 2: `document.querySelectorAll('script[src*="adsbygoogle"]').length` vẫn là `1`.
  - Bật trình chặn quảng cáo (hoặc chặn URL trong DevTools): ô vẫn cao 250 px, nút "Tải về" vẫn tải được file.
  - Trả `ADSENSE` về `""`.

- [ ] **Step 7: Commit**: `git add website/src/components/AdSlot.tsx website/src/components/DropzonePlayground.tsx website/src/app/layout.tsx && git commit -m "feat(website): show an ad slot while converting in the playground"`

### Task 3: Trang chính sách và sửa chữ

**Files:**
- Create: `website/src/app/chinh-sach-rieng-tu/page.tsx`
- Modify: `website/src/components/Footer.tsx` (cột "Hỗ trợ & Đóng góp", thêm `<li>` cuối)
- Modify: `website/src/components/FaqAccordion.tsx:29-31`
- Modify: `website/src/components/FeaturesZPattern.tsx:116`

- [ ] **Step 1: Trang `/chinh-sach-rieng-tu`**: `export const metadata = { title: "Chính sách quyền riêng tư — Morphix" }`; trang đơn giản, không dùng Header/Footer (các link `#...` của chúng chỉ đúng ở trang chủ), có link "← Về trang chủ" tới `/`. Nội dung (5 ý):
  1. Ứng dụng Morphix (Windows, Linux) không thu thập dữ liệu, không có mã theo dõi, không gửi file đi đâu.
  2. Khối thử đổi file trên website xử lý file ngay trong trình duyệt; file không được tải lên máy chủ nào.
  3. Website hiển thị quảng cáo của Google AdSense. Google và đối tác có thể dùng cookie để hiển thị quảng cáo dựa trên các lần bạn truy cập website này và website khác. Mã quảng cáo chỉ được tải khi bạn bấm chuyển đổi file.
  4. Tìm hiểu cách Google dùng dữ liệu: `https://policies.google.com/technologies/partner-sites`. Tắt quảng cáo cá nhân hoá: `https://myadcenter.google.com`.
  5. Liên hệ: link Issue tracker `${DOWNLOADS.githubRepo}/issues`.

- [ ] **Step 2: Link footer**: `<a href="/chinh-sach-rieng-tu" className="hover:text-white transition-colors">Chính sách quyền riêng tư</a>` (không `target="_blank"`).

- [ ] **Step 3: Sửa FAQ** (`FaqAccordion.tsx:29`): đổi "Morphix không chứa" thành "Ứng dụng Morphix không chứa"; thêm câu cuối: "Riêng website này có hiển thị quảng cáo của Google, xem <a href=\"/chinh-sach-rieng-tu\">Chính sách quyền riêng tư</a>." (link cùng kiểu class gạch chân teal như các link khác trong file).

- [ ] **Step 4: Sửa Features** (`FeaturesZPattern.tsx:116`): "Ứng dụng không thu thập thông tin, không gắn mã theo dõi (analytics)".

- [ ] **Step 5: Build**: `cd website && npm run build` → thành công; `ls out/chinh-sach-rieng-tu.html` có file; `grep -c googlesyndication out/index.html out/chinh-sach-rieng-tu.html` → cả hai `0`.

- [ ] **Step 6: Commit**: `git add website/src/app/chinh-sach-rieng-tu website/src/components/Footer.tsx website/src/components/FaqAccordion.tsx website/src/components/FeaturesZPattern.tsx && git commit -m "feat(website): add privacy policy page and clarify app-only tracking promise"`
