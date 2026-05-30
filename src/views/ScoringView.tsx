import { useEffect, useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import {
  checkLocalModel,
  getAiSettings,
  getBillingStatus,
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
import { billingStatusTranslationKey, formatCredits } from "@/lib/billing";
import { useI18n, type TranslationKey } from "@/lib/i18n";
import type { BillingStatus } from "@/types";

function localizedProviderLabel(provider: ReturnType<typeof providerConfig>, t: (key: TranslationKey) => string) {
  if (provider.id === "openai_compatible_vision") {
    return t("provider.openai_compatible_vision.label");
  }
  return provider.label;
}

export default function ScoringView() {
  const { t } = useI18n();
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
  const [lastRunSummary, setLastRunSummary] = useState<string | null>(null);
  const [lastRunHadFailures, setLastRunHadFailures] = useState(false);
  const [allowSavedKeyRead, setAllowSavedKeyRead] = useState(false);
  const [scoringScope, setScoringScope] = useState<"missing_evaluation" | "all">("missing_evaluation");
  const [serviceMode, setServiceMode] = useState<AiServiceMode>(() => readStoredAiServiceMode() ?? "photocurate_ai");
  const [billingStatus, setBillingStatus] = useState<BillingStatus | null>(null);

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
  const canUseManagedAi = Boolean(billingStatus?.canUseManagedAi);
  const managedAiRuntimeAvailable = false;

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
    getBillingStatus().then(setBillingStatus).catch(console.error);
  }, [setAiSettings]);

  const handleStartScoring = async () => {
    if (serviceMode === "photocurate_ai") {
      setScoreError(null);
      setScoreNotice(
        canUseManagedAi
          ? t("scoring.managedRuntimePendingWithSubscription")
          : billingStatus?.message ??
              t("scoring.managedNotConfigured")
      );
      return;
    }

    const targets = (scoringScope === "all" ? photos : photos.filter((p) => !p.latest_evaluation)).slice(0, 50);
    if (targets.length === 0) return;

    if (!hasScoringKey) {
      setScoreError(null);
      setScoreNotice(t("scoring.missingApiKeyNotice"));
      return;
    }

    if (!allowSavedKeyRead) {
      setScoreError(null);
      setScoreNotice(t("scoring.savedKeyNotice"));
      return;
    }

    setScoreError(null);
    setScoreNotice(null);
    setAnalysisComplete(false);
    setLastRunSummary(null);
    setLastRunHadFailures(false);
    setIsScoring(true);
    setScoreProgress({ current: 0, total: targets.length });

    try {
      const result = await scorePhotos(targets.map((p) => p.id), true);
      await refreshPhotos(photoSortOrder);
      setAnalysisComplete(true);
      setLastRunHadFailures(result.failed_count > 0);
      if (result.failed_count > 0) {
        setLastRunSummary(
          t("scoring.runPartialComplete", { success: result.success_count, failed: result.failed_count })
        );
      } else {
        setLastRunSummary(t("scoring.runComplete", { count: result.success_count }));
      }
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
              {t("scoring.title")}
            </h2>
            <p className="mt-1 text-sm text-gray-500">
              {t("scoring.subtitle")}
            </p>
            {localModelAvailable !== null && (
              <div className={`mt-2 flex items-center gap-1.5 text-xs ${localModelAvailable ? "text-green-600" : "text-amber-600"}`}>
                <Cpu size={12} />
                {localModelAvailable ? t("scoring.localModelLoaded") : t("scoring.localModelMissing")}
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={() => setCurrentView("ai_service")}
            className="flex items-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50"
          >
            <Settings size={14} />
            {t("scoring.manageAiService")}
          </button>
        </div>
      </header>

      <main className="flex-1 overflow-auto p-6">
        <div className="mb-5 grid grid-cols-[1fr_0.85fr] gap-4">
          <ServiceSummaryCard
            serviceMode={serviceMode}
            providerLabel={localizedProviderLabel(activeProvider, t)}
            configuredModel={configuredModel}
            hasScoringKey={hasScoringKey}
            canUseManagedAi={canUseManagedAi}
            managedAiRuntimeAvailable={managedAiRuntimeAvailable}
            billingStatus={billingStatus}
            onManage={() => setCurrentView("ai_service")}
          />
          <div className="rounded-lg border border-gray-200 bg-white p-4">
            <p className="text-xs font-medium text-gray-500">{t("scoring.indexStatus")}</p>
            <p className="mt-2 text-2xl font-semibold tracking-tight text-gray-900">
              {indexedCount}/{photos.length}
            </p>
            <p className="mt-1 text-xs text-gray-400">
              {isIndexing && indexProgress
                ? t("scoring.backgroundIndexing", { current: indexProgress.current, total: indexProgress.total })
                : t("scoring.vectorIndex")}
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
              {t("scoring.keychainConsent", { provider: localizedProviderLabel(activeProvider, t) })}
            </span>
          </label>
        )}

        <section className="rounded-lg border border-gray-200 bg-white p-6">
          <div className="flex items-center justify-between gap-5">
            <div>
              <p className="text-sm text-gray-600">
                {t("scoring.pendingEvaluations", { count: pendingEvaluationCount })}
              </p>
              <p className="mt-1 text-xs text-gray-400">
                {t("scoring.legacyScoreOnly", { count: legacyScoreOnlyCount })}
                <span className="mx-1 text-gray-300">/</span>
                {t("scoring.evaluated", { count: evaluatedCount })}
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
                  {t("scoring.fillMissing")}
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
                  {t("scoring.rescoreAll")}
                </button>
              </div>
              <button
                onClick={handleStartScoring}
                disabled={
                  isScoring ||
                  scoringTargetCount === 0 ||
                  (serviceMode === "photocurate_ai" && (!canUseManagedAi || !managedAiRuntimeAvailable))
                }
                className="flex items-center gap-2 rounded-md bg-blue-600 px-5 py-2.5 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Play size={16} />
                {isScoring
                  ? t("scoring.evaluating")
                  : serviceMode === "photocurate_ai"
                  ? canUseManagedAi && managedAiRuntimeAvailable
                    ? t("scoring.generateEvaluation")
                    : canUseManagedAi
                    ? t("scoring.runtimePending")
                    : t("scoring.subscriptionNeeded")
                  : scoringScope === "all"
                  ? t("scoring.rescore")
                  : t("scoring.generateEvaluation")}
              </button>
            </div>
          </div>

          <p className="mt-3 text-xs text-gray-400">
            {t("scoring.queue", { count: scoringTargetCount })}
            {scoringScope === "all" && t("scoring.rescoreNote")}
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
            <div
              className={`mt-4 flex items-center justify-between gap-4 rounded-md border px-3 py-3 ${
                lastRunHadFailures ? "border-amber-200 bg-amber-50" : "border-green-200 bg-green-50"
              }`}
            >
              <div
                className={`flex items-center gap-2 text-sm ${
                  lastRunHadFailures ? "text-amber-800" : "text-green-800"
                }`}
              >
                {lastRunHadFailures ? <AlertCircle size={15} /> : <Sparkles size={15} />}
                {lastRunSummary ?? t("scoring.completeDefault")}
              </div>
              <button
                type="button"
                onClick={() => setCurrentView("insights")}
                className="flex items-center gap-1.5 rounded-md bg-green-600 px-3 py-2 text-xs font-medium text-white hover:bg-green-700"
              >
                <BarChart3 size={14} />
                {t("scoring.viewInsights")}
              </button>
            </div>
          )}

          {isScoring && scoreProgress && (
            <div className="mt-4">
              <div className="mb-1 flex justify-between text-xs text-gray-500">
                <span>{t("scoring.progress")}</span>
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
  managedAiRuntimeAvailable,
  billingStatus,
  onManage,
}: {
  serviceMode: AiServiceMode;
  providerLabel: string;
  configuredModel: string;
  hasScoringKey: boolean;
  canUseManagedAi: boolean;
  managedAiRuntimeAvailable: boolean;
  billingStatus: BillingStatus | null;
  onManage: () => void;
}) {
  const { t } = useI18n();
  const isManaged = serviceMode === "photocurate_ai";
  const statusText = isManaged
    ? canUseManagedAi
      ? managedAiRuntimeAvailable
        ? t("scoring.statusAvailable")
        : t("scoring.statusEntitled")
      : billingStatus
      ? t(billingStatusTranslationKey(billingStatus.status))
      : t("scoring.statusChecking")
    : hasScoringKey ? t("scoring.statusConfigured") : t("scoring.statusNotConfigured");

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            {isManaged ? <Sparkles size={16} className="text-blue-600" /> : <KeyRound size={16} className="text-gray-700" />}
            <h3 className="text-sm font-semibold text-gray-800">
              {isManaged ? "PhotoCurate AI" : t("scoring.byok")}
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
              ? billingStatus?.canUseManagedAi
                ? managedAiRuntimeAvailable
                  ? t("scoring.managedAvailableDescription", { credits: formatCredits(billingStatus.usage.remainingCredits) })
                  : t("scoring.managedPendingDescription", { credits: formatCredits(billingStatus.usage.remainingCredits) })
                : billingStatus?.message ?? t("scoring.managedNeedsSubscriptionDescription")
              : t("scoring.byokDescription", { provider: providerLabel, model: configuredModel })}
          </p>
        </div>
        <button
          type="button"
          onClick={onManage}
          className="shrink-0 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50"
        >
          {t("common.manage")}
        </button>
      </div>
    </section>
  );
}
