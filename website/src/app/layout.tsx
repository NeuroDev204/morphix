import type { Metadata } from "next";
import { Plus_Jakarta_Sans, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const plusJakarta = Plus_Jakarta_Sans({
  subsets: ["latin", "vietnamese"],
  variable: "--font-plus-jakarta",
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-jetbrains-mono",
  display: "swap",
});

export const metadata: Metadata = {
  title: "Morphix — Đổi định dạng, nén & xử lý file 100% Offline (v0.1.4)",
  description:
    "Chuyển đổi định dạng, nén, tối ưu hoá ảnh, video, âm thanh, tài liệu ngay trên máy của bạn (Windows & Linux). 100% Offline, không gửi dữ liệu ra ngoài.",
  keywords: [
    "morphix",
    "offline converter",
    "file converter",
    "rust file tool",
    "nén file offline",
    "chuyển định dạng",
  ],
  authors: [{ name: "NeuroDev204" }],
  openGraph: {
    title: "Morphix — Đổi định dạng, nén & xử lý file 100% Offline",
    description:
      "Chuyển đổi định dạng, nén, tối ưu hoá ảnh, video, âm thanh, tài liệu ngay trên máy. 100% Offline, viết bằng Rust.",
    url: "https://morphix.app",
    siteName: "Morphix",
    locale: "vi_VN",
    type: "website",
  },
  icons: {
    icon: "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 512'><rect width='512' height='512' rx='96' fill='%230f766e'/><path d='M136 368V144L256 280L376 144V368' stroke='%23ffffff' stroke-width='48' stroke-linecap='square'/></svg>",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="vi" className={`scroll-smooth ${plusJakarta.variable} ${jetbrainsMono.variable}`}>
      <body className="antialiased selection:bg-teal-100 selection:text-teal-900 bg-grid-pattern font-sans">
        {children}
      </body>
    </html>
  );
}
