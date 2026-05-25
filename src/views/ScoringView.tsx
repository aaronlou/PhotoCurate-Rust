import { useState, useEffect } from "react";
import { useAppStore } from "@/stores/useAppStore";
import { getPhotos, scorePhotos, validateApiKey, updateAiSettings, checkLocalModel } from "@/hooks/useInvoke";
import { Star, Play, Settings, Check, AlertCircle, Cpu } from "lucide-react";

export default function ScoringView() {
  const photos = useAppStore((s) => s.photos);
  const setPhotos = useAppStore((s) => s.setPhotos);
  const isScoring = useAppStore((s) => s.isScoring);
  const setIsScoring = useAppStore((s) => s.setIsScoring);
  const scoreProgress = useAppStore((s) => s.scoreProgress);
  const setScoreProgress = useAppStore((s) => s.setScoreProgress);
  const isIndexing = useAppStore((s) => s.isIndexing);
  const indexProgress = useAppStore((s) => s.indexProgress);
  const photoSortOrder = useAppStore((s) => s.photoSortOrder);

  const [showSettings, setShowSettings] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [keyStatus, setKeyStatus] = useState<{ valid: boolean; message: string } | null>(null);
  const [localModelAvailable, setLocalModelAvailable] = useState<boolean | null>(null);
  const [scoreError, setScoreError] = useState<string | null>(null);

  const unscoredCount = photos.filter((p) => !p.has_been_scored).length;
  const indexedCount = photos.filter((p) => p.has_embedding).length;

  useEffect(() => {
    checkLocalModel().then(setLocalModelAvailable).catch(() => setLocalModelAvailable(false));
  }, []);

  const handleStartScoring = async () => {
    const unscored = photos.filter((p) => !p.has_been_scored).slice(0, 50);
    if (unscored.length === 0) return;

    setScoreError(null);
    setIsScoring(true);
    setScoreProgress({ current: 0, total: unscored.length });

    try {
      await scorePhotos(unscored.map((p) => p.id));
      const updated = await getPhotos(photoSortOrder);
      setPhotos(updated);
    } catch (e) {
      setScoreError(typeof e === "string" ? e : String(e));
      setIsScoring(false);
      setScoreProgress(null);
    }
  };

  const handleValidateKey = async () => {
    if (!apiKey.trim()) return;
    const result = await validateApiKey(apiKey.trim());
    setKeyStatus(result);
    if (result.valid) {
      await updateAiSettings({ api_key: apiKey.trim(), provider: "gemini" });
    }
  };

  return (
    <div className="flex flex-col h-full p-6">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-semibold text-gray-800 flex items-center gap-2">
            <Star size={20} className="text-amber-500" />
            AI 评分
          </h2>
          <p className="text-sm text-gray-500 mt-1">
            使用 Gemini AI 评分，Chinese-CLIP 本地模型生成搜索向量
          </p>
          {localModelAvailable !== null && (
            <div className={`flex items-center gap-1.5 mt-1 text-xs ${localModelAvailable ? "text-green-600" : "text-amber-600"}`}>
              <Cpu size={12} />
              {localModelAvailable ? "本地 Chinese-CLIP 模型已加载" : "本地模型未加载，搜索将使用 Gemini API"}
            </div>
          )}
          <div className="flex items-center gap-1.5 mt-1 text-xs text-gray-500">
            <span>
              已索引: {indexedCount}/{photos.length} 张
              {isIndexing && indexProgress && `（后台索引中 ${indexProgress.current}/${indexProgress.total}）`}
            </span>
          </div>
        </div>
        <button
          onClick={() => setShowSettings(!showSettings)}
          className="p-2 text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-md"
        >
          <Settings size={18} />
        </button>
      </div>

      {showSettings && (
        <div className="bg-gray-50 rounded-lg p-4 mb-6 border border-gray-200">
          <h3 className="text-sm font-medium text-gray-700 mb-3">API 设置</h3>
          <div className="space-y-3">
            <div>
              <label className="block text-xs text-gray-500 mb-1">Gemini API Key</label>
              <div className="flex gap-2">
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="输入 Gemini API Key"
                  className="flex-1 px-3 py-2 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <button
                  onClick={handleValidateKey}
                  className="px-3 py-2 bg-gray-800 text-white text-sm rounded-md hover:bg-gray-900"
                >
                  验证
                </button>
              </div>
              {keyStatus && (
                <div
                  className={`flex items-center gap-1.5 mt-2 text-xs ${
                    keyStatus.valid ? "text-green-600" : "text-red-500"
                  }`}
                >
                  {keyStatus.valid ? <Check size={14} /> : <AlertCircle size={14} />}
                  {keyStatus.message}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      <div className="bg-white rounded-lg border border-gray-200 p-6 mb-4">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-gray-600">
              待评分照片: <span className="font-semibold text-gray-800">{unscoredCount}</span> 张
            </p>
            <p className="text-xs text-gray-400 mt-1">
              已评分: {photos.filter((p) => p.has_been_scored).length} 张
            </p>
          </div>
          <button
            onClick={handleStartScoring}
            disabled={isScoring || unscoredCount === 0}
            className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 text-white text-sm font-medium rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Play size={16} />
            {isScoring ? "评分中..." : "开始评分"}
          </button>
        </div>

        {scoreError && (
          <div className="mt-3 flex items-center gap-1.5 text-xs text-red-500">
            <AlertCircle size={14} />
            {scoreError}
          </div>
        )}

        {isScoring && scoreProgress && (
          <div className="mt-4">
            <div className="flex justify-between text-xs text-gray-500 mb-1">
              <span>评分进度</span>
              <span>
                {scoreProgress.current} / {scoreProgress.total}
              </span>
            </div>
            <div className="w-full bg-gray-100 rounded-full h-2">
              <div
                className="bg-blue-600 h-2 rounded-full transition-all"
                style={{
                  width: `${(scoreProgress.current / scoreProgress.total) * 100}%`,
                }}
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
