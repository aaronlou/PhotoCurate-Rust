import { useEffect, useMemo, useRef, useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import {
  buildSearchIndex,
  cancelSearchIndexing,
  checkLocalModel,
  getAiSettings,
  getIndexStats,
  naturalLanguageSearch,
  rebuildAllIndex,
} from "@/hooks/useInvoke";
import type { Directory, SearchResult } from "@/types";
import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Database,
  FolderOpen,
  Image as ImageIcon,
  Search,
  Sparkles,
  Square,
  WandSparkles,
  Wrench,
} from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

function directoryName(path: string) {
  const normalized = path.replace(/\/+$/, "");
  return normalized.split("/").pop() || normalized || path;
}

function similarityBadgeClass(similarity: number): string {
  if (similarity >= 0.7) {
    return "bg-emerald-500/90 text-white";
  }
  if (similarity >= 0.5) {
    return "bg-blue-500/90 text-white";
  }
  if (similarity >= 0.3) {
    return "bg-gray-500/70 text-white";
  }
  return "bg-gray-400/60 text-white";
}

type FolderIndexSummary = {
  directory: Directory;
  total: number;
  indexed: number;
  pending: number;
  pendingPhotoIds: string[];
};

export default function SearchView() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchNotice, setSearchNotice] = useState<string | null>(null);
  const [indexStats, setIndexStats] = useState<{ count: number; dimension: number | null } | null>(null);
  const [showDiagnostics, setShowDiagnostics] = useState(false);
  const [showIndexPanel, setShowIndexPanel] = useState(false);
  const [activeIndexDirectoryId, setActiveIndexDirectoryId] = useState<string | null>(null);
  const [isRebuilding, setIsRebuilding] = useState(false);
  const [localModelAvailable, setLocalModelAvailable] = useState<boolean | null>(null);
  const [hasGeminiKey, setHasGeminiKey] = useState(false);
  const [allowSavedKeyRead, setAllowSavedKeyRead] = useState(false);
  const cancelRequestedRef = useRef(false);

  const selectedPhoto = useAppStore((s) => s.selectedPhoto);
  const setSelectedPhoto = useAppStore((s) => s.setSelectedPhoto);
  const photos = useAppStore((s) => s.photos);
  const directories = useAppStore((s) => s.directories);
  const loadLibrary = useAppStore((s) => s.loadLibrary);
  const isIndexing = useAppStore((s) => s.isIndexing);
  const indexProgress = useAppStore((s) => s.indexProgress);

  const indexedCount = photos.filter((p) => p.has_embedding).length;
  const totalPhotos = photos.length;
  const unindexedPhotos = photos.filter((p) => !p.has_embedding);
  const pendingIndexCount = totalPhotos - indexedCount;

  const folderSummaries = useMemo<FolderIndexSummary[]>(() => {
    return directories.map((directory) => {
      const folderPhotos = photos.filter((photo) => photo.directory_id === directory.id);
      const pending = folderPhotos.filter((photo) => !photo.has_embedding);
      return {
        directory,
        total: folderPhotos.length,
        indexed: folderPhotos.length - pending.length,
        pending: pending.length,
        pendingPhotoIds: pending.map((photo) => photo.id),
      };
    });
  }, [directories, photos]);

  const highRelevance = results.filter((r) => r.similarity >= 0.5);
  const moreReference = results.filter((r) => r.similarity < 0.5);
  const needsSavedGeminiKey = localModelAvailable === false && hasGeminiKey;
  const forceIndexPanelOpen = totalPhotos > 0 && indexedCount === 0;
  const indexPanelOpen = showIndexPanel || forceIndexPanelOpen;
  const indexPercent = totalPhotos > 0 ? Math.round((indexedCount / totalPhotos) * 100) : 0;
  const pendingFolderCount = folderSummaries.filter((summary) => summary.pending > 0).length;

  useEffect(() => {
    getIndexStats()
      .then(setIndexStats)
      .catch(() => setIndexStats(null));
  }, [indexedCount, isIndexing]);

  useEffect(() => {
    loadLibrary().catch(console.error);
    checkLocalModel().then(setLocalModelAvailable).catch(() => setLocalModelAvailable(false));
    getAiSettings()
      .then((settings) => setHasGeminiKey(Boolean(settings?.has_api_key)))
      .catch(() => setHasGeminiKey(false));
  }, [loadLibrary]);

  useEffect(() => {
    if (!isIndexing) {
      setActiveIndexDirectoryId(null);
    }
  }, [isIndexing]);

  const requireSavedKeyConsent = () => {
    if (!needsSavedGeminiKey || allowSavedKeyRead) {
      return true;
    }
    setSearchError(null);
    setSearchNotice("需要读取已保存的 Gemini API Key。请先勾选页面上的说明，确认后再继续。");
    return false;
  };

  const refreshIndexState = async () => {
    await loadLibrary();
    const stats = await getIndexStats();
    setIndexStats(stats);
  };

  const handleSearch = async () => {
    if (!query.trim()) return;
    if (indexedCount === 0) {
      setSearchError(null);
      setSearchNotice("请先为需要检索的文件夹生成本地索引，再开始自然语言检索。");
      return;
    }
    if (!requireSavedKeyConsent()) return;

    setIsSearching(true);
    setSearchError(null);
    setSearchNotice(null);
    try {
      const res = await naturalLanguageSearch(query.trim(), needsSavedGeminiKey);
      setResults(res.sort((a, b) => b.similarity - a.similarity));
      await refreshIndexState();
    } catch (e: any) {
      console.error(e);
      setSearchError(typeof e === "string" ? e : e?.message || "搜索失败，请检查 API Key 或网络连接");
    } finally {
      setIsSearching(false);
    }
  };

  const handleBuildFolderIndex = async (summary: FolderIndexSummary) => {
    if (summary.pendingPhotoIds.length === 0) return;
    if (!requireSavedKeyConsent()) return;

    setActiveIndexDirectoryId(summary.directory.id);
    cancelRequestedRef.current = false;
    setSearchError(null);
    setSearchNotice(null);
    try {
      const count = await buildSearchIndex(summary.pendingPhotoIds, needsSavedGeminiKey);
      await refreshIndexState();
      if (cancelRequestedRef.current) {
        setSearchNotice(`已中止“${directoryName(summary.directory.path)}”的索引任务，已保留完成的 ${count} 张本地索引。`);
      } else {
        setSearchNotice(`已为“${directoryName(summary.directory.path)}”生成 ${count} 张照片的本地索引。`);
      }
    } catch (e: any) {
      console.error(e);
      setSearchError(typeof e === "string" ? e : e?.message || "生成索引失败");
    } finally {
      cancelRequestedRef.current = false;
      setActiveIndexDirectoryId(null);
    }
  };

  const handleCancelIndexing = async () => {
    cancelRequestedRef.current = true;
    await cancelSearchIndexing();
    setSearchNotice("已请求中止索引任务，当前照片处理完成后会停止。");
  };

  const handleRebuildAll = async () => {
    if (!requireSavedKeyConsent()) return;
    if (!window.confirm("确定要重建全部文件夹索引吗？这会重新生成所有搜索向量，可能需要一些时间。")) {
      return;
    }

    setIsRebuilding(true);
    setActiveIndexDirectoryId("all");
    cancelRequestedRef.current = false;
    setSearchError(null);
    setSearchNotice(null);
    try {
      const count = await rebuildAllIndex(needsSavedGeminiKey);
      await refreshIndexState();
      if (cancelRequestedRef.current) {
        setSearchNotice("已中止全部索引重建，原有索引保持不变。");
      } else {
        setSearchNotice(`索引重建完成，成功索引 ${count} 张照片。`);
      }
    } catch (e: any) {
      console.error(e);
      setSearchError(typeof e === "string" ? e : e?.message || "重建索引失败");
    } finally {
      cancelRequestedRef.current = false;
      setIsRebuilding(false);
      setActiveIndexDirectoryId(null);
    }
  };

  const PhotoCard = ({
    result,
    size = "normal",
  }: {
    result: SearchResult;
    size?: "normal" | "large";
  }) => {
    const { photo, similarity } = result;
    const simPercent = Math.round(similarity * 100);
    const isSelected = selectedPhoto?.id === photo.id;

    return (
      <div
        key={photo.id}
        onClick={() => setSelectedPhoto(photo)}
        className={cn(
          "relative overflow-hidden rounded-lg border-2 transition-all group cursor-pointer",
          isSelected ? "border-blue-500 ring-2 ring-blue-100" : "border-transparent hover:border-gray-300"
        )}
      >
        <div className={cn("flex w-full items-center justify-center bg-gray-100", size === "large" ? "aspect-[4/3]" : "aspect-square")}>
          {photo.thumbnail_path ? (
            <img
              src={convertFileSrc(photo.thumbnail_path)}
              alt={photo.file_name}
              className="h-full w-full object-cover"
              loading="lazy"
              onError={(e) => {
                (e.target as HTMLImageElement).style.display = "none";
              }}
            />
          ) : (
            <div className="flex h-full w-full items-center justify-center bg-gray-100">
              <ImageIcon size={size === "large" ? 28 : 20} className="text-gray-300" />
            </div>
          )}
        </div>

        <div className={cn("absolute right-1.5 top-1.5 rounded px-1.5 py-0.5 text-[10px] font-medium backdrop-blur-sm", similarityBadgeClass(similarity))}>
          {simPercent}%
        </div>

        {photo.aesthetic_score !== null && (
          <div className="absolute left-1.5 top-1.5 rounded bg-black/60 px-1.5 py-0.5 text-[10px] text-white">
            <span className="text-amber-300">★</span> {Math.round(photo.aesthetic_score)}
          </div>
        )}

        <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 to-transparent px-2 py-2 opacity-0 transition-opacity group-hover:opacity-100">
          <p className="truncate text-[10px] text-white">{photo.file_name}</p>
        </div>
      </div>
    );
  };

  const noResultsReason = indexedCount === 0 ? "暂无已索引照片，无法搜索" : "未找到匹配结果，尝试换用其他关键词";

  return (
    <div className="flex h-full flex-col">
      <div className="px-6 pb-2 pt-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h2 className="mb-1 text-lg font-semibold text-gray-800">智能检索</h2>
            <p className="text-xs text-gray-400">
              用自然语言描述画面，快速找到相关照片
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2 rounded-full bg-gray-50 px-2.5 py-1 text-[11px] text-gray-500">
            <Database size={12} />
            <span>{indexedCount} / {totalPhotos} 已索引</span>
            {pendingIndexCount > 0 && <span className="text-amber-600">{pendingIndexCount} 待处理</span>}
          </div>
        </div>
      </div>

      <div className="px-6 pb-2">
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
              placeholder="例如：夕阳下的海边、猫在沙发上、红色跑车..."
              className="w-full rounded-md border border-gray-300 py-2.5 pl-9 pr-4 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <button
            onClick={handleSearch}
            disabled={isSearching || !query.trim() || indexedCount === 0}
            className="flex items-center gap-1.5 rounded-md bg-blue-600 px-5 py-2.5 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
          >
            {isSearching ? (
              <>
                <span className="h-3.5 w-3.5 rounded-full border-2 border-white/30 border-t-white animate-spin" />
                搜索中...
              </>
            ) : (
              <>
                <Search size={14} />
                搜索
              </>
            )}
          </button>
        </div>
      </div>

      <section className="mx-6 mb-2 overflow-hidden rounded-lg border border-gray-200 bg-white">
        <button
          type="button"
          onClick={() => setShowIndexPanel((open) => !open)}
          className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left hover:bg-gray-50"
        >
          <div className="flex min-w-0 items-center gap-2">
            <FolderOpen size={14} className="shrink-0 text-gray-500" />
            <span className="shrink-0 text-xs font-medium text-gray-700">本地索引</span>
            <div className="h-1.5 w-24 overflow-hidden rounded-full bg-gray-100">
              <div className="h-full rounded-full bg-blue-500" style={{ width: `${indexPercent}%` }} />
            </div>
            <span className="truncate text-[11px] text-gray-400">
              {indexedCount} / {totalPhotos} 张
              {pendingFolderCount > 0 ? `，${pendingFolderCount} 个文件夹待处理` : "，可检索"}
            </span>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {isIndexing && indexProgress && (
              <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[11px] font-medium text-emerald-700">
                {indexProgress.current} / {indexProgress.total}
              </span>
            )}
            {forceIndexPanelOpen && (
              <span className="rounded-full bg-amber-50 px-2 py-0.5 text-[11px] font-medium text-amber-700">需要索引</span>
            )}
            {indexPanelOpen ? <ChevronUp size={14} className="text-gray-400" /> : <ChevronDown size={14} className="text-gray-400" />}
          </div>
        </button>

        {isIndexing && indexProgress && (
          <div className="h-1 w-full bg-emerald-100">
            <div
              className="h-full bg-emerald-500 transition-all"
              style={{
                width: `${indexProgress.total > 0 ? (indexProgress.current / indexProgress.total) * 100 : 0}%`,
              }}
            />
          </div>
        )}

        {indexPanelOpen && (
          <div className="border-t border-gray-100 px-3 py-3">
            <div className="mb-3 flex items-start gap-2 rounded-md bg-blue-50 px-2.5 py-2 text-xs text-blue-700">
              <Database size={14} className="mt-0.5 shrink-0 text-blue-600" />
              <p className="leading-5">
                自然语言检索需要先生成本地搜索向量。索引保存在本机数据库中，你可以按文件夹决定是否处理。
              </p>
              {isIndexing && (
                <button
                  type="button"
                  onClick={handleCancelIndexing}
                  className="ml-auto flex shrink-0 items-center gap-1 rounded-md border border-blue-200 bg-white px-2 py-1 text-[11px] font-medium text-blue-700 hover:bg-blue-50"
                >
                  <Square size={11} />
                  中止
                </button>
              )}
            </div>

            {needsSavedGeminiKey && (
              <label className="mb-3 flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-2.5 py-2 text-xs text-amber-800">
                <input
                  type="checkbox"
                  checked={allowSavedKeyRead}
                  onChange={(e) => setAllowSavedKeyRead(e.target.checked)}
                  className="mt-0.5"
                />
                <span>
                  未加载本地 Chinese-CLIP 模型时，会从 macOS 钥匙串读取已保存的 Gemini API Key，只用于生成搜索向量。
                </span>
              </label>
            )}

            <div className="space-y-1.5">
              {folderSummaries.length === 0 ? (
                <div className="rounded-md bg-gray-50 px-2.5 py-2 text-xs text-gray-400">先在图库页面添加照片文件夹</div>
              ) : (
                folderSummaries.map((summary) => {
                  const isActive = activeIndexDirectoryId === summary.directory.id || activeIndexDirectoryId === "all";
                  const isComplete = summary.total > 0 && summary.pending === 0;
                  const percent = summary.total > 0 ? Math.round((summary.indexed / summary.total) * 100) : 0;

                  return (
                    <div key={summary.directory.id} className="flex items-center gap-3 rounded-md bg-gray-50 px-2.5 py-2">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <p className="truncate text-xs font-medium text-gray-800">{directoryName(summary.directory.path)}</p>
                          {isComplete && <CheckCircle2 size={13} className="shrink-0 text-emerald-500" />}
                          <span className="shrink-0 text-[11px] text-gray-400">{summary.indexed} / {summary.total}</span>
                        </div>
                        <div className="mt-1 flex items-center gap-2">
                          <div className="h-1.5 w-32 overflow-hidden rounded-full bg-white">
                            <div className="h-full rounded-full bg-blue-500" style={{ width: `${percent}%` }} />
                          </div>
                          <p className="truncate text-[11px] text-gray-400">{summary.directory.path}</p>
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => handleBuildFolderIndex(summary)}
                        disabled={isIndexing || summary.pending === 0}
                        className={cn(
                          "flex h-8 shrink-0 items-center gap-1.5 rounded-md px-2.5 text-xs font-medium",
                          summary.pending === 0
                            ? "bg-white text-gray-400"
                            : "bg-blue-600 text-white hover:bg-blue-700",
                          "disabled:opacity-60"
                        )}
                      >
                        {isActive ? (
                          <>
                            <span className="h-3 w-3 rounded-full border-2 border-white/40 border-t-white animate-spin" />
                            生成中
                          </>
                        ) : summary.pending === 0 ? (
                          "已完成"
                        ) : (
                          <>
                            <WandSparkles size={13} />
                            开始
                          </>
                        )}
                      </button>
                    </div>
                  );
                })
              )}
            </div>

            <div className="mt-3 flex items-center justify-between gap-3 border-t border-gray-100 pt-2">
              <button
                onClick={() => setShowDiagnostics(!showDiagnostics)}
                className="flex items-center gap-1 text-[11px] text-gray-400 transition-colors hover:text-gray-600"
              >
                <Wrench size={11} />
                诊断信息
                {showDiagnostics ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
              </button>
              {indexedCount > 0 && (
                <button
                  type="button"
                  onClick={handleRebuildAll}
                  disabled={isIndexing || isRebuilding}
                  className="rounded-md px-2 py-1 text-[11px] font-medium text-amber-600 hover:bg-amber-50 disabled:opacity-50"
                >
                  {isRebuilding ? "重建中..." : "重建全部"}
                </button>
              )}
            </div>

            {showDiagnostics && (
              <div className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 rounded-md border border-gray-200 bg-gray-50 p-2.5 font-mono text-[11px] text-gray-500">
                <span>数据库已索引:</span>
                <span>{indexedCount} / {totalPhotos}</span>
                <span>内存索引条目:</span>
                <span>{indexStats?.count ?? "--"}</span>
                <span>向量维度:</span>
                <span>{indexStats?.dimension ?? "--"}</span>
                <span>等待索引:</span>
                <span>{unindexedPhotos.length}</span>
                {indexStats && indexStats.count > 0 && indexStats.dimension && indexStats.count !== indexedCount && (
                  <div className="col-span-2 border-t border-gray-200 pt-1 text-amber-600">
                    内存索引与数据库不一致，建议重建索引
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </section>

      {searchError && (
        <NoticeBanner tone="red" title="搜索失败" message={searchError} onClose={() => setSearchError(null)} />
      )}

      {searchNotice && (
        <NoticeBanner tone="amber" title="提示" message={searchNotice} onClose={() => setSearchNotice(null)} />
      )}

      <div className="flex-1 overflow-auto px-6 pb-6 scrollbar-thin">
        {results.length === 0 && !isSearching && !searchError && query && (
          <div className="flex h-64 flex-col items-center justify-center text-gray-400">
            <Search size={40} strokeWidth={1.2} className="mb-3 text-gray-300" />
            <p className="text-sm text-gray-500">{noResultsReason}</p>
            {indexStats && indexStats.count > 0 && indexStats.dimension && (
              <p className="mt-2 text-xs text-gray-400">
                当前索引维度: {indexStats.dimension}，查询向量将与此匹配
              </p>
            )}
          </div>
        )}

        {results.length === 0 && !isSearching && !searchError && !query && (
          <div className="flex h-64 flex-col items-center justify-center text-gray-400">
            <Search size={40} strokeWidth={1.2} className="mb-3 text-gray-300" />
            <p className="text-sm text-gray-500">
              {indexedCount === 0 ? "选择文件夹并生成索引后即可开始检索" : "输入描述开始检索"}
            </p>
            {totalPhotos === 0 && (
              <p className="mt-2 text-xs text-gray-400">先在图库页面添加照片文件夹</p>
            )}
          </div>
        )}

        {results.length > 0 && (
          <div className="space-y-6">
            {highRelevance.length > 0 && (
              <section>
                <div className="mb-3 flex items-center gap-2">
                  <Sparkles size={14} className="text-emerald-500" />
                  <h3 className="text-sm font-semibold text-gray-800">最相关</h3>
                  <span className="text-[11px] text-gray-400">{highRelevance.length} 张</span>
                </div>
                <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4">
                  {highRelevance.map((r) => (
                    <PhotoCard key={r.photo.id} result={r} size="large" />
                  ))}
                </div>
              </section>
            )}

            {moreReference.length > 0 && (
              <section>
                <div className="mb-3 flex items-center gap-2">
                  <FolderOpen size={14} className="text-gray-400" />
                  <h3 className="text-sm font-medium text-gray-600">更多参考</h3>
                  <span className="text-[11px] text-gray-400">{moreReference.length} 张</span>
                </div>
                <div className="grid grid-cols-3 gap-2.5 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6">
                  {moreReference.map((r) => (
                    <PhotoCard key={r.photo.id} result={r} size="normal" />
                  ))}
                </div>
              </section>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function NoticeBanner({
  tone,
  title,
  message,
  onClose,
}: {
  tone: "red" | "amber";
  title: string;
  message: string;
  onClose: () => void;
}) {
  const toneClass =
    tone === "red"
      ? "border-red-200 bg-red-50 text-red-600"
      : "border-amber-200 bg-amber-50 text-amber-600";
  const titleClass = tone === "red" ? "text-red-700" : "text-amber-700";

  return (
    <div className={cn("mx-6 mb-3 flex items-start gap-2 rounded-lg border p-3", toneClass)}>
      <AlertTriangle size={14} className="mt-0.5 shrink-0" />
      <div className="flex-1">
        <p className={cn("text-xs font-medium", titleClass)}>{title}</p>
        <p className="mt-0.5 text-xs">{message}</p>
      </div>
      <button type="button" onClick={onClose} className="text-xs hover:opacity-80">
        关闭
      </button>
    </div>
  );
}
