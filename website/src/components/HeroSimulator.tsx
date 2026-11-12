"use client";

import { useState, useRef, useEffect } from "react";
import {
  MousePointer,
  Image as ImageIcon,
  Video as VideoIcon,
  FileText,
  Zap,
  ChevronRight,
  ArrowRight,
  Loader,
  Check,
} from "lucide-react";

type DemoKey = "image" | "video" | "doc";

interface ActionItem {
  label: string;
  ext: string;
  size: string;
  saving: string;
  fast: boolean;
}

interface DemoConfig {
  id: DemoKey;
  tabLabel: string;
  name: string;
  meta: string;
  icon: typeof ImageIcon;
  actions: ActionItem[];
}

const DEMOS: Record<DemoKey, DemoConfig> = {
  image: {
    id: "image",
    tabLabel: "Anh.png",
    name: "hinh-anh-banner.png",
    meta: "14.8 MB · 3840 x 2160 · PNG",
    icon: ImageIcon,
    actions: [
      {
        label: "Chuyển sang WebP (giảm 87% dung lượng)",
        ext: ".webp",
        size: "1.9 MB",
        saving: "giảm 87%",
        fast: true,
      },
      {
        label: "Chuyển sang AVIF chất lượng cao",
        ext: ".avif",
        size: "1.4 MB",
        saving: "giảm 90%",
        fast: false,
      },
      {
        label: "Đổi kích thước 50% (1920x1080)",
        ext: "-resized.png",
        size: "3.2 MB",
        saving: "giảm 78%",
        fast: false,
      },
    ],
  },
  video: {
    id: "video",
    tabLabel: "Video.mov",
    name: "podcast-quay-man-hinh.mov",
    meta: "248.5 MB · 1080p 60fps · ProRes",
    icon: VideoIcon,
    actions: [
      {
        label: "Trích xuất âm thanh MP3 (320kbps)",
        ext: ".mp3",
        size: "16.4 MB",
        saving: "giảm 93%",
        fast: true,
      },
      {
        label: "Đổi sang video MP4 (H.264 nhẹ)",
        ext: ".mp4",
        size: "38.2 MB",
        saving: "giảm 85%",
        fast: false,
      },
      {
        label: "Tạo ảnh động GIF ngắn (10s)",
        ext: ".gif",
        size: "8.1 MB",
        saving: "tối ưu web",
        fast: false,
      },
    ],
  },
  doc: {
    id: "doc",
    tabLabel: "BaoCao.docx",
    name: "hop-dong-bao-cao.docx",
    meta: "5.4 MB · 28 trang · Microsoft Word",
    icon: FileText,
    actions: [
      {
        label: "Chuyển sang Markdown (.md)",
        ext: ".md",
        size: "42 KB",
        saving: "siêu nhẹ",
        fast: true,
      },
      {
        label: "Trích xuất toàn bộ văn bản (.txt)",
        ext: ".txt",
        size: "31 KB",
        saving: "thuần text",
        fast: false,
      },
      {
        label: "Nén nhanh vào kho lưu 7Z",
        ext: ".7z",
        size: "1.1 MB",
        saving: "mã hoá an toàn",
        fast: false,
      },
    ],
  },
};

