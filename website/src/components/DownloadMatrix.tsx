"use client";

import { useState } from "react";
import {
  Laptop,
  Terminal,
  Download,
  Info,
  Package,
  Archive,
  Copy,
  Check,
} from "lucide-react";
import { DOWNLOADS } from "@/constants/downloads";

export default function DownloadMatrix() {
  const [copied, setCopied] = useState(false);

  const handleCopyCurl = async () => {
    try {
      await navigator.clipboard.writeText(DOWNLOADS.linux.curlCommand);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Lỗi sao chép:", err);
    }
  };

  return (
    <section id="tai-ve" className="py-20 sm:py-28 px-4 sm:px-8 max-w-7xl mx-auto border-t border-slate-200/80">
      <div className="text-center max-w-2xl mx-auto mb-14 space-y-3">
        <span className="text-xs font-bold uppercase tracking-widest text-teal-700">
          Tải về máy tính
        </span>
        <h2 className="text-2xl sm:text-4xl font-extrabold tracking-tight text-slate-950">
          Cài đặt Morphix v{DOWNLOADS.version}
        </h2>
        <p className="text-sm text-slate-600">
          Chọn gói cài đặt phù hợp với hệ điều hành Windows hoặc Linux của bạn.
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
        {/* ==================== CỘT WINDOWS ==================== */}
        <div
          id="card-windows"
          className="rounded-3xl border border-slate-200/90 bg-white p-6 sm:p-9 shadow-sm card-glow flex flex-col justify-between space-y-6 transition-all"
        >
          <div className="space-y-6">
            <div className="flex items-center justify-between pb-4 border-b border-slate-200/80">
              <div className="flex items-center gap-3.5">
                <div className="w-12 h-12 rounded-2xl bg-blue-50 text-blue-600 flex items-center justify-center shadow-2xs">
                  <Laptop className="w-6 h-6" />
                </div>
                <div>
                  <h3 className="text-xl font-bold text-slate-950">Windows</h3>
                  <div className="text-xs text-slate-500">{DOWNLOADS.windows.os}</div>
                </div>
              </div>
              <span className="text-xs font-bold px-3 py-1 rounded-full bg-slate-100 text-slate-700">
                v{DOWNLOADS.version}
              </span>
            </div>

            <p className="text-xs sm:text-sm text-slate-600 leading-relaxed">
              Cài cho người dùng hiện tại, <strong>không cần quyền quản trị (Admin)</strong>, và tự động
              tích hợp menu chuột phải tiện lợi.
            </p>

            {/* Direct EXE Download */}
            <div className="space-y-3">
              <label className="text-[11px] font-bold text-slate-500 uppercase tracking-wider block">
                Bản cài đặt trực tiếp (.exe):
              </label>
              <a
                href={DOWNLOADS.windows.exe}
                className="btn-shimmer w-full inline-flex items-center justify-center gap-2.5 py-4 px-6 rounded-2xl bg-teal-700 text-white font-bold text-sm shadow-md shadow-teal-900/15 hover:bg-teal-800 active:bg-teal-900 hover:-translate-y-0.5 transition-all"
              >
                <Download className="w-4 h-4" />
                <span>
                  Tải {DOWNLOADS.windows.name} ({DOWNLOADS.windows.size})
                </span>
              </a>
            </div>

            {/* SmartScreen Callout */}
            <div className="p-4 rounded-2xl bg-amber-50/80 border border-amber-200 text-xs text-amber-950 leading-relaxed space-y-1.5">
              <div className="flex items-center gap-2 font-bold text-amber-900">
                <Info className="w-4 h-4 text-amber-600 shrink-0" />
                <span>Lưu ý cảnh báo SmartScreen:</span>
              </div>
              <p className="text-amber-800">
                Bản exe chưa được mua chứng chỉ ký số đắt tiền nên lần chạy đầu Windows có thể hiện{" "}
                <em>&quot;Windows protected your PC&quot;</em>. Bạn chỉ cần bấm <strong>More info</strong>{" "}
                rồi chọn <strong>Run anyway</strong>.
              </p>
            </div>
          </div>

          <div className="pt-4 border-t border-slate-200/80 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-2 text-xs text-slate-500">
            <span>Gỡ cài: Settings → Apps hoặc <code>morphix uninstall</code></span>
            <a
              href={`${DOWNLOADS.releasesPage}/tag/v${DOWNLOADS.version}`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-teal-700 hover:underline font-medium"
            >
              Đối chiếu SHA-256
            </a>
          </div>
        </div>

        {/* ==================== CỘT LINUX ==================== */}
        <div id="tai-ve-linux" className="scroll-mt-24">
          <div
            id="card-linux"
            className="rounded-3xl border border-slate-200/90 bg-white p-6 sm:p-9 shadow-sm card-glow flex flex-col justify-between space-y-6 h-full transition-all"
          >
            <div className="space-y-6">
              <div className="flex items-center justify-between pb-4 border-b border-slate-200/80">
                <div className="flex items-center gap-3.5">
                  <div className="w-12 h-12 rounded-2xl bg-orange-50 text-orange-600 flex items-center justify-center shadow-2xs">
                    <Terminal className="w-6 h-6" />
                  </div>
                  <div>
                    <h3 className="text-xl font-bold text-slate-950">Linux (Mọi Distro)</h3>
                    <div className="text-xs text-slate-500">{DOWNLOADS.linux.os}</div>
                  </div>
                </div>
                <span className="text-xs font-bold px-3 py-1 rounded-full bg-slate-100 text-slate-700">
                  x86_64
                </span>
              </div>

              {/* Tùy chọn 4: Cài bằng curl 1 dòng */}
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="text-[11px] font-bold text-slate-500 uppercase tracking-wider">
                    Cách 1: Lệnh cài tự động (Khuyên dùng):
                  </label>
                  <span className="text-[10px] text-teal-800 font-semibold bg-teal-50 px-2 py-0.5 rounded border border-teal-200/60">
                    Không cần sudo
                  </span>
                </div>
                <div className="relative group">
                  <pre className="bg-slate-900 text-slate-100 p-4 pr-24 rounded-2xl font-mono text-xs overflow-x-auto selection:bg-teal-700">
                    <code>{DOWNLOADS.linux.curlCommand}</code>
                  </pre>
                  <button
                    type="button"
                    onClick={handleCopyCurl}
                    className="absolute top-3 right-3 px-3 py-1.5 rounded-lg bg-slate-800 text-slate-300 text-xs hover:bg-slate-700 flex items-center gap-1.5 transition-colors cursor-pointer"
                    title="Sao chép lệnh cài đặt"
                  >
                    {copied ? (
                      <>
                        <Check className="w-3.5 h-3.5 text-emerald-400" />
                        <span className="text-emerald-300 font-medium">Đã chép!</span>
                      </>
                    ) : (
                      <>
                        <Copy className="w-3.5 h-3.5" />
                        <span>Copy</span>
                      </>
                    )}
                  </button>
                </div>
              </div>

              {/* Tùy chọn 2 & 3: .deb & tarball */}
              <div className="space-y-2.5 pt-1">
                <label className="text-[11px] font-bold text-slate-500 uppercase tracking-wider block">
                  Cách 2: Gói tải về thủ công v{DOWNLOADS.version}:
                </label>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  {/* .deb */}
                  <a
                    href={DOWNLOADS.linux.deb}
                    className="inline-flex items-center justify-center gap-2 p-3.5 rounded-2xl border border-slate-200 bg-slate-50 hover:bg-slate-100 hover:border-slate-300 text-slate-800 text-xs font-semibold transition-all"
                  >
                    <Package className="w-4 h-4 text-orange-600" />
                    <span>Gói .deb ({DOWNLOADS.linux.sizeDeb})</span>
                  </a>

                  {/* tarball */}
                  <a
                    href={DOWNLOADS.linux.tarball}
                    className="inline-flex items-center justify-center gap-2 p-3.5 rounded-2xl border border-slate-200 bg-slate-50 hover:bg-slate-100 hover:border-slate-300 text-slate-800 text-xs font-semibold transition-all"
                  >
                    <Archive className="w-4 h-4 text-slate-600" />
                    <span>Tarball ({DOWNLOADS.linux.sizeTarball})</span>
                  </a>
                </div>
              </div>
            </div>

            <div className="pt-4 border-t border-slate-200/80 flex items-center justify-between text-xs text-slate-500">
              <span>Kiểm tra hệ thống: <code>morphix doctor</code></span>
              <span>Gỡ cài đặt: <code>morphix uninstall</code></span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
