"use client";

import { useEffect } from "react";
import { ADSENSE } from "@/constants/ads";

declare global {
  interface Window {
    adsbygoogle?: unknown[];
  }
}

// Renders nothing until both the publisher id and the slot id are filled in ads.ts.
export default function AdSlot() {
  const on = Boolean(ADSENSE.client && ADSENSE.slot);
  useEffect(() => {
    if (!on) return;
    try {
      (window.adsbygoogle = window.adsbygoogle || []).push({});
    } catch {
      // shortcut: a blocked or failed ad script is ignored, nothing to recover.
    }
  }, [on]);
  if (!on) return null;
  return (
    <div className="max-w-7xl mx-auto px-4 sm:px-8 py-6">
      <ins
        className="adsbygoogle block"
        style={{ display: "block" }}
        data-ad-client={ADSENSE.client}
        data-ad-slot={ADSENSE.slot}
        data-ad-format="auto"
        data-full-width-responsive="true"
      />
    </div>
  );
}
