import { useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import { pickDirectory, addDirectory, getPhotos, getDirectories } from "@/hooks/useInvoke";
import { FolderPlus, LayoutGrid, List, Image as ImageIcon } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { convertFileSrc } from "@tauri-apps/api/core";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export default function LibraryView() {
  const photos = useAppStore((s) => s.photos);
  const setPhotos = useAppStore((s) => s.setPhotos);
  const setDirectories = useAppStore((s) => s.setDirectories);
  const selectedPhoto = useAppStore((s) => s.selectedPhoto);
  const setSelectedPhoto = useAppStore((s) => s.setSelectedPhoto);
  const viewMode = useAppStore((s) => s.viewMode);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const [isAdding, setIsAdding] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const handleAddDirectory = async () => {
    setIsAdding(true);
    setErrorMsg(null);
    try {
      const path = await pickDirectory();
      console.log("[DEBUG] Picked path:", path);
      if (path) {
        const dir = await addDirectory(path);
        console.log("[DEBUG] Added directory:", dir);
        const dirs = await getDirectories();
        setDirectories(dirs);
        const ps = await getPhotos();
        console.log("[DEBUG] Photos loaded:", ps.length);
        setPhotos(ps);
      }
    } catch (e: any) {
      console.error("[DEBUG] Add directory failed:", e);
      setErrorMsg(typeof e === "string" ? e : e?.message || "添加文件夹失败，请检查控制台日志");
    } finally {
      setIsAdding(false);
    }
  };

  if (photos.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-gray-500">
        <ImageIcon size={48} strokeWidth={1.2} className="mb-4 text-gray-300" />
        <h2 className="text-base font-medium text-gray-700 mb-1">暂无照片</h2>
        <p className="text-sm text-gray-400 mb-5">添加包含照片的文件夹，开始自动扫描和 AI 评分</p>
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
        <span className="text-[11px] text-gray-400">{photos.length} 张照片</span>
        <div className="flex items-center gap-2">
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
    </div>
  );
}
