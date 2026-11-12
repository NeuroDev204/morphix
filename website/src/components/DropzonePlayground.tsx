"use client";

import { useState, useRef } from "react";
import { Sparkles, UploadCloud, Zap, CheckCircle2 } from "lucide-react";

interface FileAction {
  name: string;
  note: string;
}

interface LoadedFile {
  name: string;
  sizeMb: string;
  ext: string;
  actions: FileAction[];
}

export default function DropzonePlayground() {
  const [loadedFile, setLoadedFile] = useState<LoadedFile | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  const [activeMessage, setActiveMessage] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const processFile = (file: File) => {
    const name = file.name;
    const sizeMb = (file.size / (1024 * 1024)).toFixed(2);
    const ext = name.split(".").pop()?.toLowerCase() || "";

    let actions: FileAction[] = [
      { name: "Đổi sang WebP", note: "Giảm 80% dung lượng" },
      { name: "Nén nhanh vào 7Z", note: "LZMA2 siêu nén" },
      { name: "Xuất sang PDF", note: "Chuẩn in ấn" },
    ];

    if (["mp4", "mkv", "mov", "avi"].includes(ext)) {
      actions = [
        { name: "Trích xuất MP3", note: "320kbps Audio" },
        { name: "Tạo ảnh GIF", note: "Loop 10s cho web" },
        { name: "Nén video WebM", note: "Giảm 75% size" },
      ];
    }

    setLoadedFile({
      name,
      sizeMb,
      ext,
      actions,
    });
    setActiveMessage(null);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      processFile(e.dataTransfer.files[0]);
    }
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      processFile(e.target.files[0]);
    }
  };

  const handleReset = (e: React.MouseEvent) => {
    e.stopPropagation();
    setLoadedFile(null);
    setActiveMessage(null);
    if (fileInputRef.current) fileInputRef.current.value = "";
  };

  const handleRunSimulatedAction = (act: FileAction) => {
    if (!loadedFile) return;
    setActiveMessage(`Đã mô phỏng: ${act.name} cho tệp "${loadedFile.name}" thành công tức thì!`);
  };

  return (
    <section id="playground" className="py-16 sm:py-24 px-4 sm:px-8 max-w-7xl mx-auto">
      <div className="rounded-3xl border border-teal-200/80 bg-gradient-to-b from-teal-50/60 to-white p-6 sm:p-12 shadow-sm relative overflow-hidden">
        <div className="max-w-2xl mx-auto text-center space-y-3 mb-8">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-teal-100 text-teal-800 text-xs font-bold uppercase tracking-wider">
            <Sparkles className="w-3.5 h-3.5" /> Phòng thí nghiệm tương tác
          </div>
          <h2 className="text-2xl sm:text-4xl font-extrabold tracking-tight text-slate-950">
            Kéo thả thử file thật của bạn vào đây
          </h2>
          <p className="text-sm text-slate-600">
            Trình duyệt của bạn sẽ giả lập trực tiếp quá trình chuyển đổi Morphix với tốc độ xử lý on-device cục bộ.
          </p>
        </div>

        {/* Dropzone Container */}
        <div
          onClick={() => {
            if (!loadedFile && fileInputRef.current) {
              fileInputRef.current.click();
            }
          }}
          onDragOver={(e) => {
            e.preventDefault();
            setIsDragOver(true);
          }}
          onDragLeave={(e) => {
            e.preventDefault();
            setIsDragOver(false);
          }}
          onDrop={handleDrop}
          className={`max-w-2xl mx-auto border-2 border-dashed rounded-2xl p-8 sm:p-12 text-center transition-all cursor-pointer group shadow-xs ${
            isDragOver
              ? "border-teal-600 bg-teal-50/70"
              : "border-teal-400/70 bg-white/80 hover:bg-white hover:border-teal-500"
          }`}
        >
          <input
            ref={fileInputRef}
            type="file"
            onChange={handleFileChange}
            className="hidden"
            accept="image/*,video/*,audio/*,.pdf,.docx,.txt"
          />

          {!loadedFile ? (
            <div className="space-y-4">
              <div className="w-16 h-16 rounded-2xl bg-teal-50 text-teal-600 flex items-center justify-center mx-auto group-hover:scale-110 transition-transform shadow-xs">
                <UploadCloud className="w-8 h-8" />
              </div>
              <div>
                <div className="text-base font-bold text-slate-900">
                  Kéo & thả file ảnh, video hoặc tài liệu vào đây
                </div>
                <div className="text-xs text-slate-500 mt-1">
                  hoặc <span className="text-teal-700 font-semibold underline">chọn file từ máy tính</span> (ảnh, video, audio, pdf...)
                </div>
              </div>
              <div className="text-[11px] text-slate-400 font-mono">
                100% xử lý trong bộ nhớ trình duyệt · Không tải lên server
              </div>
            </div>
          ) : (
            <div className="space-y-4 text-left">
              <div className="p-4 rounded-xl bg-slate-50 border border-slate-200 flex items-center justify-between text-xs">
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 rounded-lg bg-teal-700 text-white flex items-center justify-center font-bold text-sm">
                    {loadedFile.ext ? loadedFile.ext.toUpperCase().slice(0, 3) : "FIL"}
                  </div>
                  <div>
                    <div className="font-bold text-slate-900 text-sm truncate max-w-[200px] sm:max-w-md">
                      {loadedFile.name}
                    </div>
                    <div className="text-slate-500 text-[11px]">
                      {loadedFile.sizeMb} MB · Tệp thực tế từ máy bạn
                    </div>
                  </div>
                </div>
                <span className="text-teal-700 font-semibold bg-teal-50 px-2.5 py-1 rounded-full border border-teal-200">
                  Đã nạp file
                </span>
              </div>

              {/* Actions Grid */}
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-2.5 pt-2">
                {loadedFile.actions.map((act) => (
                  <button
                    key={act.name}
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRunSimulatedAction(act);
                    }}
                    className="p-3 rounded-xl border border-teal-200 bg-white hover:bg-teal-50 text-left transition-all cursor-pointer group"
                  >
                    <div className="font-bold text-slate-900 group-hover:text-teal-700 flex items-center justify-between text-xs">
                      <span>{act.name}</span>
                      <Zap className="w-3.5 h-3.5 text-teal-600" />
                    </div>
                    <div className="text-[10px] text-slate-500 mt-0.5">{act.note}</div>
                  </button>
                ))}
              </div>

              {activeMessage && (
                <div className="p-3 rounded-xl bg-teal-50 border border-teal-300 text-teal-900 text-xs flex items-center gap-2">
                  <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
                  <span>{activeMessage}</span>
                </div>
              )}

              <div className="text-center pt-2">
                <button
                  type="button"
                  onClick={handleReset}
                  className="text-xs text-slate-500 hover:text-slate-800 underline cursor-pointer"
                >
                  Thử lại với file khác
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
