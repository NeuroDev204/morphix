import {
  Check,
  MousePointerClick,
  Sliders,
  CheckCircle2,
  Shield,
  Cpu,
  Zap,
} from "lucide-react";
import FormatFilterGrid from "./FormatFilterGrid";

export default function FeaturesZPattern() {
  return (
    <section id="tinh-nang" className="py-20 sm:py-28 px-4 sm:px-8 max-w-7xl mx-auto space-y-24 sm:space-y-32">
      {/* Feature 1: Context Menu */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-12 lg:gap-16 items-center">
        <div className="space-y-4">
          <div className="w-10 h-10 rounded-xl bg-teal-50 border border-teal-100 text-teal-700 font-bold text-base flex items-center justify-center shadow-2xs">
            1
          </div>
          <h2 className="text-2xl sm:text-3xl lg:text-4xl font-extrabold tracking-tight text-slate-950">
            Bấm chuột phải là xong. Không mở cửa sổ thừa.
          </h2>
          <p className="text-sm sm:text-base text-slate-600 leading-relaxed">
            Khác với các ứng dụng cồng kềnh bắt bạn mở giao diện, bấm nút Import, chọn thư mục lưu, Morphix
            tích hợp trực tiếp vào trình quản lý file của hệ điều hành. Chỉ cần bấm chuột phải lên file, chọn
            định dạng cần đổi, file kết quả xuất hiện ngay bên cạnh.
          </p>
          <ul className="space-y-2.5 text-xs sm:text-sm text-slate-700 pt-2 font-medium">
            <li className="flex items-center gap-2.5">
              <Check className="w-4 h-4 text-emerald-600 shrink-0" />
              <span>
                File gốc <strong>không bao giờ bị ghi đè</strong>
              </span>
            </li>
            <li className="flex items-center gap-2.5">
              <Check className="w-4 h-4 text-emerald-600 shrink-0" />
              <span>Gửi thông báo hệ thống ngay khi tác vụ hoàn thành</span>
            </li>
            <li className="flex items-center gap-2.5">
              <Check className="w-4 h-4 text-emerald-600 shrink-0" />
              <span>Hỗ trợ chọn và xử lý hàng loạt file cùng một lúc</span>
            </li>
          </ul>
        </div>

        <div className="rounded-3xl border border-slate-200/90 bg-white p-6 sm:p-8 shadow-sm card-glow">
          <div className="space-y-3.5 w-full max-w-md mx-auto text-xs">
            <div className="p-4 bg-slate-50/80 rounded-2xl border border-slate-200/80 shadow-2xs flex items-center gap-3.5 hover:bg-slate-100/80 transition-colors">
              <div className="w-10 h-10 rounded-xl bg-teal-50 text-teal-700 flex items-center justify-center shrink-0">
                <MousePointerClick className="w-5 h-5" />
              </div>
              <div>
                <div className="font-bold text-slate-900 text-sm">1. Chuột phải lên file</div>
                <div className="text-slate-500 text-xs">Nautilus, Dolphin, Thunar, Windows Explorer</div>
              </div>
            </div>

            <div className="p-4 bg-slate-50/80 rounded-2xl border border-slate-200/80 shadow-2xs flex items-center gap-3.5 hover:bg-slate-100/80 transition-colors">
              <div className="w-10 h-10 rounded-xl bg-teal-50 text-teal-700 flex items-center justify-center shrink-0">
                <Sliders className="w-5 h-5" />
              </div>
              <div>
                <div className="font-bold text-slate-900 text-sm">2. Chọn tác vụ tức thì</div>
                <div className="text-slate-500 text-xs">Đổi đuôi, nén kích thước, trích xuất âm thanh...</div>
              </div>
            </div>

            <div className="p-4 bg-teal-50/90 rounded-2xl border border-teal-200 shadow-2xs flex items-center gap-3.5 text-teal-950">
              <div className="w-10 h-10 rounded-xl bg-teal-700 text-white flex items-center justify-center shrink-0 shadow-sm shadow-teal-900/10">
                <CheckCircle2 className="w-5 h-5" />
              </div>
              <div>
                <div className="font-bold text-teal-950 text-sm">3. Nhận file kết quả ngay</div>
                <div className="text-teal-700 text-xs">Lưu cạnh file gốc với tốc độ SSD tối đa</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Feature 2: 100% Offline Privacy */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-12 lg:gap-16 items-center">
        <div className="order-2 md:order-1 rounded-3xl border border-slate-200/90 bg-white p-8 sm:p-10 flex items-center justify-center shadow-sm card-glow">
          <div className="text-center space-y-4 max-w-xs">
            <div className="w-20 h-20 rounded-2xl bg-emerald-100 text-emerald-700 flex items-center justify-center mx-auto shadow-xs">
              <Shield className="w-10 h-10" />
            </div>
            <div className="text-base font-bold text-slate-950">Bảo mật chuẩn Doanh nghiệp</div>
            <p className="text-xs text-slate-600 leading-relaxed">
              Báo cáo tài chính, hợp đồng quét scan, ảnh riêng tư và dữ liệu dự án không bao giờ rời khỏi máy
              tính cá nhân của bạn.
            </p>
          </div>
        </div>

        <div className="order-1 md:order-2 space-y-4">
          <div className="w-10 h-10 rounded-xl bg-teal-50 border border-teal-100 text-teal-700 font-bold text-base flex items-center justify-center shadow-2xs">
            2
          </div>
          <h2 className="text-2xl sm:text-3xl lg:text-4xl font-extrabold tracking-tight text-slate-950">
            100% Xử lý Offline. Bảo mật dữ liệu tuyệt đối.
          </h2>
          <p className="text-sm sm:text-base text-slate-600 leading-relaxed">
            Các dịch vụ chuyển đổi trực tuyến yêu cầu bạn tải tài liệu lên máy chủ của họ. Bạn không thể kiểm
            soát ai có quyền truy cập hay file được lưu lại bao lâu. Morphix chạy hoàn toàn trên CPU máy tính
            của bạn: không cần mạng, không có máy chủ trung gian, 0 nguy cơ rò rỉ.
          </p>
          <ul className="space-y-2.5 text-xs sm:text-sm text-slate-700 pt-2 font-medium">
            <li className="flex items-center gap-2.5">
              <Check className="w-4 h-4 text-emerald-600 shrink-0" />
              <span>Hoạt động hoàn hảo ngay cả khi ngắt kết nối Internet hoàn toàn</span>
            </li>
            <li className="flex items-center gap-2.5">
              <Check className="w-4 h-4 text-emerald-600 shrink-0" />
              <span>Không thu thập thông tin, không gắn mã theo dõi (analytics)</span>
            </li>
          </ul>
        </div>
      </div>

      {/* Feature 3: Format Matrix with Filter Tabs */}
      <FormatFilterGrid />

      {/* Feature 4: Rust Engine Performance */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-12 lg:gap-16 items-center">
        <div className="space-y-4">
          <div className="w-10 h-10 rounded-xl bg-teal-50 border border-teal-100 text-teal-700 font-bold text-base flex items-center justify-center shadow-2xs">
            4
          </div>
          <h2 className="text-2xl sm:text-3xl lg:text-4xl font-extrabold tracking-tight text-slate-950">
            Động cơ Rust siêu tốc. Tiết kiệm tài nguyên máy tính.
          </h2>
          <p className="text-sm sm:text-base text-slate-600 leading-relaxed">
            Được xây dựng từ đầu bằng Rust 2021 Edition, Morphix khởi động chỉ dưới 15 mili-giây, chiếm chưa
            đầy 30 MB bộ nhớ RAM khi chạy nền và tận dụng tối đa kiến trúc đa nhân CPU để chuyển đổi file
            song song với băng thông phần cứng cao nhất.
          </p>
          <ul className="space-y-2.5 text-xs sm:text-sm text-slate-700 pt-2 font-medium">
            <li className="flex items-center gap-2.5">
              <Check className="w-4 h-4 text-emerald-600 shrink-0" />
              <span>Không có Garbage Collector gây giật lag hay ngốn RAM</span>
            </li>
            <li className="flex items-center gap-2.5">
              <Check className="w-4 h-4 text-emerald-600 shrink-0" />
              <span>Xử lý đa luồng SIMD tăng tốc nén ảnh và trích xuất media</span>
            </li>
            <li className="flex items-center gap-2.5">
              <Check className="w-4 h-4 text-emerald-600 shrink-0" />
              <span>Dung lượng cài đặt siêu gọn chỉ ~24 MB (thay vì 300 MB của Electron)</span>
            </li>
          </ul>
        </div>

        <div className="rounded-3xl border border-slate-200/90 bg-white p-6 sm:p-8 shadow-sm card-glow">
          <div className="space-y-4 max-w-md mx-auto">
            <div className="p-4 rounded-2xl bg-teal-50/80 border border-teal-200 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Zap className="w-5 h-5 text-teal-700" />
                <span className="font-bold text-slate-900 text-sm">Thời gian khởi động</span>
              </div>
              <span className="font-mono font-bold text-teal-800 text-sm">&lt; 15 ms</span>
            </div>
            <div className="p-4 rounded-2xl bg-slate-50 border border-slate-200 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Cpu className="w-5 h-5 text-slate-700" />
                <span className="font-bold text-slate-900 text-sm">Mức chiếm RAM</span>
              </div>
              <span className="font-mono font-bold text-slate-800 text-sm">~ 28 MB</span>
            </div>
            <div className="p-4 rounded-2xl bg-slate-50 border border-slate-200 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <CheckCircle2 className="w-5 h-5 text-emerald-600" />
                <span className="font-bold text-slate-900 text-sm">An toàn bộ nhớ</span>
              </div>
              <span className="font-mono font-bold text-emerald-700 text-sm">100% Rust Safe</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
