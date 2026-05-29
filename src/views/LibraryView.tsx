import { useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import { pickDirectory } from "@/hooks/useInvoke";
import {
  FolderPlus,
  LayoutGrid,
  List,
  Image as ImageIcon,
  ArrowDownAZ,
  ArrowUpAZ,
  CalendarDays,
  ChevronDown,
  Sparkles,
  WandSparkles,
} from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Photo } from "@/types";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export default function LibraryView() {
  const photos = useAppStore((s) => s.photos);
  const selectedPhoto = useAppStore((s) => s.selectedPhoto);
  const setSelectedPhoto = useAppStore((s) => s.setSelectedPhoto);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const viewMode = useAppStore((s) => s.viewMode);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const photoSortOrder = useAppStore((s) => s.photoSortOrder);
  const addDirectoryAndRefresh = useAppStore((s) => s.addDirectoryAndRefresh);
  const changePhotoSortOrder = useAppStore((s) => s.changePhotoSortOrder);
  const [isAdding, setIsAdding] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [showSortMenu, setShowSortMenu] = useState(false);

  const handleAddDirectory = async () => {
    setIsAdding(true);
    setErrorMsg(null);
    try {
      const path = await pickDirectory();
      if (path) {
        await addDirectoryAndRefresh(path);
      }
    } catch (e: any) {
      console.error("Add directory failed:", e);
      setErrorMsg(typeof e === "string" ? e : e?.message || "添加文件夹失败，请检查控制台日志");
    } finally {
      setIsAdding(false);
    }
  };

  const handleSortChange = async (order: "date_desc" | "score_desc" | "score_asc") => {
    setShowSortMenu(false);
    try {
      await changePhotoSortOrder(order);
    } catch (e: any) {
      console.error("Sort photos failed:", e);
    }
  };

  const sortLabel =
    photoSortOrder === "score_desc"
      ? "评分从高到低"
      : photoSortOrder === "score_asc"
      ? "评分从低到高"
      : "按时间排序";
  const missingEvaluationCount = photos.filter((p) => !p.latest_evaluation).length;
  const evaluatedCount = photos.filter((p) => p.latest_evaluation).length;

  if (photos.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-gray-500">
        <ImageIcon size={48} strokeWidth={1.2} className="mb-4 text-gray-300" />
        <h2 className="text-base font-medium text-gray-700 mb-1">暂无照片</h2>
        <p className="text-sm text-gray-400 mb-5">添加包含照片的文件夹，开始整理、分析和筛选作品</p>
        <button
          onClick={handleAddDirectory}
          disabled={isAdding}
          className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-md hover:bg-blue-700 disabled:opacity-50"
        >
          <FolderPlus size={16} />
          {isAdding ? "添加中..." : "添加文件夹"}
        </button>
        {errorMsg && (
          <p className="mt-3 text-xs text-red-500 max-w-xs text-center">{errorMsg}</p>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-gray-100">
        <div className="flex items-center gap-3">
          <span className="text-[11px] text-gray-400">{photos.length} 张照片</span>
          {missingEvaluationCount > 0 && (
            <button
              type="button"
              onClick={() => setCurrentView("scoring")}
              className="flex items-center gap-1.5 rounded-full bg-amber-50 px-2.5 py-1 text-[11px] font-medium text-amber-700 hover:bg-amber-100"
            >
              <WandSparkles size={12} />
              {missingEvaluationCount} 张待分析
            </button>
          )}
          {missingEvaluationCount === 0 && evaluatedCount > 0 && (
            <button
              type="button"
              onClick={() => setCurrentView("insights")}
              className="flex items-center gap-1.5 rounded-full bg-green-50 px-2.5 py-1 text-[11px] font-medium text-green-700 hover:bg-green-100"
            >
              <Sparkles size={12} />
              查看洞察
            </button>
          )}
        </div>
        <div className="flex items-center gap-2">
          {/* Sort dropdown */}
          <div className="relative">
            <button
              onClick={() => setShowSortMenu(!showSortMenu)}
              className="flex items-center gap-1 px-2 py-1 text-xs text-gray-600 bg-gray-100 hover:bg-gray-200 rounded-md"
            >
              {photoSortOrder === "score_desc" && <ArrowDownAZ size={13} />}
              {photoSortOrder === "score_asc" && <ArrowUpAZ size={13} />}
              {photoSortOrder === "date_desc" && <CalendarDays size={13} />}
              <span>{sortLabel}</span>
              <ChevronDown size={12} />
            </button>
            {showSortMenu && (
              <>
                <div
                  className="fixed inset-0 z-10"
                  onClick={() => setShowSortMenu(false)}
                />
                <div className="absolute right-0 top-full mt-1 w-40 bg-white border border-gray-200 rounded-md shadow-lg z-20 py-1">
                  <button
                    onClick={() => handleSortChange("date_desc")}
                    className={cn(
                      "flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left hover:bg-gray-50",
                      photoSortOrder === "date_desc" && "text-blue-600 bg-blue-50"
                    )}
                  >
                    <CalendarDays size={13} />
                    按时间排序
                  </button>
                  <button
                    onClick={() => handleSortChange("score_desc")}
                    className={cn(
                      "flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left hover:bg-gray-50",
                      photoSortOrder === "score_desc" && "text-blue-600 bg-blue-50"
                    )}
                  >
                    <ArrowDownAZ size={13} />
                    评分从高到低
                  </button>
                  <button
                    onClick={() => handleSortChange("score_asc")}
                    className={cn(
                      "flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left hover:bg-gray-50",
                      photoSortOrder === "score_asc" && "text-blue-600 bg-blue-50"
                    )}
                  >
                    <ArrowUpAZ size={13} />
                    评分从低到高
                  </button>
                </div>
              </>
            )}
          </div>
          <div className="flex bg-gray-100 rounded-md p-0.5">
            <button
              onClick={() => setViewMode("grid")}
              className={cn(
                "p-1 rounded",
                viewMode === "grid" ? "bg-white shadow-sm text-gray-800" : "text-gray-400 hover:text-gray-600"
              )}
            >
              <LayoutGrid size={14} />
            </button>
            <button
              onClick={() => setViewMode("list")}
              className={cn(
                "p-1 rounded",
                viewMode === "list" ? "bg-white shadow-sm text-gray-800" : "text-gray-400 hover:text-gray-600"
              )}
            >
              <List size={14} />
            </button>
          </div>
          <button
            onClick={handleAddDirectory}
            disabled={isAdding}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-md disabled:opacity-50"
          >
            <FolderPlus size={14} />
            添加文件夹
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 min-h-0 flex">
        <div className="flex-1 overflow-auto p-4 scrollbar-thin">
          {viewMode === "grid" ? (
            <div className="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 gap-3">
              {photos.map((photo) => (
                <div
                  key={photo.id}
                  onClick={() => setSelectedPhoto(photo)}
                  className={cn(
                    "relative aspect-square rounded-lg overflow-hidden cursor-pointer border-2 transition-all",
                    selectedPhoto?.id === photo.id
                      ? "border-blue-500 ring-2 ring-blue-100"
                      : "border-transparent hover:border-gray-300"
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
                      <ImageIcon size={20} className="text-gray-300" />
                    </div>
                  )}
                  {photo.aesthetic_score !== null && (
                    <div className="absolute top-1.5 right-1.5 bg-black/60 text-white text-[10px] px-1.5 py-0.5 rounded">
                      {Math.round(photo.aesthetic_score)}
                    </div>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <div className="space-y-1">
              {photos.map((photo) => (
                <div
                  key={photo.id}
                  onClick={() => setSelectedPhoto(photo)}
                  className={cn(
                    "flex items-center gap-3 px-3 py-2 rounded-md cursor-pointer transition-colors",
                    selectedPhoto?.id === photo.id
                      ? "bg-blue-50"
                      : "hover:bg-gray-50"
                  )}
                >
                  <div className="w-10 h-10 bg-gray-100 rounded-md flex items-center justify-center flex-shrink-0 overflow-hidden">
                    {photo.thumbnail_path ? (
                      <img
                        src={convertFileSrc(photo.thumbnail_path)}
                        alt=""
                        className="w-full h-full object-cover"
                        loading="lazy"
                        onError={(e) => {
                          (e.target as HTMLImageElement).style.display = "none";
                        }}
                      />
                    ) : (
                      <ImageIcon size={16} className="text-gray-300" />
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-gray-800 truncate">{photo.file_name}</p>
                    <p className="text-[11px] text-gray-400 truncate">{photo.file_path}</p>
                  </div>
                  {photo.aesthetic_score !== null && (
                    <span className="text-xs font-medium text-amber-600">
                      {Math.round(photo.aesthetic_score)}
                    </span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
        {selectedPhoto && (
          <EvaluationPanel photo={selectedPhoto} />
        )}
      </div>
    </div>
  );
}

function EvaluationPanel({ photo }: { photo: Photo }) {
  const evaluation = photo.latest_evaluation;
  const setCurrentView = useAppStore((s) => s.setCurrentView);

  return (
    <aside className="w-[340px] border-l border-gray-100 bg-gray-50/60 overflow-auto p-4">
      <div className="flex items-start justify-between gap-3 mb-4">
        <div className="min-w-0">
          <p className="text-sm font-medium text-gray-800 truncate">{photo.file_name}</p>
          <p className="text-[11px] text-gray-400 truncate">{photo.file_path}</p>
        </div>
        {photo.aesthetic_score !== null && (
          <div className="shrink-0 rounded-md bg-amber-100 px-2 py-1 text-sm font-semibold text-amber-700">
            {Math.round(photo.aesthetic_score)}
          </div>
        )}
      </div>

      {!evaluation ? (
        <div className="rounded-lg border border-dashed border-gray-200 bg-white p-4">
          <p className="text-sm text-gray-500">这张照片还没有 AI 评价。</p>
          <button
            type="button"
            onClick={() => setCurrentView("scoring")}
            className="mt-3 flex items-center gap-1.5 rounded-md bg-blue-600 px-3 py-2 text-xs font-medium text-white hover:bg-blue-700"
          >
            <WandSparkles size={14} />
            生成 AI 分析
          </button>
        </div>
      ) : (
        <div className="space-y-4">
          <section className="rounded-lg border border-gray-200 bg-white p-4">
            <div className="mb-2 flex items-center gap-2 text-sm font-medium text-gray-800">
              <Sparkles size={15} className="text-blue-500" />
              AI 评价
            </div>
            <p className="text-sm leading-6 text-gray-600">{evaluation.summary}</p>
            <p className="mt-3 text-[11px] text-gray-400">
              {evaluation.model_provider} / {evaluation.model_name}
            </p>
          </section>

          <EvaluationList title="优点" items={evaluation.strengths} tone="green" />
          <EvaluationList title="不足" items={evaluation.weaknesses} tone="amber" />
          <EvaluationList title="建议" items={evaluation.suggestions} tone="blue" />

          {evaluation.dimension_scores.length > 0 && (
            <section className="rounded-lg border border-gray-200 bg-white p-4">
              <h3 className="mb-3 text-sm font-medium text-gray-800">维度评分</h3>
              <div className="space-y-3">
                {evaluation.dimension_scores.map((dimension) => (
                  <div key={dimension.name}>
                    <div className="mb-1 flex items-center justify-between text-xs">
                      <span className="text-gray-600">{dimension.name}</span>
                      <span className="font-medium text-gray-800">{Math.round(dimension.score)}</span>
                    </div>
                    <div className="h-1.5 overflow-hidden rounded-full bg-gray-100">
                      <div
                        className="h-full rounded-full bg-blue-500"
                        style={{ width: `${Math.max(0, Math.min(100, dimension.score))}%` }}
                      />
                    </div>
                    {dimension.note && (
                      <p className="mt-1 text-[11px] leading-4 text-gray-400">{dimension.note}</p>
                    )}
                  </div>
                ))}
              </div>
            </section>
          )}

          {evaluation.tags.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {evaluation.tags.map((tag) => (
                <span key={tag} className="rounded bg-gray-100 px-2 py-1 text-[11px] text-gray-500">
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </aside>
  );
}

function EvaluationList({
  title,
  items,
  tone,
}: {
  title: string;
  items: string[];
  tone: "green" | "amber" | "blue";
}) {
  if (items.length === 0) {
    return null;
  }

  const toneClass =
    tone === "green"
      ? "border-green-200 bg-green-50 text-green-800"
      : tone === "amber"
      ? "border-amber-200 bg-amber-50 text-amber-800"
      : "border-blue-200 bg-blue-50 text-blue-800";

  return (
    <section className={cn("rounded-lg border p-4", toneClass)}>
      <h3 className="mb-2 text-sm font-medium">{title}</h3>
      <ul className="space-y-1.5">
        {items.map((item) => (
          <li key={item} className="text-xs leading-5">
            {item}
          </li>
        ))}
      </ul>
    </section>
  );
}
