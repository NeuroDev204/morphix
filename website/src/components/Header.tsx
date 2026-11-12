"use client";

import { useState } from "react";
import { Download, Menu, X } from "lucide-react";
import { DOWNLOADS } from "@/constants/downloads";

function GithubIcon({ className = "w-4 h-4" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
      <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
    </svg>
  );
}

export default function Header() {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  return (
    <header className="sticky top-0 z-50 bg-white/85 backdrop-blur-md border-b border-slate-200/80 transition-all duration-200">
      <div className="max-w-7xl mx-auto px-4 sm:px-8 h-16 flex items-center justify-between">
        {/* Brand Logo */}
        <a href="#" className="flex items-center gap-3 no-underline text-slate-900 group">
          <div className="w-9 h-9 rounded-xl bg-teal-700 flex items-center justify-center text-white shadow-md shadow-teal-900/10 group-hover:scale-105 group-hover:rotate-1 transition-all duration-200">
            <svg width="20" height="20" viewBox="0 0 512 512" fill="none">
              <path
                d="M136 368V144L256 280L376 144V368"
                stroke="currentColor"
                strokeWidth="56"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </div>
          <div className="flex flex-col">
            <span className="text-lg font-bold tracking-tight leading-none text-slate-900">Morphix</span>
            <span className="text-[10px] text-slate-500 font-medium uppercase tracking-wider mt-0.5">
              Offline File Utility
            </span>
          </div>
        </a>

        {/* Desktop Navigation */}
        <nav className="hidden md:flex items-center gap-7 text-sm font-semibold text-slate-600">
          <a href="#tinh-nang" className="hover:text-teal-700 transition-colors">
            Tính năng
          </a>
          <a href="#playground" className="hover:text-teal-700 transition-colors">
            Thử nghiệm
          </a>
          <a href="#dinh-dang" className="hover:text-teal-700 transition-colors">
            Định dạng
          </a>
          <a href="#tiet-kiem" className="hover:text-teal-700 transition-colors">
            Tính dung lượng
          </a>
          <a href="#cli" className="hover:text-teal-700 transition-colors">
            CLI & Doctor
          </a>
          <a href="#tai-ve" className="hover:text-teal-700 transition-colors">
            Tải v{DOWNLOADS.version}
          </a>
          <a
            href={DOWNLOADS.githubRepo}
            target="_blank"
            rel="noopener noreferrer"
            className="hover:text-teal-700 transition-colors flex items-center gap-1.5 text-slate-700"
          >
            <GithubIcon className="w-4 h-4" /> GitHub
          </a>
        </nav>

        {/* Right CTA Button & Mobile Hamburger */}
        <div className="flex items-center gap-3">
          <a
            href="#tai-ve"
            className="hidden sm:inline-flex items-center gap-2 h-10 px-5 rounded-xl bg-teal-700 text-white font-semibold text-xs uppercase tracking-wider shadow-sm shadow-teal-900/15 hover:bg-teal-800 active:bg-teal-900 hover:shadow-md transition-all"
          >
            <Download className="w-4 h-4" />
            <span>Tải v{DOWNLOADS.version}</span>
          </a>
          <button
            type="button"
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
            className="md:hidden p-2 rounded-xl text-slate-600 hover:bg-slate-100 transition-colors cursor-pointer"
            aria-label="Toggle menu"
          >
            {mobileMenuOpen ? <X className="w-6 h-6" /> : <Menu className="w-6 h-6" />}
          </button>
        </div>
      </div>

      {/* Mobile Drawer Menu */}
      {mobileMenuOpen && (
        <div className="md:hidden border-t border-slate-200 bg-white/95 backdrop-blur-lg px-6 py-5 shadow-xl transition-all">
          <nav className="flex flex-col gap-4 text-sm font-semibold text-slate-700">
            <a
              href="#tinh-nang"
              onClick={() => setMobileMenuOpen(false)}
              className="py-1 hover:text-teal-700 transition-colors"
            >
              Tính năng
            </a>
            <a
              href="#playground"
              onClick={() => setMobileMenuOpen(false)}
              className="py-1 hover:text-teal-700 transition-colors"
            >
              Thử nghiệm kéo thả
            </a>
            <a
              href="#dinh-dang"
              onClick={() => setMobileMenuOpen(false)}
              className="py-1 hover:text-teal-700 transition-colors"
            >
              Kho định dạng
            </a>
            <a
              href="#tiet-kiem"
              onClick={() => setMobileMenuOpen(false)}
              className="py-1 hover:text-teal-700 transition-colors"
            >
              Tính dung lượng tiết kiệm
            </a>
            <a
              href="#cli"
              onClick={() => setMobileMenuOpen(false)}
              className="py-1 hover:text-teal-700 transition-colors"
            >
              CLI & Doctor
            </a>
            <a
              href="#tai-ve"
              onClick={() => setMobileMenuOpen(false)}
              className="py-1 hover:text-teal-700 transition-colors"
            >
              Tải v{DOWNLOADS.version}
            </a>
            <a
              href={DOWNLOADS.githubRepo}
              target="_blank"
              rel="noopener noreferrer"
              className="py-1 flex items-center gap-2 text-slate-800"
            >
              <GithubIcon className="w-4 h-4" /> GitHub Repository
            </a>
            <div className="pt-3 border-t border-slate-200">
              <a
                href="#tai-ve"
                onClick={() => setMobileMenuOpen(false)}
                className="w-full inline-flex items-center justify-center gap-2 py-3 px-4 rounded-xl bg-teal-700 text-white font-semibold text-xs uppercase tracking-wider shadow-sm"
              >
                <Download className="w-4 h-4" />
                <span>Tải v{DOWNLOADS.version} cho máy</span>
              </a>
            </div>
          </nav>
        </div>
      )}
    </header>
  );
}
