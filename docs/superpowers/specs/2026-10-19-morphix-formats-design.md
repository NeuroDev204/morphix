# Morphix: mở rộng danh sách định dạng

Tài liệu này bổ sung cho `2026-10-19-morphix-design.md`. Mọi mục không được nhắc ở đây giữ nguyên như spec gốc. Mục được đánh dấu **[Mình chọn]** là quyết định của người viết, cần người dùng duyệt; mục **[Đã chốt]** là đã được người dùng chốt trước đó.

## 1. Ý định và lý do

Người dùng muốn Morphix chuyển đổi được các định dạng phổ biến hơn nữa: âm thanh, video, văn bản, phụ đề, bảng CSV, file nén. Danh sách gốc có cả định dạng Office và Apple (DOCX, XLSX, PPTX, PAGES) mà spec gốc đã loại trừ. Tài liệu này ghi lại quyết định với từng nhóm, để người lập kế hoạch biết chính xác phải làm gì và không phải đoán.

## 2. Mục tiêu, không mục tiêu, phản mục tiêu

Mục tiêu và phản mục tiêu kế thừa từ spec gốc, mục 2. Bổ sung:

### Mục tiêu bổ sung
1. Âm thanh và video: chuyển định dạng, nén, cắt đoạn, đổi tốc độ, tách âm thanh (đợt 2 của spec gốc).
2. Văn bản thuần, phụ đề và bảng CSV/TSV chuyển được qua lại, làm hoàn toàn bằng Rust, không phụ thuộc chương trình ngoài.
3. PDF sang TXT, chỉ lấy lớp văn bản có sẵn trong PDF, không OCR. **[Mình chọn]**

### Không mục tiêu bổ sung
- **Office đọc và ghi** (DOCX, XLSX, XLS, PPTX, PPT, ODS): giữ loại trừ như spec gốc. **[Mình chọn]** Lý do: cần LibreOffice hoặc engine tương đương, bộ cài nặng, chậm, phải kiểm giấy phép lại. Có thể xét lại sau bằng một đợt riêng.
- **PAGES**: không hỗ trợ. **[Mình chọn]** Định dạng độc quyền của Apple, không có bộ giải mã mở đáng tin cậy.
- **PDF sang Office, EPUB hoặc Markdown**: không hỗ trợ bản đầu. **[Mình chọn]** Cần phân tích bố cục và thường cần OCR, trái với mục không mục tiêu gốc.
- **Slide** (PPT/PPTX sang PDF/ảnh, PDF sang PPTX): không hỗ trợ, theo lý do Office ở trên.
- **OCR** (nhận dạng chữ trong ảnh hoặc PDF scan): không hỗ trợ, như spec gốc.
- **EPUB đọc và ghi**: để sau. **[Mình chọn]** Cấu trúc đóng gói phức tạp hơn lợi ích; đưa vào đợt sau nếu người dùng yêu cầu.
- **RTF đọc, HTML ghi**: để sau. **[Mình chọn]** Không nằm trong nhóm bắt buộc bản đầu.

### Phản mục tiêu bổ sung
- Chuyển phụ đề hoặc CSV mà mất dữ liệu âm thầm (ví dụ mất mốc thời gian của phụ đề, mất dấu ngoặc kép trong CSV) mà không báo.
- Chương trình âm thanh hoặc video ngoài bị thiếu nhưng app vẫn báo thành công.

## 2a. Quyết định của người dùng (đã duyệt, thay thế mục tương ứng trong spec gốc)

- **[Đã chốt] Gói cài Linux là `.deb`, cần quyền quản trị (sudo) để cài.** Điều này thay thế phản mục tiêu "phải cài bằng quyền quản trị mới dùng được" trong spec gốc mục 2, chỉ với gói `.deb`. Việc dùng app sau khi cài và tích hợp menu (`install-integration`) vẫn chạy bằng quyền người dùng, không cần sudo.
- **[Đã chốt] ffmpeg đóng gói là bản GPL có `libx264`** (R1). Hệ quả phải ghi nhận: `.deb` là phân phối phần mềm GPL cho phần ffmpeg, nên phải cung cấp mã nguồn ffmpeg tương ứng và giữ thông báo giấy phép. Morphix chạy ffmpeg như tiến trình riêng, không liên kết tĩnh vào mã của Morphix. Việc tuân thủ đầy đủ được kiểm trước khi phát hành (Ruling). Đây không phải tư vấn pháp lý.
- Nguồn bản ffmpeg GPL đang dùng: gói `ffmpeg` 7:6.1.1-3ubuntu5 của Ubuntu (cấu hình `--enable-gpl --enable-libx264`), mã nguồn lấy từ gói nguồn cùng tên của Ubuntu.

## 3. Ràng buộc (kế thừa, bổ sung)

- Offline tuyệt đối; không tải về bất kỳ thứ gì khi chạy.
- Không ghi đè file gốc; đầu ra cạnh file gốc với hậu tố theo spec gốc mục 5.4.
- Lỗi phải hiện cho người dùng bằng câu nói được nguyên nhân, và ghi log.
- Giấy phép: mọi thư viện hoặc chương trình đi kèm phải được kiểm trước khi phát hành (ffmpeg LGPL, các crate Rust). Người lập kế hoạch ghi rõ giấy phép từng mục vào Ruling.

## 4. Phạm vi định dạng theo đợt

Đợt đánh số theo spec gốc mục 9. Đợt mới **2b** chèn sau đợt 2 và không phụ thuộc ffmpeg.

### Ảnh (đợt 1 của spec gốc, bổ sung)

