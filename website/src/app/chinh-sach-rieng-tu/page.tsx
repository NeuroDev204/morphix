import type { Metadata } from "next";
import { DOWNLOADS } from "@/constants/downloads";

export const metadata: Metadata = {
  title: "Chính sách quyền riêng tư — Morphix",
};

const link = "text-teal-700 font-semibold underline hover:text-teal-900";

export default function PrivacyPolicy() {
  return (
    <main className="max-w-2xl mx-auto px-4 sm:px-6 py-12 sm:py-16 text-slate-700 text-sm sm:text-base leading-relaxed">
      <a href="/" className={link}>
        ← Về trang chủ
      </a>
      <h1 className="mt-6 text-2xl sm:text-3xl font-extrabold tracking-tight text-slate-950">
        Chính sách quyền riêng tư
      </h1>

      <h2 className="mt-8 text-lg font-bold text-slate-900">Ứng dụng Morphix</h2>
      <p className="mt-2">
        Ứng dụng Morphix (Windows, Linux) không thu thập dữ liệu, không chứa mã theo dõi (analytics) và không
        gửi file của bạn đi đâu. Mọi xử lý diễn ra trên máy của bạn.
      </p>

      <h2 className="mt-8 text-lg font-bold text-slate-900">Khối thử đổi file trên website</h2>
      <p className="mt-2">
        Khối thử đổi file xử lý file ngay trong trình duyệt của bạn. File không được tải lên máy chủ nào.
      </p>

      <h2 className="mt-8 text-lg font-bold text-slate-900">Quảng cáo trên website</h2>
      <p className="mt-2">
        Website này hiển thị quảng cáo của Google AdSense. Google và các đối tác có thể dùng cookie để hiển thị
        quảng cáo dựa trên các lần bạn truy cập website này và các website khác. Mã quảng cáo chỉ được tải khi
        bạn bấm chuyển đổi file.
      </p>
      <p className="mt-2">
        Tìm hiểu cách Google dùng dữ liệu:{" "}
        <a href="https://policies.google.com/technologies/partner-sites" target="_blank" rel="noopener noreferrer" className={link}>
          policies.google.com/technologies/partner-sites
        </a>
        . Tắt quảng cáo cá nhân hoá:{" "}
        <a href="https://myadcenter.google.com" target="_blank" rel="noopener noreferrer" className={link}>
          myadcenter.google.com
        </a>
        .
      </p>

      <h2 className="mt-8 text-lg font-bold text-slate-900">Liên hệ</h2>
      <p className="mt-2">
        Có câu hỏi về quyền riêng tư? Gửi qua{" "}
        <a href={`${DOWNLOADS.githubRepo}/issues`} target="_blank" rel="noopener noreferrer" className={link}>
          Issue tracker trên GitHub
        </a>
        .
      </p>
    </main>
  );
}
