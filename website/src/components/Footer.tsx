import { DOWNLOADS } from "@/constants/downloads";

function GithubIcon({ className = "w-4 h-4" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
      <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
    </svg>
  );
}

export default function Footer() {
  return (
    <footer className="border-t border-slate-800 bg-slate-950 text-slate-300 px-4 sm:px-8 py-14">
      <div className="max-w-7xl mx-auto grid grid-cols-1 md:grid-cols-4 gap-10">
        <div className="md:col-span-2 space-y-4">
          <div className="flex items-center gap-3 text-white">
            <div className="w-8 h-8 rounded-xl bg-teal-600 flex items-center justify-center text-white shadow-sm">
              <svg width="18" height="18" viewBox="0 0 512 512" fill="none">
                <path
                  d="M136 368V144L256 280L376 144V368"
                  stroke="currentColor"
                  strokeWidth="56"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </div>
            <span className="text-xl font-bold tracking-tight">Morphix</span>
          </div>
          <p className="text-xs text-slate-400 max-w-sm leading-relaxed">
            Ứng dụng đổi định dạng, nén và xử lý tệp tin đa phương tiện tức thì ngay trên máy của bạn. 100%
            On-device, không bao giờ gửi dữ liệu ra bên ngoài.
          </p>
          <div className="flex items-center gap-4 text-xs text-slate-400 pt-2">
            <a
              href={DOWNLOADS.githubRepo}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-white flex items-center gap-1.5 transition-colors"
            >
              <GithubIcon className="w-4 h-4" /> Mã nguồn GitHub
            </a>
            <span>·</span>
            <a
              href={`${DOWNLOADS.releasesPage}/tag/v${DOWNLOADS.version}`}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-white transition-colors"
            >
              Phiên bản v{DOWNLOADS.version}
            </a>
            <span>·</span>
            <span>GPL-3.0 License</span>
          </div>
        </div>

        <div>
          <h4 className="text-xs font-bold text-white uppercase tracking-wider mb-4">Điều hướng</h4>
          <ul className="space-y-2.5 text-xs text-slate-400">
            <li>
              <a href="#tinh-nang" className="hover:text-white transition-colors">
                Tính năng chính
              </a>
            </li>
            <li>
              <a href="#playground" className="hover:text-white transition-colors">
                Thử nghiệm file thật
              </a>
            </li>
            <li>
              <a href="#tiet-kiem" className="hover:text-white transition-colors">
                Ước tính dung lượng
              </a>
            </li>
            <li>
              <a href="#cli" className="hover:text-white transition-colors">
                CLI & Chẩn đoán
              </a>
            </li>
            <li>
              <a href="#tai-ve" className="hover:text-white transition-colors">
                Trung tâm tải về
              </a>
            </li>
          </ul>
        </div>

        <div>
          <h4 className="text-xs font-bold text-white uppercase tracking-wider mb-4">Hỗ trợ & Đóng góp</h4>
          <ul className="space-y-2.5 text-xs text-slate-400">
            <li>
              <a
                href={`${DOWNLOADS.githubRepo}/issues`}
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-white transition-colors"
              >
                Báo lỗi (Issue tracker)
              </a>
            </li>
            <li>
              <a
                href={`${DOWNLOADS.githubRepo}/blob/main/README.md`}
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-white transition-colors"
              >
                Hướng dẫn dòng lệnh CLI
              </a>
            </li>
            <li>
              <a
                href={`${DOWNLOADS.githubRepo}/blob/main/LICENSE`}
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-white transition-colors"
              >
                Giấy phép GPL-3.0
              </a>
            </li>
          </ul>
        </div>
      </div>

      <div className="max-w-7xl mx-auto mt-12 pt-6 border-t border-slate-800/80 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-slate-500">
        <p>© 2026 Morphix Project. Phát hành theo giấy phép GPL-3.0-or-later.</p>
        <p>FFmpeg và PDFium đi kèm tuân theo giấy phép mã nguồn mở tương ứng.</p>
      </div>
    </footer>
  );
}
