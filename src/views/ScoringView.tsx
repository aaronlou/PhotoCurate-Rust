import { useState, useEffect } from "react";
import { useAppStore } from "@/stores/useAppStore";
import {
  getAiSettings,
  getPhotos,
  scorePhotos,
  validateApiKey,
  updateAiSettings,
  checkLocalModel,
} from "@/hooks/useInvoke";
import { Star, Play, Settings, Check, AlertCircle, Cpu, KeyRound, Server } from "lucide-react";
import type { AISettings, ScoringProvider } from "@/types";

const PROVIDERS: Array<{
  id: ScoringProvider;
  label: string;
  description: string;
  defaultModel: string;
  defaultBaseUrl: string;
}> = [
  {
    id: "gemini",
    label: "Gemini",
    description: "Google 原生视觉评分",
    defaultModel: "gemini-3.1-flash-lite",
    defaultBaseUrl: "",
  },
  {
    id: "qwen_vl",
    label: "Qwen-VL",
    description: "阿里云百炼视觉模型",
    defaultModel: "qwen3-vl-plus",
    defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  },
  {
    id: "openai_compatible_vision",
    label: "自定义",
    description: "OpenAI-compatible Vision API",
    defaultModel: "gpt-4o-mini",
    defaultBaseUrl: "",
  },
];

function providerConfig(provider: ScoringProvider) {
  return PROVIDERS.find((p) => p.id === provider) ?? PROVIDERS[0];
}

