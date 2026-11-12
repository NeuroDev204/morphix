"use client";

import { useState } from "react";
import { ChevronDown } from "lucide-react";

interface FaqItem {
  id: string;
  question: string;
  answer: React.ReactNode;
}

const FAQ_ITEMS: FaqItem[] = [
  {
    id: "need-rust",
    question: "Máy tôi không cài đặt Rust hoặc không biết lập trình thì có dùng được không?",
    answer: (
      <>
        Hoàn toàn dùng được bình thường! Rust là ngôn ngữ biên dịch trực tiếp ra mã máy native độc lập (giống như C/C++).
        Bản cài đặt <code>MorphixSetup.exe</code> trên Windows và các gói Linux (<code>.deb</code>, <code>tarball</code>, lệnh <code>curl</code>) đã được đóng gói và biên dịch sẵn toàn bộ.
        Bạn <strong>không cần cài Rust</strong>, không cần Cargo, và không cần bất kỳ môi trường lập trình nào trên máy.
      </>
    ),
  },
  {
    id: "safe",
    question: "File của tôi có thực sự được an toàn 100% không?",
    answer: (
      <>
        Hoàn toàn chính xác. Morphix không chứa bất kỳ mã theo dõi (analytics), không có máy chủ nhận file,
        và không yêu cầu đăng nhập. Toàn bộ thuật toán chuyển đổi chạy trực tiếp bằng CPU và lưu trữ trên ổ
        cứng của bạn.
      </>
    ),
  },
  {
    id: "smartscreen",
    question: "Tại sao lần đầu mở trên Windows xuất hiện cảnh báo SmartScreen?",
    answer: (
      <>
        Vì Morphix là phần mềm mã nguồn mở phi lợi nhuận nên chưa mua chứng chỉ ký số (EV Code Signing) đắt
        tiền của Microsoft. Bạn chỉ cần bấm <strong>More info</strong> rồi chọn <strong>Run anyway</strong>.
        Bạn có thể tự đối chiếu mã băm SHA256 trên GitHub Release để đảm bảo file nguyên vẹn.
      </>
    ),
  },
  {
    id: "uninstall",
    question: "Làm cách nào để gỡ cài đặt sạch sẽ?",
    answer: (
      <>
        Trên Windows: Vào <em>Settings → Apps</em> gỡ bình thường. Trên Linux: Chạy lệnh{" "}
        <code className="bg-slate-100 text-teal-800 px-1 py-0.5 rounded font-mono">
          morphix uninstall
        </code>{" "}
        trong terminal để tự động dọn sạch các mục menu chuột phải trong Nautilus, Dolphin, Thunar.
      </>
    ),
  },
];

export default function FaqAccordion() {
  const [openId, setOpenId] = useState<string | null>(null);

  const toggle = (id: string) => {
    setOpenId(openId === id ? null : id);
  };

  return (
    <section className="py-16 sm:py-24 px-4 sm:px-8 max-w-4xl mx-auto space-y-8">
      <div className="text-center space-y-3">
        <h2 className="text-2xl sm:text-3xl font-extrabold tracking-tight text-slate-950">
          Câu hỏi thường gặp
        </h2>
        <p className="text-xs sm:text-sm text-slate-600">
          Những thắc mắc phổ biến về cơ chế hoạt động và cài đặt Morphix.
        </p>
      </div>

      <div className="space-y-3">
        {FAQ_ITEMS.map((item) => {
          const isOpen = openId === item.id;
          return (
            <div
              key={item.id}
              className="rounded-2xl border border-slate-200/90 bg-white overflow-hidden shadow-2xs transition-all"
            >
              <button
                type="button"
                onClick={() => toggle(item.id)}
                className="w-full p-5 text-left font-bold text-sm text-slate-900 flex items-center justify-between hover:bg-slate-50/60 transition-colors cursor-pointer"
              >
                <span>{item.question}</span>
                <ChevronDown
                  className={`w-4 h-4 text-slate-400 transition-transform duration-200 ${
                    isOpen ? "rotate-180" : ""
                  }`}
                />
              </button>
              {isOpen && (
                <div className="px-5 pb-5 text-xs text-slate-600 leading-relaxed border-t border-slate-100 pt-3">
                  {item.answer}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
