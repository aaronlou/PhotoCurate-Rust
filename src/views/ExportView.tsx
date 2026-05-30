import { useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import { exportPhotos, pickDirectory } from "@/hooks/useInvoke";
import { useI18n } from "@/lib/i18n";
import { Download, Check, FolderOpen, AlertTriangle, FileImage, FolderTree, List, WandSparkles } from "lucide-react";
import type { ExportResult } from "@/types";

export default function ExportView() {
  const { t } = useI18n();
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
      alert(t("export.failedAlert", { error: String(e) }));
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
        {t("export.title")}
      </h2>

      {needsAnalysis && (
        <div className="mb-4 flex items-center justify-between gap-4 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3">
          <div className="flex items-start gap-2">
            <WandSparkles size={16} className="mt-0.5 shrink-0 text-amber-700" />
            <div>
              <p className="text-sm font-medium text-amber-900">{t("export.needsAnalysisTitle")}</p>
              <p className="mt-1 text-xs text-amber-700">
                {t("export.needsAnalysisDescription")}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => setCurrentView("scoring")}
            className="shrink-0 rounded-md bg-amber-600 px-3 py-2 text-xs font-medium text-white hover:bg-amber-700"
          >
            {t("export.goScoring")}
          </button>
        </div>
      )}

      <div className="bg-white rounded-lg border border-gray-200 p-5 space-y-5 mb-6">
        {/* Score threshold */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">
            {t("export.minScore", { score: minScore })}
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
          <label className="block text-sm font-medium text-gray-700 mb-2">{t("export.mode")}</label>
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
              {t("export.flat")}
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
              {t("export.preserveStructure")}
            </button>
          </div>
          <p className="text-xs text-gray-400 mt-1.5">
            {preserveStructure
              ? t("export.preserveDescription")
              : t("export.flatDescription")}
          </p>
        </div>

        {/* Destination */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">{t("export.destination")}</label>
          <div className="flex gap-2">
            <input
              type="text"
              readOnly
              value={destination}
              placeholder={t("export.destinationPlaceholder")}
              className="flex-1 px-3 py-2 text-sm border border-gray-300 rounded-md bg-gray-50"
            />
            <button
              onClick={handlePickDestination}
              className="px-3 py-2 bg-gray-100 text-gray-700 text-sm rounded-md hover:bg-gray-200 flex items-center gap-1.5"
            >
              <FolderOpen size={14} />
              {t("export.choose")}
            </button>
          </div>
        </div>

        {/* Action bar */}
        <div className="pt-2 border-t border-gray-100">
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-600">
              {t("export.count", { count: scoredPhotos.length })}
            </span>
            <button
              onClick={handleExport}
              disabled={isExporting || !destination || scoredPhotos.length === 0}
              className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 text-white text-sm font-medium rounded-md hover:bg-blue-700 disabled:opacity-50 transition-colors"
            >
              {isExporting ? (
                <>
                  <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  {t("export.exporting")}
                </>
              ) : allSuccess ? (
                <>
                  <Check size={16} />
                  {t("export.done")}
                </>
              ) : (
                <>
                  <Download size={16} />
                  {t("export.start")}
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
                ? t("export.resultPartial", { success: lastResult.exported_count, failed: lastResult.failed_count })
                : t("export.resultSuccess", { count: lastResult.exported_count })}
            </span>
          </div>

          {hasFailures && lastResult.failed_photos.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs text-amber-700 font-medium">{t("export.failureDetails")}</p>
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
