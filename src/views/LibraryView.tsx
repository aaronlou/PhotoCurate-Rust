import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import { pickDirectory } from "@/hooks/useInvoke";
import {
  AlertCircle,
  FolderPlus,
  Folder,
  FolderOpen,
  Folders,
  LayoutGrid,
  List,
  Image as ImageIcon,
  ArrowDownAZ,
  ArrowUpAZ,
  CalendarDays,
  ChevronDown,
  Sparkles,
  Trash2,
  WandSparkles,
} from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Directory, Photo } from "@/types";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

function directoryName(path: string) {
  const normalized = path.replace(/\/+$/, "");
  return normalized.split("/").pop() || normalized || path;
}

export default function LibraryView() {
  const photos = useAppStore((s) => s.photos);
  const directories = useAppStore((s) => s.directories);
  const selectedPhoto = useAppStore((s) => s.selectedPhoto);
  const setSelectedPhoto = useAppStore((s) => s.setSelectedPhoto);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const viewMode = useAppStore((s) => s.viewMode);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const photoSortOrder = useAppStore((s) => s.photoSortOrder);
  const addDirectoryAndRefresh = useAppStore((s) => s.addDirectoryAndRefresh);
  const removeDirectoryAndRefresh = useAppStore((s) => s.removeDirectoryAndRefresh);
  const changePhotoSortOrder = useAppStore((s) => s.changePhotoSortOrder);
  const loadLibrary = useAppStore((s) => s.loadLibrary);
  const [isAdding, setIsAdding] = useState(false);
  const [removingDirectoryId, setRemovingDirectoryId] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [showSortMenu, setShowSortMenu] = useState(false);
  const [selectedDirectoryId, setSelectedDirectoryId] = useState<string>("all");

  useEffect(() => {
    loadLibrary().catch((error) => {
      console.error("Load library failed:", error);
      setErrorMsg(
        typeof error === "string"
          ? error
          : error?.message || "加载图库失败，请稍后重试"
      );
    });
  }, [loadLibrary]);

  const directoryPhotoCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const photo of photos) {
      if (photo.directory_id) {
        counts.set(photo.directory_id, (counts.get(photo.directory_id) ?? 0) + 1);
      }
    }
    return counts;
  }, [photos]);

  const selectedDirectory =
    selectedDirectoryId === "all"
      ? null
      : directories.find((directory) => directory.id === selectedDirectoryId) ?? null;

  const visiblePhotos = useMemo(() => {
    if (selectedDirectoryId === "all") {
      return photos;
    }
    return photos.filter((photo) => photo.directory_id === selectedDirectoryId);
  }, [photos, selectedDirectoryId]);

  useEffect(() => {
    if (selectedDirectoryId !== "all" && !directories.some((directory) => directory.id === selectedDirectoryId)) {
      setSelectedDirectoryId("all");
    }
  }, [directories, selectedDirectoryId]);

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

  const handleSelectDirectory = (directoryId: string) => {
    setSelectedDirectoryId(directoryId);
    if (directoryId !== "all" && selectedPhoto?.directory_id !== directoryId) {
      setSelectedPhoto(null);
    }
  };

  const handleRemoveDirectory = async (event: React.MouseEvent, directory: Directory) => {
    event.stopPropagation();
    const confirmed = window.confirm(
      `从图库移出“${directoryName(directory.path)}”？\n不会删除磁盘上的照片。`
    );
    if (!confirmed) {
      return;
    }

    setRemovingDirectoryId(directory.id);
    setErrorMsg(null);
    try {
      await removeDirectoryAndRefresh(directory.id);
      if (selectedDirectoryId === directory.id) {
        setSelectedDirectoryId("all");
      }
      if (selectedPhoto?.directory_id === directory.id) {
        setSelectedPhoto(null);
      }
    } catch (e: any) {
      console.error("Remove directory failed:", e);
      setErrorMsg(typeof e === "string" ? e : e?.message || "移出文件夹失败，请检查控制台日志");
    } finally {
      setRemovingDirectoryId(null);
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
  const missingEvaluationCount = visiblePhotos.filter((p) => !p.latest_evaluation).length;
  const evaluatedCount = visiblePhotos.filter((p) => p.latest_evaluation).length;
  const selectedScopeLabel = selectedDirectory ? directoryName(selectedDirectory.path) : "全部照片";

  if (directories.length === 0 && photos.length === 0) {
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
          <span className="text-[11px] font-medium text-gray-600">{selectedScopeLabel}</span>
          <span className="text-[11px] text-gray-400">
            {visiblePhotos.length} 张照片 / {directories.length} 个文件夹
          </span>
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
        <aside className="w-[280px] shrink-0 overflow-auto border-r border-gray-100 bg-gray-50/70 px-3 py-3">
          <div className="mb-3 flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 text-sm font-medium text-gray-700">
              <Folders size={16} />
              源文件夹
            </div>
            <button
              type="button"
              onClick={handleAddDirectory}
              disabled={isAdding}
              title="添加文件夹"
              className="rounded-md p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-800 disabled:opacity-50"
            >
              <FolderPlus size={15} />
            </button>
          </div>

          <div className="space-y-1">
            <button
              type="button"
              onClick={() => handleSelectDirectory("all")}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
                selectedDirectoryId === "all"
                  ? "bg-white text-blue-700 shadow-sm ring-1 ring-blue-100"
                  : "text-gray-600 hover:bg-white"
              )}
            >
              <Folders size={15} />
              <span className="min-w-0 flex-1 truncate">全部照片</span>
              <span className="shrink-0 text-[11px] text-gray-400">{photos.length}</span>
            </button>

            {directories.map((directory) => {
              const isActive = selectedDirectoryId === directory.id;
              const count = directoryPhotoCounts.get(directory.id) ?? 0;

              return (
                <div
                  key={directory.id}
                  className={cn(
                    "group flex items-center gap-1 rounded-md transition-colors",
                    isActive
                      ? "bg-white text-blue-700 shadow-sm ring-1 ring-blue-100"
                      : "text-gray-600 hover:bg-white"
                  )}
                >
                  <button
                    type="button"
                    onClick={() => handleSelectDirectory(directory.id)}
                    className="flex min-w-0 flex-1 items-center gap-2 px-2.5 py-2 text-left"
                  >
                    {isActive ? <FolderOpen size={15} /> : <Folder size={15} />}
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium">{directoryName(directory.path)}</span>
                      <span className="block truncate text-[11px] text-gray-400">{directory.path}</span>
                    </span>
                    <span className="shrink-0 text-[11px] text-gray-400">{count}</span>
                  </button>
                  <button
                    type="button"
                    title="移出文件夹"
                    onClick={(event) => handleRemoveDirectory(event, directory)}
                    disabled={removingDirectoryId === directory.id}
                    className={cn(
                      "mr-1 shrink-0 rounded p-1 text-gray-300 opacity-0 hover:bg-red-50 hover:text-red-600 group-hover:opacity-100 disabled:pointer-events-none disabled:opacity-50",
                      isActive && "opacity-100",
                    )}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              );
            })}
          </div>

          {errorMsg && (
            <div className="mt-3 flex items-start gap-1.5 rounded-md bg-red-50 px-2.5 py-2 text-xs leading-5 text-red-600">
              <AlertCircle size={14} className="mt-0.5 shrink-0" />
              <span>{errorMsg}</span>
            </div>
          )}
        </aside>

        <div className="flex-1 overflow-auto p-4 scrollbar-thin">
          {visiblePhotos.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center text-gray-500">
              <ImageIcon size={40} strokeWidth={1.2} className="mb-3 text-gray-300" />
              <h2 className="mb-1 text-sm font-medium text-gray-700">暂无照片</h2>
              <p className="text-xs text-gray-400">
                {selectedDirectory ? "这个文件夹里还没有可识别的照片" : "已添加文件夹，但还没有可识别的照片"}
              </p>
            </div>
          ) : viewMode === "grid" ? (
            <div className="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 gap-3">
              {visiblePhotos.map((photo) => (
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
              {visiblePhotos.map((photo) => (
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
