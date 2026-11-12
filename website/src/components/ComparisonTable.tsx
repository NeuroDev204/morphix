import {
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Clock,
  MousePointerClick,
  ExternalLink,
  Ban,
} from "lucide-react";

export default function ComparisonTable() {
  return (
    <section id="so-sanh" className="py-20 sm:py-28 px-4 sm:px-8 bg-slate-100/70 border-y border-slate-200/80">
      <div className="max-w-5xl mx-auto">
        <div className="text-center max-w-2xl mx-auto mb-14 space-y-3">
          <span className="text-xs font-bold uppercase tracking-widest text-teal-700">
            Tại sao chọn Morphix?
          </span>
          <h2 className="text-2xl sm:text-4xl font-extrabold tracking-tight text-slate-950">
            So sánh Morphix với Công cụ Online
          </h2>
          <p className="text-sm text-slate-600">
            Đừng lãng phí thời gian và mạo hiểm tệp tin riêng tư của bạn trên các website tải lên đám mây.
          </p>
        </div>

        {/* Comparison Table */}
        <div className="overflow-x-auto rounded-3xl border border-slate-200/90 bg-white shadow-sm">
          <table className="w-full text-left text-sm border-collapse">
            <thead>
              <tr className="border-b border-slate-200/80 bg-slate-50/90 text-xs font-bold uppercase tracking-wider text-slate-700">
                <th className="p-4 sm:p-5 w-1/3">Tiêu chí so sánh</th>
                <th className="p-4 sm:p-5 w-1/3 text-teal-800 bg-teal-50/70 border-x border-slate-200/80 font-bold">
                  <div className="flex items-center gap-2">
                    <div className="w-5 h-5 rounded-md bg-teal-700 text-white flex items-center justify-center text-[10px] font-bold shadow-2xs">
                      M
                    </div>
                    Morphix (On-device)
                  </div>
                </th>
                <th className="p-4 sm:p-5 w-1/3 text-slate-500">
                  Web Online (CloudConvert, Convertio...)
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200/80">
              <tr className="hover:bg-slate-50/50 transition-colors">
                <td className="p-4 sm:p-5 font-semibold text-slate-900">Quyền riêng tư & Bảo mật</td>
                <td className="p-4 sm:p-5 bg-teal-50/30 border-x border-slate-200/80 font-semibold text-emerald-800">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
                    <span>100% Trên máy, 0 byte gửi ra ngoài</span>
                  </div>
                </td>
                <td className="p-4 sm:p-5 text-slate-600">
                  <div className="flex items-center gap-2">
                    <AlertTriangle className="w-4 h-4 text-amber-500 shrink-0" />
                    <span>Tải file lên server bên thứ ba lưu trữ</span>
                  </div>
                </td>
              </tr>

              <tr className="hover:bg-slate-50/50 transition-colors">
                <td className="p-4 sm:p-5 font-semibold text-slate-900">Giới hạn dung lượng</td>
                <td className="p-4 sm:p-5 bg-teal-50/30 border-x border-slate-200/80 font-semibold text-emerald-800">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
                    <span>Không giới hạn (theo ổ cứng máy bạn)</span>
                  </div>
                </td>
                <td className="p-4 sm:p-5 text-slate-600">
                  <div className="flex items-center gap-2">
                    <XCircle className="w-4 h-4 text-rose-500 shrink-0" />
                    <span>Chỉ 50MB - 100MB (bắt mua gói VIP)</span>
                  </div>
                </td>
              </tr>

              <tr className="hover:bg-slate-50/50 transition-colors">
                <td className="p-4 sm:p-5 font-semibold text-slate-900">Tốc độ chuyển đổi</td>
                <td className="p-4 sm:p-5 bg-teal-50/30 border-x border-slate-200/80 font-semibold text-emerald-800">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
                    <span>Tức thì qua SSD & CPU đa nhân</span>
                  </div>
                </td>
                <td className="p-4 sm:p-5 text-slate-600">
                  <div className="flex items-center gap-2">
                    <Clock className="w-4 h-4 text-slate-400 shrink-0" />
                    <span>Phụ thuộc tốc độ upload mạng & hàng đợi</span>
                  </div>
                </td>
              </tr>

              <tr className="hover:bg-slate-50/50 transition-colors">
                <td className="p-4 sm:p-5 font-semibold text-slate-900">Thao tác người dùng</td>
                <td className="p-4 sm:p-5 bg-teal-50/30 border-x border-slate-200/80 font-semibold text-emerald-800">
                  <div className="flex items-center gap-2">
                    <MousePointerClick className="w-4 h-4 text-teal-700 shrink-0" />
                    <span>Chuột phải 1 click ngay trong thư mục</span>
                  </div>
                </td>
                <td className="p-4 sm:p-5 text-slate-600">
                  <div className="flex items-center gap-2">
                    <ExternalLink className="w-4 h-4 text-slate-400 shrink-0" />
                    <span>Mở trình duyệt, kéo thả file, chờ tải về</span>
                  </div>
                </td>
              </tr>

              <tr className="hover:bg-slate-50/50 transition-colors">
                <td className="p-4 sm:p-5 font-semibold text-slate-900">Chi phí & Quảng cáo</td>
                <td className="p-4 sm:p-5 bg-teal-50/30 border-x border-slate-200/80 font-semibold text-emerald-800">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
                    <span>100% Miễn phí, mã nguồn mở, 0 quảng cáo</span>
                  </div>
                </td>
                <td className="p-4 sm:p-5 text-slate-600">
                  <div className="flex items-center gap-2">
                    <Ban className="w-4 h-4 text-rose-500 shrink-0" />
                    <span>Nhiều quảng cáo phiền toái, giới hạn lượt tải</span>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
