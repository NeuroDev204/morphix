import { Sparkles, Zap, ShieldCheck } from "lucide-react";

export default function MarqueeTicker() {
  const items = (
    <>
      <span className="flex items-center gap-2 text-teal-700">
        <Sparkles className="w-3.5 h-3.5" /> PNG sang WEBP
      </span>
      <span>•</span>
      <span>MP4 sang MP3</span>
      <span>•</span>
      <span>PDF sang MARKDOWN</span>
      <span>•</span>
      <span className="flex items-center gap-2 text-teal-700">
        <Zap className="w-3.5 h-3.5" /> AVIF NÉN 90%
      </span>
      <span>•</span>
      <span>NÉN 7Z LZMA2</span>
      <span>•</span>
      <span>VIDEO SANG GIF</span>
      <span>•</span>
      <span>DOCX SANG MD</span>
      <span>•</span>
      <span>TRÍCH XUẤT FLAC / WAV</span>
      <span>•</span>
      <span>GỘP ẢNH SANG PDF</span>
      <span>•</span>
      <span>PHỤ ĐỀ SRT SANG VTT</span>
      <span>•</span>
      <span className="flex items-center gap-2 text-teal-700">
        <ShieldCheck className="w-3.5 h-3.5" /> 0 BYTE CLOUD
      </span>
      <span>•</span>
    </>
  );

  return (
    <div className="border-y border-slate-200/80 bg-white py-3 overflow-hidden select-none">
      <div className="animate-marquee flex items-center gap-8 text-xs font-bold font-mono text-slate-500 uppercase tracking-widest">
        {items}
        {items}
      </div>
    </div>
  );
}
