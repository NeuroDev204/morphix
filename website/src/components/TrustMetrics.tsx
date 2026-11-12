import { ShieldCheck, FileCheck2, Zap, Code2 } from "lucide-react";

export default function TrustMetrics() {
  return (
    <section className="border-b border-slate-200/80 bg-white/70 px-4 sm:px-8 py-9">
      <div className="max-w-7xl mx-auto grid grid-cols-2 md:grid-cols-4 gap-8 text-center">
        <div className="flex flex-col items-center group">
          <div className="flex items-center gap-2.5 text-2xl sm:text-3xl font-extrabold tracking-tight text-slate-900 group-hover:scale-105 transition-transform">
            <ShieldCheck className="w-7 h-7 text-emerald-600" />
            <span>0 Byte Cloud</span>
          </div>
          <p className="text-xs text-slate-500 mt-1.5 font-medium">
            100% On-device, không bao giờ gửi file ra ngoài
          </p>
        </div>

        <div className="flex flex-col items-center group">
          <div className="flex items-center gap-2.5 text-2xl sm:text-3xl font-extrabold tracking-tight text-slate-900 group-hover:scale-105 transition-transform">
            <FileCheck2 className="w-7 h-7 text-teal-700" />
            <span>30+ Định dạng</span>
          </div>
          <p className="text-xs text-slate-500 mt-1.5 font-medium">
            Ảnh, Video, Âm thanh, Tài liệu & Nén tệp
          </p>
        </div>

        <div className="flex flex-col items-center group">
          <div className="flex items-center gap-2.5 text-2xl sm:text-3xl font-extrabold tracking-tight text-slate-900 group-hover:scale-105 transition-transform">
            <Zap className="w-7 h-7 text-amber-500" />
            <span>&lt; 15ms</span>
          </div>
          <p className="text-xs text-slate-500 mt-1.5 font-medium">
            Khởi động siêu tốc, viết hoàn toàn bằng Rust
          </p>
        </div>

        <div className="flex flex-col items-center group">
          <div className="flex items-center gap-2.5 text-2xl sm:text-3xl font-extrabold tracking-tight text-slate-900 group-hover:scale-105 transition-transform">
            <Code2 className="w-7 h-7 text-slate-700" />
            <span>GPL-3.0</span>
          </div>
          <p className="text-xs text-slate-500 mt-1.5 font-medium">
            Mã nguồn mở miễn phí mãi mãi, không quảng cáo
          </p>
        </div>
      </div>
    </section>
  );
}
