"use client";

import { useState, useRef, useEffect } from "react";
import { Sparkles, UploadCloud } from "lucide-react";
import {
  extOf,
  groupOf,
  targetsFor,
  sharedTargets,
  outputName,
  MAX_BYTES,
} from "@/lib/convert/catalog";
import {
  convertFile,
  canEncodeAvif,
  zipResults,
  resetEngines,
} from "@/lib/convert/engines";

interface SourceFileItem {
  id: string;
  file: File;
  ext: string;
  target: string;
  status: "idle" | "running" | "error" | "skipped";
  progressRatio: number | null;
  note?: string;
  errorMsg?: string;
}

interface ResultItem {
  id: string;
  name: string;
  url: string;
  blob: Blob;
  size: number;
  originalSize: number;
  ext: string;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

export default function DropzonePlayground() {
  const [sourceFiles, setSourceFiles] = useState<SourceFileItem[]>([]);
  const [results, setResults] = useState<ResultItem[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [isZipping, setIsZipping] = useState(false);
  const [hasAvif, setHasAvif] = useState(true);
  const [isDragOver, setIsDragOver] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const sessionRef = useRef(0);
  const takenNamesRef = useRef<Set<string>>(new Set());
  const createdUrlsRef = useRef<string[]>([]);

  useEffect(() => {
    canEncodeAvif().then((supported) => {
      setHasAvif(supported);
    });
  }, []);

  const getAvailableTargets = (ext: string): string[] => {
    const targets = targetsFor(ext);
    if (!hasAvif) {
      return targets.filter((t) => t !== "avif");
    }
    return targets;
  };

  const getSharedTargets = (exts: string[]): string[] | null => {
    const shared = sharedTargets(exts);
    if (!shared) return null;
    if (!hasAvif) {
      return shared.filter((t) => t !== "avif");
    }
    return shared;
  };

  const addFiles = (newFiles: FileList | File[]) => {
    const items: SourceFileItem[] = Array.from(newFiles).map((file) => {
      const ext = extOf(file.name);
      const available = getAvailableTargets(ext);
      return {
        id: `${file.name}_${file.size}_${Math.random().toString(36).slice(2, 7)}`,
        file,
        ext,
        target: available[0] || "",
        status: "idle",
        progressRatio: null,
      };
    });
    setSourceFiles((prev) => [...prev, ...items]);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      addFiles(e.dataTransfer.files);
    }
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      addFiles(e.target.files);
    }
    if (fileInputRef.current) {
      fileInputRef.current.value = "";
    }
  };

  const updateFileTarget = (id: string, target: string) => {
    setSourceFiles((prev) =>
      prev.map((f) => (f.id === id ? { ...f, target } : f))
    );
  };

  const handleReset = (e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    sessionRef.current += 1;
    resetEngines();
    for (const url of createdUrlsRef.current) {
      URL.revokeObjectURL(url);
    }
    createdUrlsRef.current = [];
    takenNamesRef.current.clear();
    setSourceFiles([]);
    setResults([]);
    setIsRunning(false);
    if (fileInputRef.current) {
      fileInputRef.current.value = "";
    }
  };

