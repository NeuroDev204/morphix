"use client";

import { useEffect } from "react";
import { ADSENSE } from "@/constants/ads";
import { adScriptSrc, isAdConfigured } from "@/lib/ads";

declare global {
  interface Window {
    adsbygoogle?: unknown[];
  }
}

export default function AdSlot() {
  const configured = isAdConfigured(ADSENSE);

  useEffect(() => {
    if (!configured) return;
    const src = adScriptSrc(ADSENSE.client);
    // Loaded on first show, not with the page, so visitors who never convert get no ad script.
    if (!document.querySelector(`script[src="${src}"]`)) {
      const s = document.createElement("script");
      s.async = true;
      s.src = src;
      s.crossOrigin = "anonymous";
      document.head.appendChild(s);
    }
    try {
      (window.adsbygoogle ||= []).push({});
    } catch {
      // StrictMode mounts twice in dev: the second push throws "already have ads".
    }
  }, [configured]);

  return (
    <div className="min-h-[250px] rounded-xl border border-slate-200 bg-white/60 p-2">
      <div className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider mb-1">Quảng cáo</div>
      {configured ? (
        <ins
          className="adsbygoogle"
          style={{ display: "block" }}
          data-ad-client={ADSENSE.client}
          data-ad-slot={ADSENSE.slot}
          data-ad-format="auto"
          data-full-width-responsive="true"
        />
      ) : (
        <div className="h-[220px] rounded-lg bg-slate-100 flex items-center justify-center text-xs text-slate-400">
          Chỗ đặt quảng cáo
        </div>
      )}
    </div>
  );
}
