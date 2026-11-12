"use client";

import { useEffect, useState } from "react";
import { Download, Terminal, ShieldCheck, HardDrive, Cpu, Sparkles } from "lucide-react";
import { DOWNLOADS } from "@/constants/downloads";
import HeroSimulator from "./HeroSimulator";

export default function Hero() {
  const [detectedOs, setDetectedOs] = useState<"windows" | "linux" | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const ua = navigator.userAgent;
    const isWin = /Windows|Win32|Win64/i.test(ua);
    const isLin = /Linux/i.test(ua) && !/Android/i.test(ua);

    if (isWin) {
      setDetectedOs("windows");
      const cardWin = document.getElementById("card-windows");
      if (cardWin) cardWin.classList.add("ring-2", "ring-teal-500", "border-teal-500");
    } else if (isLin) {
      setDetectedOs("linux");
      const cardLin = document.getElementById("card-linux");
      if (cardLin) cardLin.classList.add("ring-2", "ring-teal-500", "border-teal-500");
    }
  }, []);

  const handleScrollToLinux = () => {
    const target = document.getElementById("tai-ve-linux");
    if (target) {
      target.scrollIntoView({ behavior: "smooth", block: "center" });
      const cardLinux = document.getElementById("card-linux") || target;
      cardLinux.classList.add("ring-4", "ring-teal-400", "scale-[1.02]", "transition-all");
      setTimeout(() => {
        cardLinux.classList.remove("ring-4", "ring-teal-400", "scale-[1.02]");
      }, 1800);
    }
  };

  return (
    <section className="relative overflow-hidden pt-12 pb-16 sm:pt-20 sm:pb-24 border-b border-slate-200/80 bg-radial-hero">
      <div className="max-w-7xl mx-auto px-4 sm:px-8 grid grid-cols-1 lg:grid-cols-12 gap-12 lg:gap-10 items-center">
        {/* Left: Copy & Actions */}
        <div className="lg:col-span-7 space-y-6">
          <div className="inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-teal-50 border border-teal-200/90 text-teal-900 text-xs font-semibold shadow-2xs">
            <span className="w-2 h-2 rounded-full bg-teal-600 animate-pulse" />
            Morphix v{DOWNLOADS.version} phát hành · 100% Miễn phí & Mã nguồn mở
          </div>

          <h1 className="text-3xl sm:text-5xl lg:text-6xl font-extrabold tracking-tight text-slate-950 leading-[1.12]">
            Đổi định dạng & nén file{" "}
            <span className="text-transparent bg-clip-text bg-gradient-to-r from-teal-700 via-teal-600 to-emerald-600">
              ngay trên máy
            </span>{" "}
            của bạn.
          </h1>

          <p className="text-base sm:text-lg text-slate-600 font-normal leading-relaxed max-w-2xl">
            Bấm chuột phải lên file, chọn việc cần làm. File kết quả nằm cạnh file gốc,{" "}
            <strong className="font-semibold text-slate-800">không gửi 1 byte nào ra ngoài</strong>. Viết
            bằng Rust, tốc độ tức thì, an toàn tuyệt đối cho tài liệu mật.
          </p>

          {/* Primary Hero Actions */}
          <div className="pt-2 flex flex-col sm:flex-row items-stretch sm:items-center gap-3.5">
            {/* Windows Main CTA with Shimmer Effect */}
            <a
              href={DOWNLOADS.windows.exe}
              id="btn-hero-win"
              className="btn-shimmer inline-flex items-center justify-center gap-3 h-14 px-8 rounded-2xl bg-teal-700 text-white font-bold text-base shadow-lg shadow-teal-900/20 hover:bg-teal-800 active:bg-teal-900 hover:-translate-y-0.5 transition-all group"
            >
              <Download className="w-5 h-5 group-hover:-translate-y-0.5 transition-transform" />
              <span>Tải cho Windows (exe) · v{DOWNLOADS.version}</span>
            </a>

            {/* Linux CTA Anchor with Smooth Scroll */}
            <button
              type="button"
              id="btn-hero-linux"
              onClick={handleScrollToLinux}
              className="cursor-pointer inline-flex items-center justify-center gap-2.5 h-14 px-6 rounded-2xl border border-slate-300 bg-white/80 backdrop-blur-xs text-slate-700 font-semibold text-sm hover:bg-white hover:border-slate-400 hover:shadow-xs active:bg-slate-100 transition-all"
            >
              <Terminal className="w-4 h-4 text-slate-500" />
              <span>Cài đặt cho Linux (cuộn xuống)</span>
            </button>
          </div>

          {/* Feature Badges below CTA */}
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-xs text-slate-600 pt-1 font-medium">
            <span className="flex items-center gap-1.5">
              <ShieldCheck className="w-4 h-4 text-emerald-600" /> Không cần quyền Admin
            </span>
            <span className="flex items-center gap-1.5">
              <HardDrive className="w-4 h-4 text-emerald-600" /> Dung lượng gọn ~{DOWNLOADS.windows.size}
            </span>
            <span className="flex items-center gap-1.5">
              <Cpu className="w-4 h-4 text-emerald-600" /> Windows 10/11 & Linux
            </span>
          </div>

          {/* Detected OS Banner */}
          {detectedOs && (
            <div
              id="detected-os-banner"
              className="flex items-center gap-2 text-xs font-semibold text-teal-900 bg-teal-50/90 border border-teal-200/90 px-3.5 py-2.5 rounded-xl max-w-md shadow-2xs"
            >
              <Sparkles className="w-4 h-4 text-teal-600 shrink-0" />
              <span id="detected-os-text">
                {detectedOs === "windows" ? (
                  <>
                    Hệ điều hành của bạn: <strong>Windows (64-bit)</strong>. Bản cài đặt exe đã sẵn sàng tải về.
                  </>
                ) : (
                  <>
                    Hệ điều hành của bạn: <strong>Linux (x86_64)</strong>. Bạn có thể dùng lệnh curl hoặc tải gói .deb bên dưới.
                  </>
                )}
              </span>
            </div>
          )}
        </div>

        {/* Right: Interactive Desktop Simulator */}
        <div className="lg:col-span-5">
          <HeroSimulator />
        </div>
      </div>
    </section>
  );
}
