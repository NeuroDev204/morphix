"use client";

import { useState } from "react";
import { Calculator } from "lucide-react";

export default function StorageCalculator() {
  const [gb, setGb] = useState(25);

  const savedSpace = (gb * 0.85).toFixed(1);
  const uploadHours = (gb * 0.1).toFixed(1);

  return (
    <section id="tiet-kiem" className="py-16 sm:py-24 px-4 sm:px-8 max-w-7xl mx-auto border-t border-slate-200/80">
      <div className="max-w-4xl mx-auto bg-slate-900 text-white rounded-3xl p-8 sm:p-12 shadow-2xl relative overflow-hidden">
        <div className="absolute -right-20 -bottom-20 w-80 h-80 rounded-full bg-teal-500/10 blur-3xl pointer-events-none" />

        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 items-center">
          <div className="lg:col-span-7 space-y-5">
            <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-teal-900/60 border border-teal-500/40 text-teal-300 text-xs font-semibold">
              <Calculator className="w-3.5 h-3.5" /> Công cụ ước tính dung lượng
            </div>
            <h2 className="text-2xl sm:text-3xl font-extrabold tracking-tight text-white leading-tight">
              Bạn có bao nhiêu file ảnh & video cần lưu trữ?
            </h2>
            <p className="text-xs sm:text-sm text-slate-300 leading-relaxed">
              Kéo thanh trượt để xem công nghệ nén WebP/AVIF và 7-Zip LZMA2 của Morphix tiết kiệm cho bạn bao
              nhiêu dung lượng ổ cứng.
            </p>

            {/* Slider Control */}
            <div className="pt-2 space-y-2">
              <div className="flex items-center justify-between text-xs font-mono">
                <span className="text-slate-400">Dung lượng file gốc:</span>
                <span className="text-teal-400 font-bold text-base">{gb} GB</span>
              </div>
              <input
                type="range"
                min="1"
                max="100"
                value={gb}
                onChange={(e) => setGb(parseInt(e.target.value, 10))}
                className="w-full h-2.5 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-teal-400"
              />
              <div className="flex justify-between text-[10px] text-slate-500 font-mono">
                <span>1 GB</span>
                <span>25 GB</span>
                <span>50 GB</span>
                <span>100 GB</span>
              </div>
            </div>
          </div>

          {/* Stats Output Box */}
          <div className="lg:col-span-5 bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 space-y-5 text-center">
            <div className="space-y-1">
              <div className="text-[11px] text-slate-400 uppercase tracking-wider font-semibold">
                Dung lượng ổ cứng tiết kiệm được:
              </div>
              <div className="text-4xl sm:text-5xl font-black text-transparent bg-clip-text bg-gradient-to-r from-teal-400 to-emerald-400">
                {savedSpace} GB
              </div>
              <div className="text-xs text-teal-300 font-medium">Giảm trung bình ~85% dung lượng</div>
            </div>

            <div className="pt-4 border-t border-slate-700/80 grid grid-cols-2 gap-3 text-left text-xs">
              <div>
                <div className="text-[10px] text-slate-400">Dữ liệu tải lên cloud:</div>
                <div className="font-bold text-emerald-400 font-mono">0 Byte (100% Safe)</div>
              </div>
              <div>
                <div className="text-[10px] text-slate-400">Thời gian tiết kiệm:</div>
                <div className="font-bold text-white font-mono">~{uploadHours} giờ upload</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
