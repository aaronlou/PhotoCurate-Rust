import { useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import { exportPhotos, pickDirectory } from "@/hooks/useInvoke";
import { Download, Check, FolderOpen, AlertTriangle, FileImage, FolderTree, List, WandSparkles } from "lucide-react";
import type { ExportResult } from "@/types";

export default function ExportView() {
  const photos = useAppStore((s) => s.photos);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const [minScore, setMinScore] = useState(70);
  const [destination, setDestination] = useState("");
  const [isExporting, setIsExporting] = useState(false);
  const [lastResult, setLastResult] = useState<ExportResult | null>(null);
  const [preserveStructure, setPreserveStructure] = useState(false);

  const scoredPhotos = photos.filter((p) => p.has_been_scored && (p.aesthetic_score ?? 0) >= minScore);
  const analyzedCount = photos.filter((p) => p.has_been_scored).length;
  const needsAnalysis = photos.length > 0 && analyzedCount === 0;

  const handlePickDestination = async () => {
    const path = await pickDirectory();
    if (path) setDestination(path);
  };

  const handleExport = async () => {
    if (!destination || scoredPhotos.length === 0) return;
    setIsExporting(true);
    setLastResult(null);
    try {
      const result = await exportPhotos(
        scoredPhotos.map((p) => p.id),
        destination,
        preserveStructure
      );
      setLastResult(result);
    } catch (e) {
      console.error(e);
      alert(`导出失败: ${e}`);
    } finally {
      setIsExporting(false);
    }
  };

  const hasFailures = lastResult && lastResult.failed_count > 0;
  const allSuccess = lastResult && lastResult.failed_count === 0 && lastResult.exported_count > 0;

  return (
    <div className="flex flex-col h-full p-6 overflow-auto">
      <h2 className="text-lg font-semibold text-gray-800 mb-4 flex items-center gap-2">
        <Download size={20} />
        精选导出
      </h2>

      {needsAnalysis && (
        <div className="mb-4 flex items-center justify-between gap-4 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3">
          <div className="flex items-start gap-2">
            <WandSparkles size={16} className="mt-0.5 shrink-0 text-amber-700" />
            <div>
              <p className="text-sm font-medium text-amber-900">先完成 AI 分析，才能按评分精选导出</p>
              <p className="mt-1 text-xs text-amber-700">
                当前图库还没有已评分照片。分析完成后，这里会自动筛选高分作品。
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => setCurrentView("scoring")}
            className="shrink-0 rounded-md bg-amber-600 px-3 py-2 text-xs font-medium text-white hover:bg-amber-700"
          >
            去作品分析
          </button>
        </div>
      )}

      <div className="bg-white rounded-lg border border-gray-200 p-5 space-y-5 mb-6">
        {/* Score threshold */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">
            最低评分阈值: {minScore}
          </label>
          <input
            type="range"
            min={0}
            max={100}
            value={minScore}
            onChange={(e) => {
              setMinScore(Number(e.target.value));
              setLastResult(null);
            }}
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-400 mt-1">
            <span>0</span>
            <span>50</span>
            <span>100</span>
          </div>
        </div>

        {/* Export mode */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">导出方式</label>
          <div className="flex gap-3">
            <button
              onClick={() => setPreserveStructure(false)}
              className={`flex items-center gap-2 px-4 py-2.5 rounded-md border text-sm transition-colors ${
                !preserveStructure
                  ? "border-blue-500 bg-blue-50 text-blue-700"
                  : "border-gray-200 text-gray-600 hover:bg-gray-50"
              }`}
            >
              <List size={16} />
              扁平导出
            </button>
            <button
              onClick={() => setPreserveStructure(true)}
              className={`flex items-center gap-2 px-4 py-2.5 rounded-md border text-sm transition-colors ${
                preserveStructure
                  ? "border-blue-500 bg-blue-50 text-blue-700"
                  : "border-gray-200 text-gray-600 hover:bg-gray-50"
              }`}
            >
              <FolderTree size={16} />
              保留原目录结构
            </button>
          </div>
          <p className="text-xs text-gray-400 mt-1.5">
            {preserveStructure
              ? "按原始文件夹层级复制到目标目录"
              : "所有照片直接复制到目标目录，重名自动加序号"}
          </p>
        </div>

        {/* Destination */}
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

        {/* Action bar */}
        <div className="pt-2 border-t border-gray-100">
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-600">
              将导出 <span className="font-semibold text-gray-800">{scoredPhotos.length}</span> 张照片
            </span>
            <button
              onClick={handleExport}
              disabled={isExporting || !destination || scoredPhotos.length === 0}
              className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 text-white text-sm font-medium rounded-md hover:bg-blue-700 disabled:opacity-50 transition-colors"
            >
              {isExporting ? (
                <>
                  <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  导出中...
                </>
              ) : allSuccess ? (
                <>
                  <Check size={16} />
                  导出完成
                </>
              ) : (
                <>
                  <Download size={16} />
                  开始导出
                </>
              )}
            </button>
          </div>
        </div>
      </div>

      {/* Results */}
      {lastResult && (
        <div
          className={`rounded-lg border p-4 ${
            hasFailures
              ? "bg-amber-50 border-amber-200"
              : "bg-green-50 border-green-200"
          }`}
        >
          <div className="flex items-center gap-2 mb-3">
            {hasFailures ? (
              <AlertTriangle size={18} className="text-amber-600" />
            ) : (
              <Check size={18} className="text-green-600" />
            )}
            <span className={`font-medium ${hasFailures ? "text-amber-800" : "text-green-800"}`}>
              {hasFailures
                ? `导出完成，${lastResult.exported_count} 张成功，${lastResult.failed_count} 张失败`
                : `成功导出 ${lastResult.exported_count} 张照片`}
            </span>
          </div>

          {hasFailures && lastResult.failed_photos.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs text-amber-700 font-medium">失败详情：</p>
              <div className="max-h-48 overflow-auto rounded-md bg-white border border-amber-100">
                {lastResult.failed_photos.map((f) => (
                  <div
                    key={f.id}
                    className="flex items-start gap-2 px-3 py-2 text-xs border-b border-amber-50 last:border-0"
                  >
                    <FileImage size={14} className="text-amber-500 mt-0.5 shrink-0" />
                    <div className="min-w-0">
                      <p className="text-gray-700 truncate">{f.file_name}</p>
                      <p className="text-red-500">{f.error}</p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