export default function ScoringView() {
  const photos = useAppStore((s) => s.photos);
  const setPhotos = useAppStore((s) => s.setPhotos);
  const aiSettings = useAppStore((s) => s.aiSettings);
  const setAiSettings = useAppStore((s) => s.setAiSettings);
  const isScoring = useAppStore((s) => s.isScoring);
  const setIsScoring = useAppStore((s) => s.setIsScoring);
  const scoreProgress = useAppStore((s) => s.scoreProgress);
  const setScoreProgress = useAppStore((s) => s.setScoreProgress);
  const isIndexing = useAppStore((s) => s.isIndexing);
  const indexProgress = useAppStore((s) => s.indexProgress);
  const photoSortOrder = useAppStore((s) => s.photoSortOrder);

  const [showSettings, setShowSettings] = useState(false);
  const [scoringProvider, setScoringProvider] = useState<ScoringProvider>("gemini");
  const [scoringModel, setScoringModel] = useState(providerConfig("gemini").defaultModel);
  const [scoringBaseUrl, setScoringBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [keyStatus, setKeyStatus] = useState<{ valid: boolean; message: string } | null>(null);
  const [localModelAvailable, setLocalModelAvailable] = useState<boolean | null>(null);
  const [scoreError, setScoreError] = useState<string | null>(null);

  const unscoredCount = photos.filter((p) => !p.has_been_scored).length;
  const indexedCount = photos.filter((p) => p.has_embedding).length;

  useEffect(() => {
    checkLocalModel().then(setLocalModelAvailable).catch(() => setLocalModelAvailable(false));
    getAiSettings().then((settings) => {
      setAiSettings(settings);
      if (settings) {
        syncSettingsForm(settings);
      }
    }).catch(console.error);
  }, [setAiSettings]);

  const syncSettingsForm = (settings: AISettings) => {
    const provider = settings.scoring_provider ?? "gemini";
    const config = providerConfig(provider);
    setScoringProvider(provider);
    setScoringModel(settings.scoring_model || config.defaultModel);
    setScoringBaseUrl(settings.scoring_base_url || config.defaultBaseUrl);
  };

  const handleProviderChange = (provider: ScoringProvider) => {
    const config = providerConfig(provider);
    setScoringProvider(provider);
    setScoringModel(config.defaultModel);
    setScoringBaseUrl(config.defaultBaseUrl);
    setApiKey("");
    setKeyStatus(null);
  };

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
    const result = await validateApiKey({
      scoring_provider: scoringProvider,
      scoring_model: scoringModel.trim() || providerConfig(scoringProvider).defaultModel,
      scoring_base_url: scoringBaseUrl.trim(),
      scoring_api_key: apiKey.trim(),
    });
    setKeyStatus(result);
    if (result.valid) {
      const settings = await updateAiSettings({
        provider: "gemini",
        scoring_provider: scoringProvider,
        scoring_model: scoringModel.trim(),
        scoring_base_url: scoringBaseUrl.trim(),
        scoring_api_key: apiKey.trim(),
      });
      setAiSettings(settings);
      syncSettingsForm(settings);
      setApiKey("");
    }
  };

  const handleSaveModelSettings = async () => {
    const settings = await updateAiSettings({
      provider: "gemini",
      scoring_provider: scoringProvider,
      scoring_model: scoringModel.trim(),
      scoring_base_url: scoringBaseUrl.trim(),
    });
    setAiSettings(settings);
    syncSettingsForm(settings);
    setKeyStatus({ valid: true, message: "模型设置已保存" });
  };

  const activeProvider = providerConfig(scoringProvider);
  const configuredModel = aiSettings?.scoring_model || activeProvider.defaultModel;
  const hasScoringKey = aiSettings?.scoring_provider === scoringProvider && Boolean(aiSettings?.has_scoring_api_key);
  const providerNeedsBaseUrl = scoringProvider !== "gemini";

  return (
    <div className="flex flex-col h-full p-6">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-semibold text-gray-800 flex items-center gap-2">
            <Star size={20} className="text-amber-500" />
            AI 评分
          </h2>
          <p className="text-sm text-gray-500 mt-1">
            使用 {providerConfig(aiSettings?.scoring_provider ?? scoringProvider).label} 评分，Chinese-CLIP 本地模型生成搜索向量
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
          <div className="flex items-start justify-between gap-3 mb-4">
            <div>
              <h3 className="text-sm font-medium text-gray-800">评分模型</h3>
              <p className="text-xs text-gray-500 mt-1">Gemini 用于默认评分和无本地模型时的搜索索引；Qwen-VL 只影响照片评分。</p>
            </div>
            <div className={`flex items-center gap-1.5 text-xs ${hasScoringKey ? "text-green-600" : "text-amber-600"}`}>
              <KeyRound size={13} />
              {hasScoringKey ? "已保存密钥" : "未保存密钥"}
            </div>
          </div>
          <div className="space-y-4">
            <div className="grid grid-cols-3 gap-2">
              {PROVIDERS.map((provider) => (
                <button
                  key={provider.id}
                  type="button"
                  onClick={() => handleProviderChange(provider.id)}
                  className={`rounded-md border px-3 py-2 text-left transition-colors ${
                    scoringProvider === provider.id
                      ? "border-blue-500 bg-blue-50 text-blue-700"
                      : "border-gray-200 bg-white text-gray-700 hover:border-gray-300"
                  }`}
                >
                  <span className="block text-sm font-medium">{provider.label}</span>
                  <span className="mt-0.5 block text-xs text-gray-500">{provider.description}</span>
                </button>
              ))}
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs text-gray-500 mb-1">模型</label>
                <input
                  type="text"
                  value={scoringModel}
                  onChange={(e) => {
                    setScoringModel(e.target.value);
                    setKeyStatus(null);
                  }}
                  placeholder={activeProvider.defaultModel}
                  className="w-full px-3 py-2 text-sm border border-gray-300 rounded-md bg-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Base URL</label>
                <div className="relative">
                  <Server size={14} className="absolute left-3 top-2.5 text-gray-400" />
                  <input
                    type="text"
                    value={scoringBaseUrl}
                    onChange={(e) => {
                      setScoringBaseUrl(e.target.value);
                      setKeyStatus(null);
                    }}
                    placeholder={providerNeedsBaseUrl ? activeProvider.defaultBaseUrl || "https://api.example.com/v1" : "Gemini 不需要填写"}
                    disabled={!providerNeedsBaseUrl}
                    className="w-full pl-8 pr-3 py-2 text-sm border border-gray-300 rounded-md bg-white disabled:bg-gray-100 disabled:text-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                </div>
              </div>
            </div>

            <div>
              <label className="block text-xs text-gray-500 mb-1">{activeProvider.label} API Key</label>
              <div className="flex gap-2">
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder={hasScoringKey ? "留空则继续使用已保存密钥" : `输入 ${activeProvider.label} API Key`}
                  className="flex-1 px-3 py-2 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <button
                  onClick={handleValidateKey}
                  disabled={!apiKey.trim()}
                  className="px-3 py-2 bg-gray-800 text-white text-sm rounded-md hover:bg-gray-900"
                >
                  验证
                </button>
                <button
                  onClick={handleSaveModelSettings}
                  className="px-3 py-2 bg-white border border-gray-300 text-gray-700 text-sm rounded-md hover:bg-gray-50"
                >
                  保存
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
            <div className="text-xs text-gray-500">
              当前评分: {providerConfig(aiSettings?.scoring_provider ?? scoringProvider).label}
              <span className="mx-1 text-gray-300">/</span>
              {configuredModel}
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
