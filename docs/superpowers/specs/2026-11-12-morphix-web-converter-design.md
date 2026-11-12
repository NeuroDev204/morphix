# Chuyển đổi file thật ngay trên website: thiết kế

Ngày: 2026-11-12. Phạm vi: `website/` (Next.js 15, `output: "export"`, bản tĩnh).

## Ý định

Khối "Phòng thí nghiệm tương tác" (`website/src/components/DropzonePlayground.tsx`, `id="playground"`)
hiện chỉ giả lập: thả 1 file, bấm nút, hiện thông báo "đã mô phỏng". Chủ dự án muốn khối này
**chuyển đổi thật trong trình duyệt và cho tải file kết quả về**, để khách thấy Morphix làm được
việc trước khi cài app.

Luồng do chủ dự án mô tả: kéo thả file vào → bấm định dạng → danh sách tải về.

## Mục tiêu

1. Thả hoặc chọn **nhiều file cùng lúc**.
2. Chuyển đổi thật cho ảnh, âm thanh, video, văn bản. Không gửi file lên server nào.
3. Kết quả hiện thành danh sách tải về, tải từng file hoặc tất cả.

## Ngoài phạm vi

- PDF và 7Z/ZIP làm đầu vào (gộp ảnh thành PDF, PDF sang text, nén, giải nén).
- Các việc khác của app: nén, đổi kích thước, xoá metadata, chọn chất lượng.
- Ghi ra DOCX (Markdown sang DOCX, HTML sang DOCX).
- Thay đổi bất kỳ crate Rust nào. Không biên dịch `morphix-core` sang WASM.

## Điều làm hỏng tính năng dù chạy được

- File của người dùng rời khỏi máy. Dòng "Không tải lên server" phải luôn đúng.
- Trang chủ nặng thêm cho người không dùng khối này: ffmpeg không được tải khi mở trang.
- Nút bấm rồi không ra file, hoặc ra file hỏng mà vẫn báo thành công.

## Ràng buộc

- Web là bản tĩnh, chưa rõ nơi host, nên **không giả định đặt được header COOP/COEP**. Dùng ffmpeg
  bản một luồng (không cần `SharedArrayBuffer`).
- Giữ kiểu dáng Tailwind hiện có của khối (tông teal, bo góc, cỡ chữ).
- Chữ trên giao diện bằng tiếng Việt.

## Hành vi (chủ dự án đã quyết)

### Sau khi thả file

Mỗi file là một dòng: huy hiệu đuôi file, tên, dung lượng.

- **Các file cùng nhóm** (ví dụ toàn ảnh): bên dưới danh sách có một hàng nút định dạng
  "Chuyển tất cả sang". Bấm một nút là chuyển ngay tất cả file sang định dạng đó.
- **Các file khác nhóm**: mỗi dòng có một dropdown **bên phải** để chọn định dạng đích riêng, và
  một nút "Chuyển đổi N file" ở dưới.

### Kết quả (phương án B trong mockup)

- File gốc **giữ nguyên** ở danh sách trên. Người dùng đổi định dạng và chuyển lại được.
- Bên dưới có danh sách riêng "Đã xong: tải về". Mỗi lần chuyển xong thêm một dòng: tên file mới,
  dung lượng, nút "Tải về".
- Mockup đã duyệt: `.superpowers/brainstorm/239419-1794452572/content/multi-file-layout.html`.

## Các lựa chọn của người viết thiết kế (chủ dự án chưa quyết, hãy xem lại)

1. **Nhóm và định dạng đích.** Nhóm lấy theo đuôi file, theo đúng cách chia của app
   (`crates/morphix-core/src/catalog.rs`: `text_group`, `media_actions`):

   | Nhóm | Đầu vào | Đích |
   |---|---|---|
   | Ảnh | jpg, png, webp, avif, gif, tiff, bmp | JPG, PNG, WebP, GIF, TIFF, BMP; AVIF chỉ khi trình duyệt mã hoá được |
   | Âm thanh | mp3, m4a, wav, flac, ogg, opus, aiff, wma | như `AUDIO_TARGETS` của app, bỏ đích ffmpeg bản web không mã hoá được |
   | Video | mp4, mov, mkv, webm, avi, wmv | như `VIDEO_TARGETS` của app (MP4, MKV, WebM, MOV, AVI, GIF, MP3) |
   | txt, md | | TXT, Markdown, HTML (từ md) |
   | srt, vtt | | SRT, VTT, TXT |
   | csv, tsv | | CSV, TSV, Markdown |
   | html, docx | | Markdown, HTML, TXT |

   Đích trùng với định dạng gốc của file thì không hiện trong dropdown; ở chế độ nút chung, file
   đã ở định dạng đó bị bỏ qua kèm ghi chú.