  const runConversionLoop = async (
    filesToProcess: { item: SourceFileItem; chosenTarget: string }[],
    currentSession: number
  ) => {
    setIsRunning(true);

    for (const { item, chosenTarget } of filesToProcess) {
      if (sessionRef.current !== currentSession) break;

      setSourceFiles((prev) =>
        prev.map((f) =>
          f.id === item.id
            ? { ...f, status: "running", progressRatio: null, note: "Đang bắt đầu..." }
            : f
        )
      );

      try {
        const blob = await convertFile(item.file, chosenTarget, (ratio, note) => {
          if (sessionRef.current !== currentSession) return;
          setSourceFiles((prev) =>
            prev.map((f) =>
              f.id === item.id ? { ...f, progressRatio: ratio, note } : f
            )
          );
        });

        if (sessionRef.current !== currentSession) {
          return;
        }

        const outName = outputName(item.file.name, chosenTarget, takenNamesRef.current);
        const url = URL.createObjectURL(blob);
        createdUrlsRef.current.push(url);

        setResults((prev) => [
          ...prev,
          {
            id: `${outName}_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
            name: outName,
            url,
            blob,
            size: blob.size,
            originalSize: item.file.size,
            ext: chosenTarget,
          },
        ]);

        setSourceFiles((prev) =>
          prev.map((f) =>
            f.id === item.id
              ? { ...f, status: "idle", progressRatio: 1, note: "Hoàn thành" }
              : f
          )
        );
      } catch (err: any) {
        if (sessionRef.current !== currentSession) return;
        setSourceFiles((prev) =>
          prev.map((f) =>
            f.id === item.id
              ? {
                  ...f,
                  status: "error",
                  errorMsg: err?.message || "Lỗi chuyển đổi",
                }
              : f
          )
        );
      }
    }

    if (sessionRef.current === currentSession) {
      setIsRunning(false);
    }
  };

  const handleConvertAllShared = async (target: string) => {
    if (isRunning) return;
    const currentSession = ++sessionRef.current;

    const filesToProcess: { item: SourceFileItem; chosenTarget: string }[] = [];

    for (const item of sourceFiles) {
      if (item.file.size > MAX_BYTES || groupOf(item.ext) === null) {
        continue;
      }
      const supported = getAvailableTargets(item.ext);
      if (!supported.includes(target)) {
        setSourceFiles((prev) =>
          prev.map((f) =>
            f.id === item.id
              ? {
                  ...f,
                  status: "skipped",
                  note: `Bỏ qua: không chuyển được sang ${target.toUpperCase()}`,
                }
              : f
          )
        );
      } else {
        filesToProcess.push({ item, chosenTarget: target });
      }
    }

    await runConversionLoop(filesToProcess, currentSession);
  };

  const handleConvertIndependent = async () => {
    if (isRunning) return;
    const currentSession = ++sessionRef.current;

    const filesToProcess: { item: SourceFileItem; chosenTarget: string }[] = [];

    for (const item of sourceFiles) {
      if (item.file.size > MAX_BYTES || groupOf(item.ext) === null) {
        continue;
      }
      const supported = getAvailableTargets(item.ext);
      const chosenTarget = item.target || supported[0];
      if (chosenTarget && supported.includes(chosenTarget)) {
        filesToProcess.push({ item, chosenTarget });
      }
    }

    await runConversionLoop(filesToProcess, currentSession);
  };

  const handleDownloadZip = async () => {
    if (results.length < 2 || isZipping) return;
    setIsZipping(true);
    try {
      const zipBlob = await zipResults(
        results.map((r) => ({ name: r.name, blob: r.blob }))
      );
      const url = URL.createObjectURL(zipBlob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "morphix.zip";
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      setTimeout(() => URL.revokeObjectURL(url), 60000);
    } catch (err) {
      console.error("Lỗi khi nén zip:", err);
    } finally {
      setIsZipping(false);
    }
  };

  const convertibleCount = sourceFiles.filter(
    (f) => groupOf(f.ext) !== null && f.file.size <= MAX_BYTES
  ).length;

  const sharedModeTargets =
    sourceFiles.length > 0
      ? getSharedTargets(sourceFiles.map((f) => f.ext))
      : null;

  return (
    <section id="playground" className="py-16 sm:py-24 px-4 sm:px-8 max-w-7xl mx-auto">
      <div className="rounded-3xl border border-teal-200/80 bg-gradient-to-b from-teal-50/60 to-white p-6 sm:p-12 shadow-sm relative overflow-hidden">
        <div className="max-w-2xl mx-auto text-center space-y-3 mb-8">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-teal-100 text-teal-800 text-xs font-bold uppercase tracking-wider">
            <Sparkles className="w-3.5 h-3.5" /> Phòng thí nghiệm tương tác
          </div>
          <h2 className="text-2xl sm:text-4xl font-extrabold tracking-tight text-slate-950">
            Kéo thả thử file thật của bạn vào đây
          </h2>
          <p className="text-sm text-slate-600">
            Trình duyệt của bạn chuyển đổi thật ngay trên máy. File không rời khỏi thiết bị.
          </p>
        </div>

        {/* Dropzone Container */}
        <div
          onClick={() => {
            if (sourceFiles.length === 0 && fileInputRef.current) {
              fileInputRef.current.click();
            }
          }}
          onDragOver={(e) => {
            e.preventDefault();
            setIsDragOver(true);
          }}
          onDragLeave={(e) => {
            e.preventDefault();
            setIsDragOver(false);
          }}
          onDrop={handleDrop}
          className={`max-w-2xl mx-auto border-2 border-dashed rounded-2xl p-6 sm:p-10 transition-all shadow-xs ${
            sourceFiles.length === 0 ? "cursor-pointer text-center group" : "text-left"
          } ${
            isDragOver
              ? "border-teal-600 bg-teal-50/70"
              : "border-teal-400/70 bg-white/80 hover:bg-white hover:border-teal-500"
          }`}
        >
          <input
            ref={fileInputRef}
            type="file"
            multiple
            onChange={handleFileChange}
            className="hidden"
          />

          {sourceFiles.length === 0 ? (
            <div className="space-y-4">
              <div className="w-16 h-16 rounded-2xl bg-teal-50 text-teal-600 flex items-center justify-center mx-auto group-hover:scale-110 transition-transform shadow-xs">
                <UploadCloud className="w-8 h-8" />
              </div>
              <div>
                <div className="text-base font-bold text-slate-900">
                  Kéo & thả file ảnh, video hoặc tài liệu vào đây
                </div>
                <div className="text-xs text-slate-500 mt-1">
                  hoặc{" "}
                  <span className="text-teal-700 font-semibold underline">
                    chọn file từ máy tính
                  </span>{" "}
                  (ảnh, video, audio, tài liệu...)
                </div>
              </div>
              <div className="text-[11px] text-slate-400 font-mono">
                100% xử lý trong bộ nhớ trình duyệt · Không tải lên server
              </div>
            </div>
          ) : (
            <div className="space-y-3">
              {/* Source Files List */}
              <div className="space-y-2">
                {sourceFiles.map((item) => {
                  const isUnsupported = groupOf(item.ext) === null;
                  const isTooLarge = item.file.size > MAX_BYTES;
                  const targets = getAvailableTargets(item.ext);

                  return (
                    <div
                      key={item.id}
                      className="flex items-center gap-2.5 p-2.5 sm:p-3 border border-slate-200 rounded-xl bg-slate-50 text-xs"
                    >
                      <div className="w-9 h-9 rounded-lg bg-teal-700 text-white flex items-center justify-center font-bold text-xs shrink-0 uppercase">
                        {item.ext ? item.ext.slice(0, 4) : "FIL"}
                      </div>

                      <div className="flex-1 min-w-0">
                        <b className="block text-slate-900 truncate">{item.file.name}</b>

                        {isTooLarge ? (
                          <span className="text-rose-600 text-[11px] font-medium block">
                            File quá lớn cho trình duyệt, hãy dùng app
                          </span>
                        ) : isUnsupported ? (
                          <span className="text-amber-700 text-[11px] font-medium block">
                            Chỉ có trong app ·{" "}
                            <a href="#tai-ve" className="underline hover:text-amber-900">
                              #tai-ve
                            </a>
                          </span>
                        ) : item.status === "error" ? (
                          <span className="text-rose-600 text-[11px] font-medium block">
                            {item.errorMsg || "Lỗi chuyển đổi"}
                          </span>
                        ) : item.status === "skipped" ? (
                          <span className="text-slate-500 text-[11px] block">
                            {item.note || "Đã bỏ qua"}
                          </span>
                        ) : item.status === "running" ? (
                          <div className="mt-1">
                            <span className="text-teal-700 text-[11px] font-medium block">
                              {item.note || "Đang chuyển đổi..."}{" "}
                              {item.progressRatio !== null
                                ? `${Math.round(item.progressRatio * 100)}%`
                                : ""}
                            </span>
                            <div className="w-full h-1 bg-slate-200 rounded-full mt-1 overflow-hidden">
                              <div
                                className="h-full bg-teal-500 rounded-full transition-all duration-200"
                                style={{
                                  width:
                                    item.progressRatio !== null
                                      ? `${Math.round(item.progressRatio * 100)}%`
                                      : "50%",
                                }}
                              />
                            </div>
                          </div>
                        ) : (
                          <span className="text-slate-500 text-[11px] block">
                            {formatSize(item.file.size)}
                            {item.note && ` · ${item.note}`}
                          </span>
                        )}
                      </div>

                      {/* Dropdown for independent mode */}
                      {!isTooLarge && !isUnsupported && sharedModeTargets === null && (
                        <select
                          value={item.target}
                          disabled={isRunning}
                          onChange={(e) => updateFileTarget(item.id, e.target.value)}
                          aria-label={`Chọn định dạng cho ${item.file.name}`}
                          className="border border-teal-300 rounded-lg px-2.5 py-1.5 bg-white text-teal-800 font-semibold text-xs focus:outline-teal-500 disabled:opacity-50 cursor-pointer"
                        >
                          {targets.map((t) => (
                            <option key={t} value={t}>
                              {t.toUpperCase()}
                            </option>
                          ))}
                        </select>
                      )}
                    </div>
                  );
                })}
              </div>

              {/* Conversion Triggers */}
              {sharedModeTargets !== null ? (
                <div className="pt-2">
                  <div className="text-[11px] font-bold text-slate-500 uppercase tracking-wider mb-2">
                    Chuyển tất cả sang
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {sharedModeTargets.map((target) => (
                      <button
                        key={target}
                        type="button"
                        disabled={isRunning}
                        onClick={() => handleConvertAllShared(target)}
                        className="border border-teal-300 hover:border-teal-600 bg-white hover:bg-teal-50 text-slate-900 rounded-lg px-3 py-1.5 text-xs font-bold transition-colors disabled:opacity-50 cursor-pointer"
                      >
                        {target.toUpperCase()}
                      </button>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="flex justify-end pt-2">
                  <button
                    type="button"
                    disabled={isRunning || convertibleCount === 0}
                    onClick={handleConvertIndependent}
                    className="bg-teal-700 hover:bg-teal-800 text-white font-bold text-xs sm:text-sm px-4 py-2.5 rounded-xl shadow-xs transition-colors disabled:opacity-50 cursor-pointer"
                  >
                    {isRunning ? "Đang xử lý..." : `Chuyển đổi ${convertibleCount} file`}
                  </button>
                </div>
              )}

              {/* Download Results Section */}
              {results.length > 0 && (
                <div className="pt-4">
                  <div className="text-[11px] font-bold text-slate-500 uppercase tracking-wider mb-2">
                    Đã xong: tải về
                  </div>
                  <div className="space-y-2">
                    {results.map((res) => {
                      const reduced =
                        res.originalSize > 0 && res.size < res.originalSize;
                      const pct = reduced
                        ? Math.round((1 - res.size / res.originalSize) * 100)
                        : 0;

                      return (
                        <div
                          key={res.id}
                          className="flex items-center gap-2.5 p-2.5 sm:p-3 border border-emerald-200 rounded-xl bg-emerald-50/60 text-xs"
                        >
                          <div className="w-9 h-9 rounded-lg bg-emerald-600 text-white flex items-center justify-center font-bold text-xs shrink-0 uppercase">
                            {res.ext.slice(0, 4)}
                          </div>
                          <div className="flex-1 min-w-0">
                            <b className="block text-slate-900 truncate">{res.name}</b>
                            <span className="text-slate-500 text-[11px] block">
                              {formatSize(res.size)}
                              {reduced && ` · giảm ${pct}%`}
                            </span>
                          </div>
                          <a
                            href={res.url}
                            download={res.name}
                            className="bg-emerald-100 hover:bg-emerald-200 text-emerald-800 border border-emerald-300 font-bold text-xs px-3 py-1.5 rounded-lg transition-colors whitespace-nowrap"
                          >
                            Tải về
                          </a>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Footer Controls */}
              <div className="flex items-center justify-between pt-4 mt-2 border-t border-slate-200/60 text-xs">
                <div className="flex items-center gap-3">
                  <button
                    type="button"
                    disabled={isRunning}
                    onClick={() => fileInputRef.current?.click()}
                    className="text-teal-700 font-semibold underline hover:text-teal-900 disabled:opacity-50 cursor-pointer"
                  >
                    Thêm file
                  </button>
                  <button
                    type="button"
                    onClick={() => handleReset()}
                    className="text-slate-500 hover:text-slate-800 underline cursor-pointer"
                  >
                    Làm lại với file khác
                  </button>
                </div>

                {results.length >= 2 && (
                  <button
                    type="button"
                    disabled={isZipping}
                    onClick={handleDownloadZip}
                    className="bg-teal-700 hover:bg-teal-800 text-white font-bold px-3.5 py-1.5 rounded-lg transition-colors cursor-pointer disabled:opacity-50"
                  >
                    {isZipping ? "Đang tạo zip..." : "Tải tất cả (.zip)"}
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