| Định dạng | Đầu vào | Đầu ra | Ghi chú |
|---|---|---|---|
| JPG, PNG, TIFF, BMP, GIF, AVIF | có | có | đã có trong plan core |
| WebP | có | có (có thể mất mát) | cần `libwebp` cho bản mất mát |
| HEIC | chưa | chưa | **[Mình chọn]** để sau. Cần `libheif`, có vướng bằng sáng chế |
| SVG | chưa | chỉ PDF hoặc PNG | **[Mình chọn]** để sau, cần bộ dựng SVG |

### Âm thanh và video (đợt 2 của spec gốc, bổ sung định dạng)

Chạy qua ffmpeg như spec gốc mục 7.

| Nhóm | Định dạng đầu vào | Đầu ra |
|---|---|---|
| Âm thanh | MP3, M4A, WAV, FLAC, OGG, Opus, AIFF, WMA | MP3, M4A, WAV, FLAC, OGG, Opus |
| Video | MP4, MOV, MKV, WebM, AVI, WMV, GIF | MP4, MOV, MKV, WebM, AVI |

- **[Mình chọn]** WMA, AIFF, WMV chỉ là đầu vào, không có đầu ra, vì định dạng xuất ít được cần và tránh tăng bộ mã hoá. Người lập kế hoạch có thể bỏ trạng thái "chỉ đầu vào" sau khi thử ffmpeg.
- **[Mình chọn]** GIF là đầu ra của video (chuyển video ngắn thành GIF) và là đầu vào của ảnh.

### Văn bản, phụ đề, bảng nhỏ (đợt 2b, mới)

Làm bằng Rust thuần, không gọi chương trình ngoài.

| Nhóm | Đầu vào | Đầu ra |
|---|---|---|
| Văn bản | TXT, MD | TXT, MD |
| Phụ đề | SRT, VTT | SRT, VTT, TXT (chỉ nội dung, bỏ mốc thời gian) |
| Bảng | CSV, TSV | CSV, TSV |

Quy tắc chuyển:
- Phụ đề giữ nguyên mốc thời gian khi đổi SRT sang VTT. Khi xuất TXT thì bỏ mốc, giữ dòng văn bản.
- CSV và TSV: bảo toàn dấu ngoặc kép và ký tự xuống dòng trong ô; không làm mất dữ liệu. Nếu một ô không biểu diễn được ở định dạng đích, báo lỗi đúng dòng, không cắt âm thầm.
- Mã hoá: đầu vào UTF-8 và UTF-8 có BOM; các mã hoá khác báo lỗi rõ, không đoán.

### PDF (đợt 3 của spec gốc, bổ sung)

- PDF sang JPG hoặc PNG: như spec gốc.
- **PDF sang TXT** (mới, **[Mình chọn]**): chỉ lấy lớp văn bản có sẵn trong PDF. Nếu trang không có lớp văn bản (PDF scan) thì báo rõ "trang N không có chữ có thể chọn, cần OCR, chưa hỗ trợ", không tạo file rỗng.

### File nén (đợt 4 của spec gốc, bổ sung)

- ZIP, TAR, GZIP, 7Z: đọc và ghi như spec gốc.
- RAR: **chỉ đọc**, và để sau đợt 4. **[Mình chọn]** Giấy phép RAR đóng; cần kiểm trước khi làm.

## 5. Chuyển đổi không hỗ trợ: thông báo

Khi người dùng chọn một cặp không hỗ trợ (ví dụ DOCX sang PDF), Morphix không tạo file. Vòng tròn không hiện múi đó. Nếu chạy từ CLI, thông báo nói rõ: "DOCX chưa được hỗ trợ; Morphix chưa đọc định dạng Office", và thoát với mã lỗi 2 (dùng sai lệnh).

## 6. Tiêu chí hoàn thành

Mỗi đợt có tiêu chí kiểm bằng chạy thật, theo quy tắc spec gốc mục 9:
- **Đợt 2 (âm thanh, video):** chuyển mỗi định dạng trong bảng sang ít nhất một đích, nghe và xem được; cắt đoạn đúng thời gian; ffmpeg thiếu thì báo đúng cách cài.
- **Đợt 2b (văn bản, phụ đề, CSV):** bộ file mẫu gồm tiếng Việt có dấu, dấu ngoặc kép trong CSV, ô có xuống dòng, phụ đề nhiều dòng. Chuyển qua lại không mất ký tự; chạy được qua CLI và vòng tròn.
- **Đợt 3 (PDF sang TXT):** PDF có lớp văn bản ra đúng chữ; PDF scan báo đúng lỗi.
- **Đợt 4 (nén):** ZIP, TAR.GZ, 7Z giải nén và nén lại được; file gốc không đổi.

## 7. Các mục để người lập kế hoạch quyết định (không cần người dùng)

- Chọn crate Rust cho CSV, phụ đề và Markdown sau khi thử nhỏ, ghi kết quả vào Ruling.
- Cách đóng gói ffmpeg trên Linux và Windows (đã nêu ở spec gốc mục 7).
- Chọn thư viện đọc lớp văn bản PDF (`pdfium-render` hoặc `lopdf`), theo thử nghiệm.

## 8. Câu hỏi còn mở cho người dùng

1. Có đồng ý không đưa Office vào bản đầu? (Khuyến nghị: không.)
2. Có đồng ý bỏ PAGES hoàn toàn? (Khuyến nghị: có.)
3. Có đồng ý thêm đợt 2b (văn bản, phụ đề, CSV) vào lộ trình? (Khuyến nghị: có.)
4. HEIC và SVG để sau có được không? (Khuyến nghị: có.)
5. PDF sang TXT có được đưa vào đợt 3 không? (Khuyến nghị: có, không OCR.)
6. EPUB, RTF và HTML để sau có được không? (Khuyến nghị: có.)
