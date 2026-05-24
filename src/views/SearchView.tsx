import { useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import { naturalLanguageSearch } from "@/hooks/useInvoke";
import { Search, Image as ImageIcon } from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";

export default function SearchView() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ReturnType<typeof useAppStore.getState>["searchResults"]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const selectedPhoto = useAppStore((s) => s.selectedPhoto);
  const setSelectedPhoto = useAppStore((s) => s.setSelectedPhoto);

  const handleSearch = async () => {
    if (!query.trim()) return;
    setIsSearching(true);
    try {
      const res = await naturalLanguageSearch(query.trim());
      setResults(res.map((r) => r.photo));
    } catch (e) {
      console.error(e);
    } finally {
      setIsSearching(false);
    }
  };

  return (
    <div className="flex flex-col h-full p-6">
      <h2 className="text-lg font-semibold text-gray-800 mb-4">智能检索</h2>

      <div className="flex gap-2 mb-6">
        <div className="flex-1 relative">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            placeholder="输入自然语言描述，如：夕阳下的海边、猫的照片..."
            className="w-full pl-9 pr-4 py-2.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <button
          onClick={handleSearch}
          disabled={isSearching || !query.trim()}
          className="px-5 py-2.5 bg-blue-600 text-white text-sm font-medium rounded-md hover:bg-blue-700 disabled:opacity-50"
        >
          {isSearching ? "搜索中..." : "搜索"}
        </button>
      </div>

      {results.length > 0 && (
        <p className="text-xs text-gray-500 mb-3">找到 {results.length} 张相关照片</p>
      )}

      <div className="flex-1 overflow-auto scrollbar-thin">
        {results.length === 0 && !isSearching ? (
          <div className="flex flex-col items-center justify-center h-64 text-gray-400">
            <Search size={40} strokeWidth={1.2} className="mb-3 text-gray-300" />
            <p className="text-sm">输入描述开始检索</p>
          </div>
        ) : (
          <div className="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 gap-3">
            {results.map((photo) => (
              <div
                key={photo.id}
                onClick={() => setSelectedPhoto(photo)}
                className={`relative aspect-square rounded-lg overflow-hidden cursor-pointer border-2 transition-all ${
                  selectedPhoto?.id === photo.id
                    ? "border-blue-500 ring-2 ring-blue-100"
                    : "border-transparent hover:border-gray-300"
                }`}
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
        )}
      </div>
    </div>
  );
}
