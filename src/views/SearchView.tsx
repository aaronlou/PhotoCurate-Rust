import { useState, useEffect } from "react";
import { useAppStore } from "@/stores/useAppStore";
import {
  naturalLanguageSearch,
  getIndexStats,
  buildSearchIndex,
  rebuildAllIndex,
} from "@/hooks/useInvoke";
import type { SearchResult } from "@/types";
import {
  Search,
  Image as ImageIcon,
  Database,
  AlertTriangle,
  Sparkles,
  FolderOpen,
  Wrench,
  ChevronDown,
  ChevronUp,
} from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
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

export default function SearchView() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [indexStats, setIndexStats] = useState<{
    count: number;
    dimension: number | null;
  } | null>(null);
  const [showDiagnostics, setShowDiagnostics] = useState(false);
  const [isBuildingIndex, setIsBuildingIndex] = useState(false);
  const [isRebuilding, setIsRebuilding] = useState(false);

  const selectedPhoto = useAppStore((s) => s.selectedPhoto);
  const setSelectedPhoto = useAppStore((s) => s.setSelectedPhoto);
  const photos = useAppStore((s) => s.photos);
  const isIndexing = useAppStore((s) => s.isIndexing);
  const indexProgress = useAppStore((s) => s.indexProgress);

  const indexedCount = photos.filter((p) => p.has_embedding).length;
  const totalPhotos = photos.length;
  const unindexedPhotos = photos.filter((p) => !p.has_embedding);

  const highRelevance = results.filter((r) => r.similarity >= 0.5);
  const moreReference = results.filter((r) => r.similarity < 0.5);

  useEffect(() => {
    getIndexStats()
      .then(setIndexStats)
      .catch(() => setIndexStats(null));
  }, [indexedCount, isIndexing]);

  const handleSearch = async () => {
    if (!query.trim()) return;
    setIsSearching(true);
    setSearchError(null);
    try {
      const res = await naturalLanguageSearch(query.trim());
      const sorted = res.sort((a, b) => b.similarity - a.similarity);
      setResults(sorted);

      // Refresh index stats after search
      const stats = await getIndexStats();
      setIndexStats(stats);
    } catch (e: any) {
      console.error(e);
      setSearchError(
        typeof e === "string"
          ? e
          : e?.message || "搜索失败，请检查 API Key 或网络连接"
      );
    } finally {
      setIsSearching(false);
    }
  };

  const handleBuildIndex = async () => {
    if (unindexedPhotos.length === 0) return;
    setIsBuildingIndex(true);
    try {
      await buildSearchIndex(unindexedPhotos.map((p) => p.id));
    } catch (e) {
      console.error(e);
    } finally {
      setIsBuildingIndex(false);
    }
  };

  const handleRebuildAll = async () => {
    if (!window.confirm("确定要重建所有索引吗？这会清空现有向量并重新生成，可能需要一些时间。")) {
      return;
    }
    setIsRebuilding(true);
    setSearchError(null);
    try {
      const count = await rebuildAllIndex();
      setSearchError(`索引重建完成，成功索引 ${count} 张照片`);
      const stats = await getIndexStats();
      setIndexStats(stats);
    } catch (e: any) {
      console.error(e);
      setSearchError(
        typeof e === "string"
          ? e
          : e?.message || "重建索引失败"
      );
    } finally {
      setIsRebuilding(false);
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
          "relative rounded-lg overflow-hidden cursor-pointer border-2 transition-all group",
          isSelected
            ? "border-blue-500 ring-2 ring-blue-100"
            : "border-transparent hover:border-gray-300"
        )}
      >
        <div
          className={cn(
            "w-full bg-gray-100 flex items-center justify-center",
            size === "large" ? "aspect-[4/3]" : "aspect-square"
          )}
        >
          {photo.thumbnail_path ? (
            <img
              src={convertFileSrc(photo.thumbnail_path)}
              alt={photo.file_name}
              className="w-full h-full object-cover"
              loading="lazy"
              onError={(e) => {
                (e.target as HTMLImageElement).style.display = "none";
              }}
            />
          ) : (
            <div className="w-full h-full bg-gray-100 flex items-center justify-center">
              <ImageIcon
                size={size === "large" ? 28 : 20}
                className="text-gray-300"
              />
            </div>
          )}
        </div>

        <div
          className={cn(
            "absolute top-1.5 right-1.5 text-[10px] px-1.5 py-0.5 rounded font-medium backdrop-blur-sm",
            similarityBadgeClass(similarity)
          )}
        >
          {simPercent}%
        </div>

        {photo.aesthetic_score !== null && (
          <div className="absolute top-1.5 left-1.5 bg-black/60 text-white text-[10px] px-1.5 py-0.5 rounded">
            <span className="text-amber-300">★</span>{" "}
            {Math.round(photo.aesthetic_score)}
          </div>
        )}

        <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 to-transparent px-2 py-2 opacity-0 group-hover:opacity-100 transition-opacity">
          <p className="text-[10px] text-white truncate">{photo.file_name}</p>
        </div>
      </div>
    );
  };

  const noResultsReason =
    indexedCount === 0
      ? "暂无已索引照片，无法搜索"
      : "未找到匹配结果，尝试换用其他关键词";

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="px-6 pt-6 pb-3">
        <h2 className="text-lg font-semibold text-gray-800 mb-1">智能检索</h2>
        <p className="text-xs text-gray-400">
          输入自然语言描述，通过 AI 语义匹配找到相关照片
        </p>
      </div>

      {/* Indexing status */}
      {isIndexing && indexProgress && (
        <div className="mx-6 mb-3 bg-emerald-50 border border-emerald-200 rounded-lg p-3">
          <div className="flex justify-between text-xs mb-1">
            <span className="text-emerald-700 font-medium flex items-center gap-1">
              <Sparkles size={12} />
              正在生成搜索索引...
            </span>
            <span className="text-emerald-600">
              {indexProgress.current} / {indexProgress.total}
            </span>
          </div>
          <div className="w-full bg-emerald-200 rounded-full h-1.5">
            <div
              className="bg-emerald-600 h-1.5 rounded-full transition-all"
              style={{
                width: `${(indexProgress.current / indexProgress.total) * 100}%`,
              }}
            />
          </div>
        </div>
      )}

      {/* No index warning */}
      {!isIndexing && indexedCount === 0 && totalPhotos > 0 && (
        <div className="mx-6 mb-3 bg-amber-50 border border-amber-200 rounded-lg p-3 flex items-start gap-2">
          <AlertTriangle
            size={14}
            className="text-amber-600 mt-0.5 flex-shrink-0"
          />
          <div className="flex-1">
            <p className="text-xs text-amber-700 font-medium">
              暂无已索引照片
            </p>
            <p className="text-xs text-amber-600 mt-0.5">
              请先配置 Gemini API Key 或加载本地 Chinese-CLIP 模型，然后点击"建立索引"按钮。
            </p>
          </div>
        </div>
      )}

      {/* Diagnostics toggle */}
      <div className="mx-6 mb-2">
        <button
          onClick={() => setShowDiagnostics(!showDiagnostics)}
          className="flex items-center gap-1 text-[11px] text-gray-400 hover:text-gray-600 transition-colors"
        >
          <Wrench size={11} />
          诊断信息
          {showDiagnostics ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
        </button>
        {showDiagnostics && (
          <div className="mt-1.5 bg-gray-50 border border-gray-200 rounded-md p-2.5 space-y-1 text-[11px] text-gray-500 font-mono">
            <div className="flex justify-between">
              <span>数据库已索引:</span>
              <span>
                {indexedCount} / {totalPhotos}
              </span>
            </div>
            <div className="flex justify-between">
              <span>内存索引条目:</span>
              <span>{indexStats?.count ?? "--"}</span>
            </div>
            <div className="flex justify-between">
              <span>向量维度:</span>
              <span>{indexStats?.dimension ?? "--"}</span>
            </div>
            <div className="flex justify-between">
              <span>等待索引:</span>
              <span>{unindexedPhotos.length}</span>
            </div>
            {indexStats &&
              indexStats.count > 0 &&
              indexStats.dimension &&
              indexStats.count !== indexedCount && (
                <div className="text-amber-600 pt-1 border-t border-gray-200 mt-1">
                  ⚠️ 内存索引与数据库不一致，建议重建索引
                </div>
              )}
          </div>
        )}
      </div>

      {/* Search bar */}
      <div className="px-6 pb-3">
        <div className="flex gap-2">
          <div className="flex-1 relative">
            <Search
              size={16}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400"
            />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
              placeholder="例如：夕阳下的海边、猫在沙发上、红色跑车..."
              className="w-full pl-9 pr-4 py-2.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <button
            onClick={handleSearch}
            disabled={isSearching || !query.trim()}
            className="px-5 py-2.5 bg-blue-600 text-white text-sm font-medium rounded-md hover:bg-blue-700 disabled:opacity-50 flex items-center gap-1.5"
          >
            {isSearching ? (
              <>
                <span className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
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

      {/* Stats bar */}
      <div className="px-6 pb-2 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 text-xs text-gray-400">
            <Database size={12} />
            <span>
              {indexedCount} / {totalPhotos} 张照片已索引
            </span>
          </div>
          {unindexedPhotos.length > 0 && !isIndexing && (
            <button
              onClick={handleBuildIndex}
              disabled={isBuildingIndex}
              className="text-[11px] text-blue-600 hover:text-blue-700 font-medium disabled:opacity-50"
            >
              {isBuildingIndex ? "建立中..." : "建立索引"}
            </button>
          )}
          {indexedCount > 0 && !isIndexing && (
            <button
              onClick={handleRebuildAll}
              disabled={isRebuilding}
              className="text-[11px] text-amber-600 hover:text-amber-700 font-medium disabled:opacity-50"
            >
              {isRebuilding ? "重建中..." : "重建索引"}
            </button>
          )}
        </div>
        {results.length > 0 && (
          <span className="text-xs text-gray-500">
            找到 {results.length} 张相关照片
          </span>
        )}
      </div>

      {/* Error banner */}
      {searchError && (
        <div className="mx-6 mb-3 bg-red-50 border border-red-200 rounded-lg p-3 flex items-start gap-2">
          <AlertTriangle
            size={14}
            className="text-red-600 mt-0.5 flex-shrink-0"
          />
          <div className="flex-1">
            <p className="text-xs text-red-700 font-medium">搜索失败</p>
            <p className="text-xs text-red-600 mt-0.5">{searchError}</p>
          </div>
          <button
            onClick={() => setSearchError(null)}
            className="text-xs text-red-500 hover:text-red-700"
          >
            关闭
          </button>
        </div>
      )}

      {/* Results */}
      <div className="flex-1 overflow-auto px-6 pb-6 scrollbar-thin">
        {results.length === 0 && !isSearching && !searchError && query && (
          <div className="flex flex-col items-center justify-center h-64 text-gray-400">
            <Search size={40} strokeWidth={1.2} className="mb-3 text-gray-300" />
            <p className="text-sm text-gray-500">{noResultsReason}</p>
            {indexStats && indexStats.count > 0 && indexStats.dimension && (
              <p className="text-xs text-gray-400 mt-2">
                当前索引维度: {indexStats.dimension}，查询向量将与此匹配
              </p>
            )}
          </div>
        )}

        {results.length === 0 && !isSearching && !searchError && !query && (
          <div className="flex flex-col items-center justify-center h-64 text-gray-400">
            <Search size={40} strokeWidth={1.2} className="mb-3 text-gray-300" />
            <p className="text-sm text-gray-500">
              {indexedCount === 0
                ? "等待索引完成后即可开始检索"
                : "输入描述开始检索"}
            </p>
            {totalPhotos === 0 && (
              <p className="text-xs text-gray-400 mt-2">
                先在图库页面添加照片文件夹
              </p>
            )}
          </div>
        )}

        {results.length > 0 && (
          <div className="space-y-6">
            {highRelevance.length > 0 && (
              <section>
                <div className="flex items-center gap-2 mb-3">
                  <Sparkles size={14} className="text-emerald-500" />
                  <h3 className="text-sm font-semibold text-gray-800">
                    最相关
                  </h3>
                  <span className="text-[11px] text-gray-400">
                    {highRelevance.length} 张
                  </span>
                </div>
                <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3">
                  {highRelevance.map((r) => (
                    <PhotoCard key={r.photo.id} result={r} size="large" />
                  ))}
                </div>
              </section>
            )}

            {moreReference.length > 0 && (
              <section>
                <div className="flex items-center gap-2 mb-3">
                  <FolderOpen size={14} className="text-gray-400" />
                  <h3 className="text-sm font-medium text-gray-600">
                    更多参考
                  </h3>
                  <span className="text-[11px] text-gray-400">
                    {moreReference.length} 张
                  </span>
                </div>
                <div className="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 gap-2.5">
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
