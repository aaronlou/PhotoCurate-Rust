import { useEffect, useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import {
  checkLocalModel,
  getAiSettings,
  scorePhotos,
} from "@/hooks/useInvoke";
import {
  AlertCircle,
  Cpu,
  KeyRound,
  Play,
  Settings,
  Sparkles,
  Star,
  BarChart3,
} from "lucide-react";
import {
  AiServiceMode,
  persistAiServiceMode,
  providerConfig,
  readStoredAiServiceMode,
  resolveInitialAiServiceMode,
} from "@/lib/aiService";

export default function ScoringView() {
  const photos = useAppStore((s) => s.photos);
  const refreshPhotos = useAppStore((s) => s.refreshPhotos);
  const aiSettings = useAppStore((s) => s.aiSettings);
  const setAiSettings = useAppStore((s) => s.setAiSettings);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const isScoring = useAppStore((s) => s.isScoring);
  const setIsScoring = useAppStore((s) => s.setIsScoring);
  const scoreProgress = useAppStore((s) => s.scoreProgress);
  const setScoreProgress = useAppStore((s) => s.setScoreProgress);
  const isIndexing = useAppStore((s) => s.isIndexing);
  const indexProgress = useAppStore((s) => s.indexProgress);
  const photoSortOrder = useAppStore((s) => s.photoSortOrder);

  const [localModelAvailable, setLocalModelAvailable] = useState<boolean | null>(null);
  const [scoreError, setScoreError] = useState<string | null>(null);
  const [scoreNotice, setScoreNotice] = useState<string | null>(null);
  const [analysisComplete, setAnalysisComplete] = useState(false);
  const [allowSavedKeyRead, setAllowSavedKeyRead] = useState(false);
  const [scoringScope, setScoringScope] = useState<"missing_evaluation" | "all">("missing_evaluation");
  const [serviceMode, setServiceMode] = useState<AiServiceMode>(() => readStoredAiServiceMode() ?? "photocurate_ai");

  const photosMissingEvaluation = photos.filter((p) => !p.latest_evaluation);
  const pendingEvaluationCount = photosMissingEvaluation.length;
  const legacyScoreOnlyCount = photos.filter((p) => p.has_been_scored && !p.latest_evaluation).length;
  const evaluatedCount = photos.filter((p) => p.latest_evaluation).length;
  const scoringCandidates = scoringScope === "all" ? photos : photosMissingEvaluation;
  const scoringTargetCount = Math.min(scoringCandidates.length, 50);
  const indexedCount = photos.filter((p) => p.has_embedding).length;
  const scoringProvider = aiSettings?.scoring_provider ?? "gemini";
  const activeProvider = providerConfig(scoringProvider);
  const configuredModel = aiSettings?.scoring_model || activeProvider.defaultModel;
  const hasScoringKey = Boolean(aiSettings?.has_scoring_api_key);
  const canUseManagedAi = false;

  useEffect(() => {
    checkLocalModel().then(setLocalModelAvailable).catch(() => setLocalModelAvailable(false));
    getAiSettings()
      .then((settings) => {
        setAiSettings(settings);
        if (settings) {
          const resolvedMode = resolveInitialAiServiceMode(settings.has_scoring_api_key);
          setServiceMode(resolvedMode);
          persistAiServiceMode(resolvedMode);
        }
      })
      .catch(console.error);
  }, [setAiSettings]);

  const handleStartScoring = async () => {
    if (serviceMode === "photocurate_ai") {
      setScoreError(null);
      setScoreNotice("PhotoCurate AI 托管服务还在准备中。当前版本请到 AI 服务页切换为“自带 API Key”。");
      return;
    }

    const targets = (scoringScope === "all" ? photos : photos.filter((p) => !p.latest_evaluation)).slice(0, 50);
    if (targets.length === 0) return;

    if (!hasScoringKey) {
      setScoreError(null);
      setScoreNotice("请先到 AI 服务页填写并验证 API Key，再开始生成评价。");
      return;
    }

    if (!allowSavedKeyRead) {
      setScoreError(null);
      setScoreNotice("即将读取已保存的 API Key。请先勾选下方说明，确认后再开始评分。");
      return;
    }

    setScoreError(null);
    setScoreNotice(null);
    setAnalysisComplete(false);
    setIsScoring(true);
    setScoreProgress({ current: 0, total: targets.length });

    try {
      await scorePhotos(targets.map((p) => p.id), true);
      await refreshPhotos(photoSortOrder);
      setAnalysisComplete(true);
    } catch (e) {
      setScoreError(typeof e === "string" ? e : String(e));
      setScoreNotice(null);
      setIsScoring(false);
      setScoreProgress(null);
    }
  };

  return (
    <div className="flex h-full flex-col bg-[#f7f8fa]">
      <header className="border-b border-gray-200 bg-white px-6 py-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="flex items-center gap-2 text-lg font-semibold text-gray-800">
              <Star size={20} className="text-amber-500" />
              作品分析
            </h2>
            <p className="mt-1 text-sm text-gray-500">
              补全评分、点评、标签和维度数据，为洞察、检索和精选导出提供基础
            </p>
            {localModelAvailable !== null && (
              <div className={`mt-2 flex items-center gap-1.5 text-xs ${localModelAvailable ? "text-green-600" : "text-amber-600"}`}>
                <Cpu size={12} />
                {localModelAvailable ? "本地 Chinese-CLIP 模型已加载" : "本地模型未加载，搜索将使用 Gemini API"}
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={() => setCurrentView("ai_service")}
            className="flex items-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50"
          >
            <Settings size={14} />
            管理 AI 服务
          </button>
        </div>
      </header>

      <main className="flex-1 overflow-auto p-6">
        <div className="mb-5 grid grid-cols-[1fr_0.85fr] gap-4">
          <ServiceSummaryCard
            serviceMode={serviceMode}
            providerLabel={activeProvider.label}
            configuredModel={configuredModel}
            hasScoringKey={hasScoringKey}
            canUseManagedAi={canUseManagedAi}
            onManage={() => setCurrentView("ai_service")}
          />
          <div className="rounded-lg border border-gray-200 bg-white p-4">
            <p className="text-xs font-medium text-gray-500">索引状态</p>
            <p className="mt-2 text-2xl font-semibold tracking-tight text-gray-900">
              {indexedCount}/{photos.length}
            </p>
            <p className="mt-1 text-xs text-gray-400">
              {isIndexing && indexProgress
                ? `后台索引中 ${indexProgress.current}/${indexProgress.total}`
                : "用于自然语言检索的向量索引"}
            </p>
          </div>
        </div>

        {serviceMode === "byok" && hasScoringKey && (
          <label className="mb-4 flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
            <input
              type="checkbox"
              checked={allowSavedKeyRead}
              onChange={(e) => setAllowSavedKeyRead(e.target.checked)}
              className="mt-0.5"
            />
            <span>
              开始评分时会从 macOS 钥匙串读取 PhotoCurate 保存的 {activeProvider.label} API Key，
              只用于本次连接 {activeProvider.label} 评分服务；不会读取其他钥匙串项目，也不会把 Key 显示在界面上。
            </span>
          </label>
        )}

        <section className="rounded-lg border border-gray-200 bg-white p-6">
          <div className="flex items-center justify-between gap-5">
            <div>
              <p className="text-sm text-gray-600">
                待生成评价: <span className="font-semibold text-gray-800">{pendingEvaluationCount}</span> 张
              </p>
              <p className="mt-1 text-xs text-gray-400">
                仅有旧评分: {legacyScoreOnlyCount} 张
                <span className="mx-1 text-gray-300">/</span>
                已有评价: {evaluatedCount} 张
              </p>
            </div>
            <div className="flex items-center gap-3">
              <div className="flex rounded-md border border-gray-200 bg-gray-50 p-0.5">
                <button
                  type="button"
                  onClick={() => setScoringScope("missing_evaluation")}
                  className={`rounded px-3 py-1.5 text-xs font-medium ${
                    scoringScope === "missing_evaluation"
                      ? "bg-white text-blue-600 shadow-sm"
                      : "text-gray-500 hover:text-gray-700"
                  }`}
                >
                  补全缺失
                </button>
                <button
                  type="button"
                  onClick={() => setScoringScope("all")}
                  className={`rounded px-3 py-1.5 text-xs font-medium ${
                    scoringScope === "all"
                      ? "bg-white text-blue-600 shadow-sm"
                      : "text-gray-500 hover:text-gray-700"
                  }`}
                >
                  重新评价全部
                </button>
              </div>
              <button
                onClick={handleStartScoring}
                disabled={isScoring || scoringTargetCount === 0 || (serviceMode === "photocurate_ai" && !canUseManagedAi)}
                className="flex items-center gap-2 rounded-md bg-blue-600 px-5 py-2.5 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Play size={16} />
                {isScoring
                  ? "评价中..."
                  : serviceMode === "photocurate_ai"
                  ? "即将开放"
                  : scoringScope === "all"
                  ? "重新评价"
                  : "生成评价"}
              </button>
            </div>
          </div>

          <p className="mt-3 text-xs text-gray-400">
            本次队列: {scoringTargetCount} 张
            {scoringScope === "all" && "，会重新调用 AI 并更新评分"}
          </p>

          {scoreError && (
            <div className="mt-3 flex items-center gap-1.5 text-xs text-red-500">
              <AlertCircle size={14} />
              {scoreError}
            </div>
          )}

          {scoreNotice && (
            <div className="mt-3 flex items-center gap-1.5 text-xs text-amber-600">
              <AlertCircle size={14} />
              {scoreNotice}
            </div>
          )}

          {analysisComplete && (
            <div className="mt-4 flex items-center justify-between gap-4 rounded-md border border-green-200 bg-green-50 px-3 py-3">
              <div className="flex items-center gap-2 text-sm text-green-800">
                <Sparkles size={15} />
                分析已完成，可以查看更新后的作品洞察。
              </div>
              <button
                type="button"
                onClick={() => setCurrentView("insights")}
                className="flex items-center gap-1.5 rounded-md bg-green-600 px-3 py-2 text-xs font-medium text-white hover:bg-green-700"
              >
                <BarChart3 size={14} />
                查看洞察
              </button>
            </div>
          )}

          {isScoring && scoreProgress && (
            <div className="mt-4">
              <div className="mb-1 flex justify-between text-xs text-gray-500">
                <span>评价进度</span>
                <span>
                  {scoreProgress.current} / {scoreProgress.total}
                </span>
              </div>
              <div className="h-2 w-full rounded-full bg-gray-100">
                <div
                  className="h-2 rounded-full bg-blue-600 transition-all"
                  style={{
                    width: `${(scoreProgress.current / scoreProgress.total) * 100}%`,
                  }}
                />
              </div>
            </div>
          )}
        </section>
      </main>
    </div>
  );
}

function ServiceSummaryCard({
  serviceMode,
  providerLabel,
  configuredModel,
  hasScoringKey,
  canUseManagedAi,
  onManage,
}: {
  serviceMode: AiServiceMode;
  providerLabel: string;
  configuredModel: string;
  hasScoringKey: boolean;
  canUseManagedAi: boolean;
  onManage: () => void;
}) {
  const isManaged = serviceMode === "photocurate_ai";
  const statusText = isManaged
    ? canUseManagedAi ? "可用" : "即将开放"
    : hasScoringKey ? "已配置" : "未配置";

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            {isManaged ? <Sparkles size={16} className="text-blue-600" /> : <KeyRound size={16} className="text-gray-700" />}
            <h3 className="text-sm font-semibold text-gray-800">
              {isManaged ? "PhotoCurate AI" : "自带 API Key"}
            </h3>
            <span className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${
              isManaged && !canUseManagedAi
                ? "bg-gray-100 text-gray-500"
                : hasScoringKey || canUseManagedAi
                ? "bg-green-50 text-green-700"
                : "bg-amber-50 text-amber-700"
            }`}>
              {statusText}
            </span>
          </div>
          <p className="mt-2 text-xs leading-5 text-gray-500">
            {isManaged
              ? "托管 AI 未来会提供一站式评分、点评和分析额度。当前版本请切换到自带 API Key 继续生成评价。"
              : `当前使用 ${providerLabel} / ${configuredModel} 生成评分和结构化点评。`}
          </p>
        </div>
        <button
          type="button"
          onClick={onManage}
          className="shrink-0 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50"
        >
          管理
        </button>
      </div>
    </section>
  );
}