export default function HeroSimulator() {
  const [activeTab, setActiveTab] = useState<DemoKey>("image");
  const [isProcessing, setIsProcessing] = useState(false);
  const [progress, setProgress] = useState(0);
  const [toastData, setToastData] = useState<{
    fileName: string;
    size: string;
    saving: string;
  } | null>({
    fileName: "hinh-anh-banner.webp",
    size: "1.9 MB",
    saving: "giảm 87%",
  });

  const timerRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, []);

  const currentDemo = DEMOS[activeTab];
  const IconComponent = currentDemo.icon;

  const handleSelectTab = (key: DemoKey) => {
    setActiveTab(key);
    // Reset toast to default for the selected tab
    const demo = DEMOS[key];
    const baseName = demo.name.substring(0, demo.name.lastIndexOf("."));
    setToastData({
      fileName: `${baseName}${demo.actions[0].ext}`,
      size: demo.actions[0].size,
      saving: demo.actions[0].saving,
    });
    setIsProcessing(false);
    setProgress(0);
  };

  const handleRunAction = (action: ActionItem) => {
    if (isProcessing) return;
    setIsProcessing(true);
    setProgress(0);
    setToastData(null);

    let p = 0;
    if (timerRef.current) clearInterval(timerRef.current);

    timerRef.current = setInterval(() => {
      p += Math.floor(Math.random() * 25) + 20;
      if (p >= 100) {
        p = 100;
        if (timerRef.current) clearInterval(timerRef.current);
        setProgress(100);
        setTimeout(() => {
          setIsProcessing(false);
          const baseName = currentDemo.name.substring(
            0,
            currentDemo.name.lastIndexOf(".")
          );
          setToastData({
            fileName: `${baseName}${action.ext}`,
            size: action.size,
            saving: action.saving,
          });
        }, 150);
      } else {
        setProgress(p);
      }
    }, 45);
  };

  return (
    <div className="relative">
      {/* Ambient Glow under the mockup */}
      <div className="absolute -inset-2 bg-gradient-to-r from-teal-500/20 to-emerald-500/20 rounded-3xl blur-xl opacity-75" />

      <div className="relative rounded-2xl border border-slate-800 bg-slate-900/95 backdrop-blur-md text-slate-100 p-5 shadow-2xl overflow-hidden font-sans">
        {/* Window Title bar */}
        <div className="flex items-center justify-between pb-3.5 border-b border-slate-800 text-xs text-slate-400">
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-rose-500/90 shadow-2xs" />
            <div className="w-3 h-3 rounded-full bg-amber-500/90 shadow-2xs" />
            <div className="w-3 h-3 rounded-full bg-emerald-500/90 shadow-2xs" />
          </div>
          <div className="flex items-center gap-1.5 text-[11px] font-mono text-slate-400">
            <MousePointer className="w-3 h-3 text-teal-400" />
            <span>Mô phỏng chuột phải (Bấm để thử)</span>
          </div>
          <div className="w-6" />
        </div>

        {/* Interactive File Tabs */}
        <div className="pt-3.5 pb-2">
          <div className="text-[10px] font-bold text-slate-400 uppercase tracking-wider mb-2 flex items-center justify-between">
            <span>Chọn tập tin mẫu:</span>
            <span className="text-teal-400 font-normal">Click để đổi file</span>
          </div>
          <div className="grid grid-cols-3 gap-2 text-xs">
            {(["image", "video", "doc"] as DemoKey[]).map((key) => {
              const d = DEMOS[key];
              const TabIcon = d.icon;
              const isActive = activeTab === key;
              return (
                <button
                  key={key}
                  type="button"
                  onClick={() => handleSelectTab(key)}
                  className={`p-2 rounded-lg font-medium text-left flex items-center gap-2 transition-all cursor-pointer ${
                    isActive
                      ? "bg-teal-950/80 border border-teal-500 text-white shadow-2xs"
                      : "bg-slate-800/80 border border-slate-700 text-slate-300 hover:bg-slate-800"
                  }`}
                >
                  <TabIcon
                    className={`w-4 h-4 shrink-0 ${
                      key === "image"
                        ? "text-teal-400"
                        : key === "video"
                        ? "text-blue-400"
                        : "text-amber-400"
                    }`}
                  />
                  <span className="truncate text-[11px]">{d.tabLabel}</span>
                </button>
              );
            })}
          </div>
        </div>

        {/* File details card */}
        <div className="p-3 my-2 rounded-xl bg-slate-800/90 border border-slate-700/80 flex items-center justify-between text-xs">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-lg bg-teal-500/20 text-teal-300 flex items-center justify-center shrink-0">
              <IconComponent className="w-4 h-4" />
            </div>
            <div>
              <div className="font-bold text-white text-xs">{currentDemo.name}</div>
              <div className="text-[11px] text-slate-400 font-mono">
                {currentDemo.meta}
              </div>
            </div>
          </div>
          <span className="text-[10px] text-teal-300 font-semibold uppercase px-2 py-0.5 rounded bg-teal-950/80 border border-teal-500/30">
            Chuột phải
          </span>
        </div>

        {/* Context Submenu */}
        <div className="p-2 rounded-xl bg-slate-800/95 border border-slate-700 shadow-xl space-y-1 text-xs">
          <div className="px-2.5 py-1 text-[10px] font-bold text-slate-400 uppercase tracking-wider border-b border-slate-700/80 flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <div className="w-3.5 h-3.5 rounded bg-teal-700 text-white flex items-center justify-center text-[9px] font-bold">
                M
              </div>
              Menu Morphix
            </div>
            <span className="text-[10px] text-teal-400">Bấm tác vụ để chạy thử:</span>
          </div>

          <div className="space-y-1 pt-1">
            {currentDemo.actions.map((act, index) => {
              const isFirst = index === 0;
              return (
                <button
                  key={act.label}
                  type="button"
                  onClick={() => handleRunAction(act)}
                  disabled={isProcessing}
                  className={`w-full p-2 rounded-lg text-left text-xs font-medium flex items-center justify-between cursor-pointer transition-all ${
                    isFirst
                      ? "bg-teal-900/60 text-white border border-teal-500/50 hover:bg-teal-900/80"
                      : "text-slate-300 hover:bg-slate-700/60 hover:text-white"
                  }`}
                >
                  <div className="flex items-center gap-2">
                    {isFirst ? (
                      <Zap className="w-3.5 h-3.5 text-teal-400" />
                    ) : (
                      <ChevronRight className="w-3.5 h-3.5 text-slate-500" />
                    )}
                    <span>{act.label}</span>
                  </div>
                  <ArrowRight
                    className={`w-3.5 h-3.5 ${
                      isFirst ? "text-teal-300" : "text-slate-500"
                    }`}
                  />
                </button>
              );
            })}
          </div>
        </div>

        {/* Simulated Progress */}
        {isProcessing && (
          <div className="mt-3 p-3 rounded-xl bg-slate-800 border border-teal-500/50 space-y-2 text-xs">
            <div className="flex items-center justify-between text-[11px]">
              <span className="text-teal-300 font-medium flex items-center gap-1.5">
                <Loader className="w-3.5 h-3.5 animate-spin text-teal-400" />
                Đang xử lý on-device bằng động cơ Rust...
              </span>
              <span className="font-mono text-white">{progress}%</span>
            </div>
            <div className="w-full h-1.5 rounded-full bg-slate-700 overflow-hidden">
              <div
                className="h-full bg-teal-400 transition-all duration-150"
                style={{ width: `${progress}%` }}
              />
            </div>
          </div>
        )}

        {/* Success Toast */}
        {!isProcessing && toastData && (
          <div className="mt-3 p-3 rounded-xl bg-teal-950/90 border border-teal-500/70 text-teal-200 flex items-start gap-3 shadow-lg">
            <div className="w-7 h-7 rounded-lg bg-teal-500/30 text-teal-300 flex items-center justify-center shrink-0">
              <Check className="w-4 h-4 text-teal-300" />
            </div>
            <div className="space-y-0.5">
              <div className="text-xs font-bold text-white flex items-center gap-2">
                <span>Morphix hoàn tất tức thì</span>
                <span className="text-[10px] font-normal px-1.5 py-0.2 rounded bg-teal-900 text-teal-300 font-mono">
                  18ms
                </span>
              </div>
              <div className="text-[11px] text-teal-300/90 leading-tight">
                Đã tạo{" "}
                <code className="text-white font-mono bg-teal-900/90 px-1 py-0.5 rounded">
                  {toastData.fileName}
                </code>{" "}
                ({toastData.size}) {toastData.saving}, nằm cạnh file gốc.
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
