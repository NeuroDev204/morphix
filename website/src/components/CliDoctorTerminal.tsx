"use client";

import { useState } from "react";
import { Terminal } from "lucide-react";

type CliTabKey = "doctor" | "convert" | "audio";

interface CommandDetail {
  id: CliTabKey;
  tabLabel: string;
  command: string;
  output: React.ReactNode;
}

const COMMANDS: Record<CliTabKey, CommandDetail> = {
  doctor: {
    id: "doctor",
    tabLabel: "morphix doctor",
    command: "morphix doctor",
    output: (
      <div className="space-y-2">
        <div>
          <span className="text-teal-400 font-bold">$ morphix doctor</span>
        </div>
        <div className="text-slate-400">
          Đang kiểm tra môi trường và tích hợp menu chuột phải...
        </div>
        <div className="pt-2 space-y-1">
          <div>
            <span className="text-emerald-400 font-semibold">✔ Động cơ Rust Core:</span> v0.1.4 (x86_64-unknown-linux-gnu)
          </div>
          <div>
            <span className="text-emerald-400 font-semibold">✔ Thư viện đồ hoạ:</span> WebKit2GTK 4.1 đã phát hiện
          </div>
          <div>
            <span className="text-emerald-400 font-semibold">✔ Trình quản lý Nautilus:</span> nautilus-python đã kích hoạt menu động
          </div>
          <div>
            <span className="text-emerald-400 font-semibold">✔ Trình quản lý Dolphin / Thunar:</span> Tệp service menu đã cấu hình
          </div>
          <div>
            <span className="text-emerald-400 font-semibold">✔ Đa phương tiện FFmpeg 7:</span> Tích hợp sẵn trong bundle
          </div>
          <div>
            <span className="text-emerald-400 font-semibold">✔ Trình đọc PDFium:</span> Đã sẵn sàng
          </div>
        </div>
        <div className="pt-2 text-teal-300 font-bold">
          Trạng thái: Hoàn hảo. Tất cả các menu chuột phải đang hoạt động không cần sudo.
        </div>
      </div>
    ),
  },
  convert: {
    id: "convert",
    tabLabel: "convert --to webp",
    command: "morphix convert --to webp anh-chup-may-anh.png",
    output: (
      <div className="space-y-2">
        <div>
          <span className="text-teal-400 font-bold">
            $ morphix convert --to webp anh-chup-may-anh.png
          </span>
        </div>
        <div className="text-slate-400">[1/2] Đang giải mã ảnh PNG 48.2 MB...</div>
        <div className="text-slate-400">[2/2] Mã hoá WebP đa luồng với 16 nhân CPU...</div>
        <div className="pt-2 space-y-1">
          <div className="text-emerald-400 font-semibold">Thành công trong 0.28s:</div>
          <div className="pl-3 text-slate-300">Gốc: &nbsp; anh-chup-may-anh.png &nbsp; (48.2 MB)</div>
          <div className="pl-3 text-slate-300">
            Đích: &nbsp; anh-chup-may-anh.webp &nbsp; (4.1 MB){" "}
            <span className="text-teal-300 font-bold">[-91.5%]</span>
          </div>
        </div>
        <div className="text-slate-500 text-[11px] pt-1">
          File kết quả nằm tại cùng thư mục, file gốc được bảo toàn.
        </div>
      </div>
    ),
  },
  audio: {
    id: "audio",
    tabLabel: "extract --audio",
    command: "morphix extract --audio --bitrate 320k video-hoi-thao.mp4",
    output: (
      <div className="space-y-2">
        <div>
          <span className="text-teal-400 font-bold">
            $ morphix extract --audio --bitrate 320k video-hoi-thao.mp4
          </span>
        </div>
        <div className="text-slate-400">
          [1/2] Đọc luồng âm thanh AAC từ container MP4 (1.2 GB)...
        </div>
        <div className="text-slate-400">
          [2/2] Xuất file MP3 stereo 48kHz 320kbps không nén lại video...
        </div>
        <div className="pt-2 space-y-1">
          <div className="text-emerald-400 font-semibold">Hoàn tất trong 1.14s:</div>
          <div className="pl-3 text-slate-300">Đã tạo: video-hoi-thao.mp3 (112.4 MB)</div>
        </div>
        <div className="text-teal-300 text-[11px] pt-1">
          Đã gửi thông báo hệ thống: &quot;Morphix: Trích xuất audio thành công&quot;
        </div>
      </div>
    ),
  },
};

export default function CliDoctorTerminal() {
  const [activeTab, setActiveTab] = useState<CliTabKey>("doctor");

  return (
    <section id="cli" className="py-16 sm:py-24 px-4 sm:px-8 max-w-7xl mx-auto">
      <div className="rounded-3xl border border-slate-800 bg-slate-950 text-slate-100 p-6 sm:p-12 shadow-2xl space-y-8">
        <div className="max-w-2xl mx-auto text-center space-y-3">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-slate-800 text-teal-300 text-xs font-mono font-semibold">
            <Terminal className="w-3.5 h-3.5" /> Dành cho Developer & Power User
          </div>
          <h2 className="text-2xl sm:text-3xl font-extrabold tracking-tight text-white">
            Điều khiển bằng dòng lệnh CLI & Công cụ chẩn đoán
          </h2>
          <p className="text-xs sm:text-sm text-slate-400">
            Ngoài menu chuột phải, Morphix cung cấp CLI siêu nhanh và lệnh <code>morphix doctor</code> để
            kiểm tra tính tương thích.
          </p>
        </div>

        {/* Terminal Interactive Box */}
        <div className="max-w-3xl mx-auto rounded-2xl border border-slate-800 bg-slate-900 shadow-xl overflow-hidden font-mono text-xs">
          {/* Terminal Tab Bar */}
          <div className="flex items-center justify-between px-4 py-3 bg-slate-950 border-b border-slate-800">
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 rounded-full bg-rose-500" />
              <div className="w-3 h-3 rounded-full bg-amber-500" />
              <div className="w-3 h-3 rounded-full bg-emerald-500" />
              <span className="ml-2 text-slate-400 text-[11px]">bash — neuro@morphix-pc:~</span>
            </div>

            {/* Command Switcher */}
            <div className="flex items-center gap-1.5 text-[11px]">
              {(["doctor", "convert", "audio"] as CliTabKey[]).map((tabKey) => {
                const isActive = activeTab === tabKey;
                return (
                  <button
                    key={tabKey}
                    type="button"
                    onClick={() => setActiveTab(tabKey)}
                    className={`px-2.5 py-1 rounded cursor-pointer transition-all ${
                      isActive
                        ? "bg-teal-900/80 text-teal-200 border border-teal-500/50"
                        : "bg-slate-800 text-slate-400 hover:text-white"
                    }`}
                  >
                    {COMMANDS[tabKey].tabLabel}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Terminal Content Area */}
          <div className="p-5 space-y-3 leading-relaxed">
            {COMMANDS[activeTab].output}
          </div>
        </div>
      </div>
    </section>
  );
}