2. **Bộ máy.** JPG, PNG, WebP (và AVIF nếu có) dùng `createImageBitmap` + canvas, không thêm thư
   viện. Âm thanh, video, GIF, TIFF, BMP dùng `@ffmpeg/ffmpeg` bản một luồng, **chỉ tải khi có
   việc cần đến nó lần đầu** (khoảng 30 MB), có dòng trạng thái "Đang tải bộ chuyển đổi".
   Lõi ffmpeg lấy từ CDN với phiên bản ghim cứng; đây là tải mã về, không gửi file đi.
3. **File không hỗ trợ** (pdf, zip, 7z, đuôi lạ): vẫn hiện thành dòng, ghi "Chỉ có trong app" kèm
   liên kết tới `#tai-ve`, không có dropdown, không tính vào N.
4. **Giới hạn 200 MB mỗi file.** Lớn hơn thì dòng đó báo "File quá lớn cho trình duyệt, hãy dùng
   app" và không chuyển.
5. **Chạy lần lượt từng file**, không song song. Mỗi file đang chạy có thanh tiến trình.
6. **Lỗi theo từng file.** Một file hỏng thì dòng của nó hiện lỗi, các file còn lại vẫn chạy. Không
   bao giờ thêm dòng tải về cho file lỗi hoặc rỗng (0 byte).
7. **"Tải tất cả (.zip)"** hiện khi có từ 2 kết quả trở lên.
8. **Dòng kết quả ghi mức giảm dung lượng** ("giảm 78%") khi file mới nhỏ hơn file gốc.
9. **"Thêm file"** để bổ sung vào danh sách, **"Làm lại với file khác"** xoá hết, dừng ffmpeg
   đang chạy và thu hồi các object URL.
10. **Sửa câu mô tả** của khối: bỏ chữ "giả lập", nói là chuyển đổi thật trên máy người dùng.
11. Chất lượng mã hoá dùng mặc định cố định, không có ô chỉnh.

## Để người thực hiện quyết

- Cách tách file: gợi ý một module thuần (đuôi file → nhóm → danh sách đích, các bộ chuyển
  văn bản) tách khỏi component để kiểm thử được.
- Thư viện cho Markdown/HTML/DOCX và cho zip. Ưu tiên ít phụ thuộc nhất; tự viết nếu chỉ vài chục
  dòng (SRT↔VTT, CSV↔TSV, CSV→bảng Markdown).
- Tham số ffmpeg cho từng đích. Trước khi cố định bảng định dạng, kiểm tra trên tài liệu của
  `@ffmpeg/core` xem bản dựng có bộ mã hoá nào; đích nào không mã hoá được thì bỏ khỏi bảng.
- Cách hiện tiến trình cho ảnh (gần như tức thì, có thể không cần thanh).

## Kiểm chứng

- Một bài test nhỏ cho module thuần: bảng nhóm/đích và mỗi bộ chuyển văn bản tự viết. Không thêm
  framework test nếu trình chạy test có sẵn của Node đủ dùng.
- Chạy tay trên `next dev` và trên bản `next build` tĩnh: thả 3 ảnh → WebP; thả ảnh + mp4 + srt
  → mỗi file một đích; mở file tải về bằng chương trình khác để chắc là không hỏng.
- Mở tab Network: không có request nào mang nội dung file; ffmpeg không được tải khi chỉ chuyển
  PNG sang WebP.
- `npm run build` và `npx tsc --noEmit` trong `website/` phải qua (`next lint` chưa được cấu hình nên không dùng).
