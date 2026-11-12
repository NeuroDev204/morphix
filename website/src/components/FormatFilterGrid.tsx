"use client";

import { useState } from "react";
import { Image, Video, FileText, Archive } from "lucide-react";

type Category = "all" | "image" | "video" | "doc" | "archive";

interface FormatItem {
  id: string;
  category: "image" | "video" | "doc" | "archive";
  title: string;
  extensions: string;
  description: string;
  icon: typeof Image;
}

const FORMAT_CARDS: FormatItem[] = [
  {
    id: "img",
    category: "image",
    title: "Hình ảnh",
    extensions: "JPG, PNG, WebP, AVIF, GIF, TIFF, BMP, PDF",
    description:
      "Chuyển đổi đuôi ảnh, nén tối ưu WebP/AVIF cho website, đổi kích thước phần trăm/pixel, gộp nhiều ảnh thành một file PDF.",
    icon: Image,
  },
  {
    id: "vid",
    category: "video",
    title: "Video & Âm thanh",
    extensions: "MP4, MKV, WebM, MOV, AVI, MP3, WAV, FLAC, OGG, OPUS",
    description:
      "Trích xuất nhạc MP3 từ video, tạo ảnh động GIF từ đoạn video, đổi codec tiết kiệm dung lượng lưu trữ.",
    icon: Video,
  },
  {
    id: "doc",
    category: "doc",
    title: "Văn bản & Phụ đề",
    extensions: "PDF, DOCX, TXT, MD, SRT, VTT, CSV, TSV, HTML",
    description:
      "Trích xuất văn bản từ PDF sang TXT/Markdown, chuyển đổi phụ đề phim SRT/VTT, trích xuất bảng tính dữ liệu CSV.",
    icon: FileText,
  },
  {
    id: "arc",
    category: "archive",
    title: "Nén & Giải nén",
    extensions: "ZIP, TAR.GZ, 7Z",
    description:
      "Nén tệp siêu gọn với thuật toán 7-Zip LZMA2, tạo gói tarball chuẩn Linux cho lập trình viên và quản trị hệ thống.",
    icon: Archive,
  },
];

const TABS: { id: Category; label: string }[] = [
  { id: "all", label: "Tất cả (30+)" },
  { id: "image", label: "Hình ảnh" },
  { id: "video", label: "Video & Âm thanh" },
  { id: "doc", label: "Văn bản & PDF" },
  { id: "archive", label: "Nén 7Z/ZIP" },
];

export default function FormatFilterGrid() {
  const [activeFilter, setActiveFilter] = useState<Category>("all");

  const filteredCards =
    activeFilter === "all"
      ? FORMAT_CARDS
      : FORMAT_CARDS.filter((c) => c.category === activeFilter);

  return (
    <div id="dinh-dang" className="space-y-10 pt-4">
      <div className="text-center max-w-2xl mx-auto space-y-3">
        <div className="w-10 h-10 rounded-xl bg-teal-50 border border-teal-100 text-teal-700 font-bold text-base flex items-center justify-center mx-auto shadow-2xs">
          3
        </div>
        <h2 className="text-2xl sm:text-3xl lg:text-4xl font-extrabold tracking-tight text-slate-950">
          Hỗ trợ toàn diện hơn 30 định dạng tệp tin
        </h2>
        <p className="text-sm text-slate-600">
          Được tích hợp sẵn các bộ giải mã công nghiệp mạnh mẽ như FFmpeg và PDFium để xử lý trơn tru mọi loại media.
        </p>

        {/* Interactive Filter Tabs */}
        <div className="pt-4 flex flex-wrap items-center justify-center gap-2">
          {TABS.map((tab) => {
            const isActive = activeFilter === tab.id;
            return (
              <button
                key={tab.id}
                type="button"
                onClick={() => setActiveFilter(tab.id)}
                className={`px-4 py-1.5 rounded-full text-xs font-semibold cursor-pointer transition-all ${
                  isActive
                    ? "bg-teal-700 text-white shadow-2xs"
                    : "bg-white border border-slate-200 text-slate-700 hover:bg-slate-50"
                }`}
              >
                {tab.label}
              </button>
            );
          })}
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
        {filteredCards.map((card) => {
          const IconComponent = card.icon;
          return (
            <div
              key={card.id}
              className="p-6 rounded-3xl border border-slate-200/90 bg-white shadow-sm card-glow space-y-4 transition-all"
            >
              <div className="w-12 h-12 rounded-2xl bg-teal-50 text-teal-700 flex items-center justify-center">
                <IconComponent className="w-6 h-6" />
              </div>
              <div>
                <h3 className="font-bold text-slate-950 text-base">{card.title}</h3>
                <p className="text-xs text-slate-500 font-mono mt-1">{card.extensions}</p>
              </div>
              <p className="text-xs text-slate-600 leading-relaxed">
                {card.description}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
}
