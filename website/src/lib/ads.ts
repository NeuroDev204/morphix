export function shouldShowAd(isRunning: boolean, resultCount: number): boolean {
  return isRunning || resultCount > 0;
}

export function isAdConfigured(cfg: { client: string; slot: string }): boolean {
  return cfg.client.trim() !== "" && cfg.slot.trim() !== "";
}

export function adScriptSrc(client: string): string {
  return `https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=${encodeURIComponent(client)}`;
}
