import { useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import { exportPhotos, pickDirectory } from "@/hooks/useInvoke";
import { Download, Check, FolderOpen } from "lucide-react";

export default function ExportView() {
  const photos = useAppStore((s) => s.photos);
  const [minScore, setMinScore] = useState(70);
  const [destination, setDestination] = useState("");
  const [isExporting, setIsExporting] = useState(false);
  const [exported, setExported] = useState(false);

  const scoredPhotos = photos.filter((p) => p.has_been_scored && (p.aesthetic_score ?? 0) >= minScore);

  const handlePickDestination = async () => {
    const path = await pickDirectory();
    if (path) setDestination(path);
  };

  const handleExport = async () => {
    if (!destination || scoredPhotos.length === 0) return;
    setIsExporting(true);
    try {
      await exportPhotos(scoredPhotos.map((p) => p.id), destination);
      setExported(true);
    } catch (e) {
      console.error(e);
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <div className="flex flex-col h-full p-6">
      <h2 className="text-lg font-semibold text-gray-800 mb-4 flex items-center gap-2">
        <Download size={20} />
        精选导出
      </h2>

      <div className="bg-white rounded-lg border border-gray-200 p-5 space-y-5 mb-6">
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">
            最低评分阈值: {minScore}
          </label>
          <input
            type="range"
            min={0}
            max={100}
            value={minScore}
            onChange={(e) => setMinScore(Number(e.target.value))}
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-400 mt-1">
            <span>0</span>
            <span>50</span>
            <span>100</span>
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">导出目标文件夹</label>
          <div className="flex gap-2">
            <input
              type="text"
              readOnly
              value={destination}
              placeholder="选择导出文件夹"
              className="flex-1 px-3 py-2 text-sm border border-gray-300 rounded-md bg-gray-50"
            />
            <button
              onClick={handlePickDestination}
              className="px-3 py-2 bg-gray-100 text-gray-700 text-sm rounded-md hover:bg-gray-200 flex items-center gap-1.5"
            >
              <FolderOpen size={14} />
              选择
            </button>
          </div>
        </div>

        <div className="pt-2 border-t border-gray-100">
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-600">
              将导出 <span className="font-semibold text-gray-800">{scoredPhotos.length}</span> 张照片
            </span>
            <button
              onClick={handleExport}
              disabled={isExporting || !destination || scoredPhotos.length === 0}
              className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 text-white text-sm font-medium rounded-md hover:bg-blue-700 disabled:opacity-50"
            >
              {exported ? <Check size={16} /> : <Download size={16} />}
              {isExporting ? "导出中..." : exported ? "导出完成" : "开始导出"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
