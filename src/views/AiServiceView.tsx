import { useEffect, useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import {
  checkLocalModel,
  getAiSettings,
  getBillingStatus,
  openBillingPortal,
  restoreManagedAiPurchases,
  startManagedAiCheckout,
  updateAiSettings,
  validateApiKey,
} from "@/hooks/useInvoke";
import {
  AlertCircle,
  BarChart3,
  CalendarDays,
  Check,
  CreditCard,
  Cpu,
  ExternalLink,
  Images,
  KeyRound,
  ReceiptText,
  RefreshCw,
  Server,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Target,
  TrendingUp,
  Zap,
} from "lucide-react";
import type {
  AISettings,
  BillingActionResult,
  BillingInterval,
  BillingPlanId,
  BillingStatus,
  ScoringProvider,
} from "@/types";
import {
  AiServiceMode,
  persistAiServiceMode,
  providerConfig,
  PROVIDERS,
  readStoredAiServiceMode,
  resolveInitialAiServiceMode,
} from "@/lib/aiService";
import {
  BILLING_PLANS,
  billingProviderTranslationKey,
  billingProviderLabel,
  billingStatusTranslationKey,
  formatCredits,
  getBillingPlan,
  getPlanPrice,
  usagePercent,
} from "@/lib/billing";
import { useI18n, type SupportedLocale, type TranslationKey } from "@/lib/i18n";

function localizedBillingStatus(
  status: BillingStatus | null | undefined,
  t: (key: TranslationKey) => string,
  fallback: string,
) {
  return status ? t(billingStatusTranslationKey(status.status)) : fallback;
}

function localizedBillingProvider(
  status: BillingStatus | null,
  t: (key: TranslationKey) => string,
) {
  const key = billingProviderTranslationKey(status);
  return key ? t(key) : billingProviderLabel(status);
}

function localizedPlanText(
  planId: BillingPlanId,
  field: "audience" | "price" | "monthly" | "annual" | "feature1" | "feature2" | "cta",
  fallback: string,
  t: (key: TranslationKey) => string,
) {
  const key = `billing.plan.${planId}.${field}` as TranslationKey;
  const value = t(key);
  return value === key ? fallback : value;
}

function localizedProviderLabel(provider: ReturnType<typeof providerConfig>, t: (key: TranslationKey) => string) {
  if (provider.id === "openai_compatible_vision") {
    return t("provider.openai_compatible_vision.label");
  }
  return provider.label;
}

function localizedProviderDescription(provider: ReturnType<typeof providerConfig>, t: (key: TranslationKey) => string) {
  if (provider.id === "gemini") {
    return t("provider.gemini.description");
  }
  if (provider.id === "qwen_vl") {
    return t("provider.qwen_vl.description");
  }
  return t("provider.openai_compatible_vision.description");
}

export default function AiServiceView() {
  const { t } = useI18n();
  const aiSettings = useAppStore((s) => s.aiSettings);
  const setAiSettings = useAppStore((s) => s.setAiSettings);
  const photos = useAppStore((s) => s.photos);

  const [serviceMode, setServiceMode] = useState<AiServiceMode>(() => readStoredAiServiceMode() ?? "photocurate_ai");
  const [scoringProvider, setScoringProvider] = useState<ScoringProvider>("gemini");
  const [scoringModel, setScoringModel] = useState(providerConfig("gemini").defaultModel);
  const [scoringBaseUrl, setScoringBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [keyStatus, setKeyStatus] = useState<{ valid: boolean; message: string } | null>(null);
  const [localModelAvailable, setLocalModelAvailable] = useState<boolean | null>(null);
  const [billingStatus, setBillingStatus] = useState<BillingStatus | null>(null);
  const [billingInterval, setBillingInterval] = useState<BillingInterval>("monthly");
  const [billingMessage, setBillingMessage] = useState<string | null>(null);
  const [billingBusyAction, setBillingBusyAction] = useState<string | null>(null);

  const evaluatedCount = photos.filter((p) => p.latest_evaluation).length;
  const activeProvider = providerConfig(scoringProvider);
  const configuredModel = aiSettings?.scoring_model || activeProvider.defaultModel;
  const hasScoringKey = aiSettings?.scoring_provider === scoringProvider && Boolean(aiSettings?.has_scoring_api_key);
  const providerNeedsBaseUrl = scoringProvider !== "gemini";
  const managedAiBadge = billingStatus?.canUseManagedAi
    ? t("scoring.statusAvailable")
    : billingStatus
    ? t(billingStatusTranslationKey(billingStatus.status))
    : t("scoring.statusChecking");

  useEffect(() => {
    checkLocalModel().then(setLocalModelAvailable).catch(() => setLocalModelAvailable(false));
    getAiSettings()
      .then((settings) => {
        setAiSettings(settings);
        if (settings) {
          syncSettingsForm(settings);
          const resolvedMode = resolveInitialAiServiceMode(settings.has_scoring_api_key);
          setServiceMode(resolvedMode);
          persistAiServiceMode(resolvedMode);
        }
      })
      .catch(console.error);
    getBillingStatus()
      .then(setBillingStatus)
      .catch((error) => {
        console.error(error);
        setBillingMessage(t("aiService.billingFetchFailed"));
      });
  }, [setAiSettings, t]);

  const syncSettingsForm = (settings: AISettings) => {
    const provider = settings.scoring_provider ?? "gemini";
    const config = providerConfig(provider);
    setScoringProvider(provider);
    setScoringModel(settings.scoring_model || config.defaultModel);
    setScoringBaseUrl(settings.scoring_base_url || config.defaultBaseUrl);
  };

  const selectServiceMode = (mode: AiServiceMode) => {
    setServiceMode(mode);
    persistAiServiceMode(mode);
    setKeyStatus(null);
  };

  const handleProviderChange = (provider: ScoringProvider) => {
    const config = providerConfig(provider);
    setScoringProvider(provider);
    setScoringModel(config.defaultModel);
    setScoringBaseUrl(config.defaultBaseUrl);
    setApiKey("");
    setKeyStatus(null);
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
      setServiceMode("byok");
      persistAiServiceMode("byok");
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
    setKeyStatus({ valid: true, message: t("aiService.modelSettingsSaved") });
  };

  const runBillingAction = async (
    actionKey: string,
    action: () => Promise<BillingActionResult>
  ) => {
    setBillingBusyAction(actionKey);
    setBillingMessage(null);
    try {
      const result = await action();
      setBillingStatus(result.status);
      setBillingMessage(result.message);
    } catch (error) {
      setBillingMessage(typeof error === "string" ? error : String(error));
    } finally {
      setBillingBusyAction(null);
    }
  };

  return (
    <div className="flex h-full flex-col bg-[#f7f8fa]">
      <header className="border-b border-gray-200 bg-white px-6 py-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="flex items-center gap-2 text-lg font-semibold text-gray-800">
              <Sparkles size={20} className="text-blue-600" />
              {t("aiService.title")}
            </h2>
            <p className="mt-1 text-sm text-gray-500">
              {t("aiService.subtitle")}
            </p>
          </div>
          <div className="rounded-md border border-gray-200 bg-gray-50 px-3 py-2 text-right">
            <p className="text-[11px] font-medium text-gray-400">{t("aiService.currentMode")}</p>
            <p className="mt-0.5 text-sm font-semibold text-gray-800">
              {serviceMode === "photocurate_ai" ? "PhotoCurate AI" : `${localizedProviderLabel(activeProvider, t)} BYOK`}
            </p>
          </div>
        </div>
      </header>

      <main className="flex-1 overflow-auto p-6">
        <section className="grid grid-cols-[1.15fr_0.85fr] gap-5">
          <div className="space-y-5">
            <section className="rounded-lg border border-gray-200 bg-white p-5">
              <div className="mb-4 flex items-center justify-between gap-4">
                <div>
                  <h3 className="text-sm font-semibold text-gray-800">{t("aiService.chooseModeTitle")}</h3>
                  <p className="mt-1 text-xs text-gray-500">
                    {t("aiService.chooseModeSubtitle")}
                  </p>
                </div>
                <span className="rounded-full bg-blue-50 px-2 py-1 text-xs font-medium text-blue-700">
                  {t("aiService.recommendedManaged")}
                </span>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <ModeCard
                  active={serviceMode === "photocurate_ai"}
                  icon={<Sparkles size={17} />}
                  title="PhotoCurate AI"
                  badge={managedAiBadge}
                  description={t("aiService.managedDescription")}
                  onClick={() => selectServiceMode("photocurate_ai")}
                />
                <ModeCard
                  active={serviceMode === "byok"}
                  icon={<SlidersHorizontal size={17} />}
                  title={t("aiService.byokTitle")}
                  badge={hasScoringKey ? t("aiService.byokBadgeConfigured") : t("aiService.byokBadgeNeedsConfig")}
                  description={t("aiService.byokDescription")}
                  onClick={() => selectServiceMode("byok")}
                />
              </div>
            </section>

            {serviceMode === "photocurate_ai" ? (
              <ManagedAiPanel
                billingStatus={billingStatus}
                billingInterval={billingInterval}
                billingMessage={billingMessage}
                billingBusyAction={billingBusyAction}
                evaluatedCount={evaluatedCount}
                onIntervalChange={setBillingInterval}
                onStartCheckout={(planId) =>
                  runBillingAction(`checkout:${planId}`, () =>
                    startManagedAiCheckout(planId, billingInterval)
                  )
                }
                onRestore={() => runBillingAction("restore", restoreManagedAiPurchases)}
                onOpenPortal={() => runBillingAction("portal", openBillingPortal)}
              />
            ) : (
              <ByokSettingsPanel
                activeProvider={activeProvider}
                configuredModel={configuredModel}
                hasScoringKey={hasScoringKey}
                providerNeedsBaseUrl={providerNeedsBaseUrl}
                scoringProvider={scoringProvider}
                scoringModel={scoringModel}
                scoringBaseUrl={scoringBaseUrl}
                apiKey={apiKey}
                keyStatus={keyStatus}
                onProviderChange={handleProviderChange}
                onModelChange={(value) => {
                  setScoringModel(value);
                  setKeyStatus(null);
                }}
                onBaseUrlChange={(value) => {
                  setScoringBaseUrl(value);
                  setKeyStatus(null);
                }}
                onApiKeyChange={setApiKey}
                onValidateKey={handleValidateKey}
                onSaveModelSettings={handleSaveModelSettings}
              />
            )}
          </div>

          <aside className="space-y-5">
            <ServiceStatusPanel
              serviceMode={serviceMode}
              providerLabel={localizedProviderLabel(activeProvider, t)}
              configuredModel={configuredModel}
              hasScoringKey={hasScoringKey}
              evaluatedCount={evaluatedCount}
              localModelAvailable={localModelAvailable}
              billingStatus={billingStatus}
            />
            <DifferentiationPanel />
          </aside>
        </section>
      </main>
    </div>
  );
}

function ModeCard({
  active,
  icon,
  title,
  badge,
  description,
  onClick,
}: {
  active: boolean;
  icon: React.ReactNode;
  title: string;
  badge: string;
  description: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-lg border p-4 text-left transition-colors ${
        active ? "border-blue-500 bg-blue-50" : "border-gray-200 bg-white hover:border-gray-300"
      }`}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-2">
          <span className={active ? "text-blue-600" : "text-gray-600"}>{icon}</span>
          <span className="text-sm font-semibold text-gray-800">{title}</span>
        </div>
        <span className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${
          active ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-500"
        }`}>
          {badge}
        </span>
      </div>
      <p className="mt-2 text-xs leading-5 text-gray-500">{description}</p>
    </button>
  );
}

function ManagedAiPanel({
  billingStatus,
  billingInterval,
  billingMessage,
  billingBusyAction,
  evaluatedCount,
  onIntervalChange,
  onStartCheckout,
  onRestore,
  onOpenPortal,
}: {
  billingStatus: BillingStatus | null;
  billingInterval: BillingInterval;
  billingMessage: string | null;
  billingBusyAction: string | null;
  evaluatedCount: number;
  onIntervalChange: (interval: BillingInterval) => void;
  onStartCheckout: (planId: BillingPlanId) => void;
  onRestore: () => void;
  onOpenPortal: () => void;
}) {
  const { t, locale } = useI18n();
  const valueItems = [
    {
      icon: <Images size={14} />,
      title: t("aiService.crossLibraryTitle"),
      description: t("aiService.crossLibraryDescription"),
    },
    {
      icon: <TrendingUp size={14} />,
      title: t("aiService.growthTitle"),
      description: t("aiService.growthDescription"),
    },
    {
      icon: <Target size={14} />,
      title: t("aiService.practiceTitle"),
      description: t("aiService.practiceDescription"),
    },
  ];
  const currentPlan = getBillingPlan((billingStatus?.planId as BillingPlanId | undefined) ?? "free");
  const percent = usagePercent(billingStatus);
  const remainingCredits = billingStatus?.usage.remainingCredits ?? currentPlan.includedCredits;
  const includedCredits = billingStatus?.usage.includedCredits ?? currentPlan.includedCredits;
  const usedCredits = billingStatus?.usage.usedCredits ?? 0;
  const canManageBilling = Boolean(billingStatus?.billingPortalAvailable);
  const canRestorePurchases = Boolean(billingStatus?.restoreAvailable);

  return (
    <section className="rounded-lg border border-blue-100 bg-white p-5">
      <div className="grid grid-cols-[1fr_0.82fr] gap-5">
        <div>
          <div className="flex items-start justify-between gap-4">
            <div>
              <div className="flex items-center gap-2">
                <Zap size={16} className="text-blue-600" />
                <h3 className="text-sm font-semibold text-gray-800">{t("aiService.subscriptionTitle")}</h3>
              </div>
              <p className="mt-2 text-sm leading-6 text-gray-500">
                {t("aiService.subscriptionDescription")}
              </p>
            </div>
            <div className="flex rounded-md border border-gray-200 bg-gray-50 p-0.5">
              {(["monthly", "annual"] as BillingInterval[]).map((interval) => (
                <button
                  key={interval}
                  type="button"
                  onClick={() => onIntervalChange(interval)}
                  className={`rounded px-3 py-1.5 text-xs font-medium ${
                    billingInterval === interval
                      ? "bg-white text-blue-600 shadow-sm"
                      : "text-gray-500 hover:text-gray-700"
                  }`}
                >
                  {interval === "monthly" ? t("aiService.monthly") : t("aiService.annual")}
                </button>
              ))}
            </div>
          </div>

          <div className="mt-4 grid grid-cols-3 gap-2">
            {valueItems.map((item) => (
              <div key={item.title} className="rounded-md border border-gray-200 bg-gray-50 p-3">
                <div className="flex items-center gap-1.5 text-xs font-semibold text-gray-800">
                  <span className="text-blue-600">{item.icon}</span>
                  {item.title}
                </div>
                <p className="mt-2 text-[11px] leading-5 text-gray-500">{item.description}</p>
              </div>
            ))}
          </div>

          <div className="mt-4 grid grid-cols-3 gap-2">
            {BILLING_PLANS.map((plan) => (
              <PlanCard
                key={plan.id}
                plan={plan}
                interval={billingInterval}
                isCurrent={billingStatus?.planId === plan.id}
                busy={billingBusyAction === `checkout:${plan.id}`}
                checkoutAvailable={Boolean(billingStatus?.checkoutAvailable)}
                onStartCheckout={() => onStartCheckout(plan.id)}
              />
            ))}
          </div>
        </div>

        <div className="rounded-lg border border-gray-200 bg-gray-50 p-4">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-gray-500">{t("aiService.creditUsage")}</span>
            <span className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${
              billingStatus?.canUseManagedAi ? "bg-green-50 text-green-700" : "bg-amber-50 text-amber-700"
            }`}>
              {localizedBillingStatus(billingStatus, t, t("scoring.statusChecking"))}
            </span>
          </div>
          <div className="mt-4">
            <div className="flex items-end gap-2">
              <span className="text-3xl font-semibold tracking-tight text-gray-900">
                {formatCredits(remainingCredits)}
              </span>
              <span className="pb-1 text-xs text-gray-400">
                {t("aiService.availableCredits", { credits: formatCredits(includedCredits) })}
              </span>
            </div>
            <div className="mt-3 h-2 overflow-hidden rounded-full bg-gray-200">
              <div
                className="h-full rounded-full bg-blue-600 transition-all"
                style={{ width: `${percent}%` }}
              />
            </div>
          </div>
          <p className="mt-3 text-xs leading-5 text-gray-500">
            {t("aiService.usageDescription", {
              used: formatCredits(usedCredits),
              evaluated: evaluatedCount,
            })}
          </p>
          <div className="mt-4 space-y-2 rounded-md border border-gray-200 bg-white p-3">
            <StatusRow
              icon={<CreditCard size={14} />}
              label={t("aiService.currentPlan")}
              value={currentPlan.name}
            />
            <StatusRow
              icon={<ReceiptText size={14} />}
              label={t("aiService.paymentProvider")}
              value={localizedBillingProvider(billingStatus, t)}
            />
            <StatusRow
              icon={<CalendarDays size={14} />}
              label={t("aiService.renewalDate")}
              value={formatBillingDate(billingStatus?.renewsAt, locale, t("aiService.dateUnset"))}
            />
          </div>

          {billingStatus?.message && (
            <div className="mt-3 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs leading-5 text-amber-800">
              {billingStatus.message}
            </div>
          )}

          {billingMessage && (
            <div className="mt-3 rounded-md border border-blue-100 bg-blue-50 px-3 py-2 text-xs leading-5 text-blue-800">
              {billingMessage}
            </div>
          )}

          <div className="mt-4 grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={onRestore}
              disabled={!canRestorePurchases || billingBusyAction === "restore"}
              className="flex items-center justify-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <RefreshCw size={13} className={billingBusyAction === "restore" ? "animate-spin" : ""} />
              {t("aiService.restorePurchases")}
            </button>
            <button
              type="button"
              onClick={onOpenPortal}
              disabled={!canManageBilling || billingBusyAction === "portal"}
              className="flex items-center justify-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <ExternalLink size={13} />
              {t("aiService.manageSubscription")}
            </button>
          </div>

          <div className="mt-3 rounded-md border border-gray-200 bg-white px-3 py-2 text-xs leading-5 text-gray-500">
            {t("aiService.managedPrivacyNote")}
          </div>
        </div>
      </div>
    </section>
  );
}

function ByokSettingsPanel({
  activeProvider,
  configuredModel,
  hasScoringKey,
  providerNeedsBaseUrl,
  scoringProvider,
  scoringModel,
  scoringBaseUrl,
  apiKey,
  keyStatus,
  onProviderChange,
  onModelChange,
  onBaseUrlChange,
  onApiKeyChange,
  onValidateKey,
  onSaveModelSettings,
}: {
  activeProvider: ReturnType<typeof providerConfig>;
  configuredModel: string;
  hasScoringKey: boolean;
  providerNeedsBaseUrl: boolean;
  scoringProvider: ScoringProvider;
  scoringModel: string;
  scoringBaseUrl: string;
  apiKey: string;
  keyStatus: { valid: boolean; message: string } | null;
  onProviderChange: (provider: ScoringProvider) => void;
  onModelChange: (value: string) => void;
  onBaseUrlChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  onValidateKey: () => void;
  onSaveModelSettings: () => void;
}) {
  const { t } = useI18n();
  const activeProviderLabel = localizedProviderLabel(activeProvider, t);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-5">
      <div className="mb-4 flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-semibold text-gray-800">{t("aiService.byokSettingsTitle")}</h3>
          <p className="mt-1 text-xs text-gray-500">{t("aiService.byokSettingsDescription")}</p>
        </div>
        <div className={`flex items-center gap-1.5 text-xs ${hasScoringKey ? "text-green-600" : "text-amber-600"}`}>
          <KeyRound size={13} />
          {hasScoringKey ? t("aiService.keySaved") : t("aiService.keyNotSaved")}
        </div>
      </div>

      <div className="space-y-4">
        <div className="grid grid-cols-3 gap-2">
          {PROVIDERS.map((provider) => (
            <button
              key={provider.id}
              type="button"
              onClick={() => onProviderChange(provider.id)}
              className={`rounded-md border px-3 py-2 text-left transition-colors ${
                scoringProvider === provider.id
                  ? "border-blue-500 bg-blue-50 text-blue-700"
                  : "border-gray-200 bg-white text-gray-700 hover:border-gray-300"
              }`}
            >
              <span className="block text-sm font-medium">{localizedProviderLabel(provider, t)}</span>
              <span className="mt-0.5 block text-xs text-gray-500">{localizedProviderDescription(provider, t)}</span>
            </button>
          ))}
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="mb-1 block text-xs text-gray-500">{t("aiService.model")}</label>
            <input
              type="text"
              value={scoringModel}
              onChange={(e) => onModelChange(e.target.value)}
              placeholder={activeProvider.defaultModel}
              className="w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs text-gray-500">Base URL</label>
            <div className="relative">
              <Server size={14} className="absolute left-3 top-2.5 text-gray-400" />
              <input
                type="text"
                value={scoringBaseUrl}
                onChange={(e) => onBaseUrlChange(e.target.value)}
                placeholder={providerNeedsBaseUrl ? activeProvider.defaultBaseUrl || "https://api.example.com/v1" : t("aiService.geminiNoBaseUrl")}
                disabled={!providerNeedsBaseUrl}
                className="w-full rounded-md border border-gray-300 bg-white py-2 pl-8 pr-3 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100 disabled:text-gray-400"
              />
            </div>
          </div>
        </div>

        <div>
          <label className="mb-1 block text-xs text-gray-500">{activeProviderLabel} API Key</label>
          <div className="flex gap-2">
            <input
              type="password"
              value={apiKey}
              onChange={(e) => onApiKeyChange(e.target.value)}
              placeholder={
                hasScoringKey
                  ? t("aiService.apiKeyPlaceholderSaved")
                  : t("aiService.apiKeyPlaceholderNew", { provider: activeProviderLabel })
              }
              className="flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            <button
              type="button"
              onClick={onValidateKey}
              disabled={!apiKey.trim()}
              className="rounded-md bg-gray-800 px-3 py-2 text-sm text-white hover:bg-gray-900 disabled:opacity-50"
            >
              {t("aiService.validate")}
            </button>
            <button
              type="button"
              onClick={onSaveModelSettings}
              className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 hover:bg-gray-50"
            >
              {t("aiService.save")}
            </button>
          </div>
          {keyStatus && (
            <div className={`mt-2 flex items-center gap-1.5 text-xs ${
              keyStatus.valid ? "text-green-600" : "text-red-500"
            }`}>
              {keyStatus.valid ? <Check size={14} /> : <AlertCircle size={14} />}
              {keyStatus.message}
            </div>
          )}
        </div>

        <div className="text-xs text-gray-500">
          {t("aiService.currentScoring", { provider: activeProviderLabel })}
          <span className="mx-1 text-gray-300">/</span>
          {configuredModel}
        </div>
      </div>
    </section>
  );
}

function ServiceStatusPanel({
  serviceMode,
  providerLabel,
  configuredModel,
  hasScoringKey,
  evaluatedCount,
  localModelAvailable,
  billingStatus,
}: {
  serviceMode: AiServiceMode;
  providerLabel: string;
  configuredModel: string;
  hasScoringKey: boolean;
  evaluatedCount: number;
  localModelAvailable: boolean | null;
  billingStatus: BillingStatus | null;
}) {
  const { t } = useI18n();

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-5">
      <h3 className="text-sm font-semibold text-gray-800">{t("aiService.statusTitle")}</h3>
      <div className="mt-4 space-y-3">
        <StatusRow
          icon={<Sparkles size={14} />}
          label={t("aiService.aiMode")}
          value={serviceMode === "photocurate_ai" ? "PhotoCurate AI" : t("aiService.byokTitle")}
        />
        <StatusRow
          icon={<KeyRound size={14} />}
          label={t("aiService.scoringModel")}
          value={serviceMode === "photocurate_ai" ? t("aiService.managedModel") : `${providerLabel} / ${configuredModel}`}
        />
        <StatusRow
          icon={<ShieldCheck size={14} />}
          label={t("aiService.keyStatus")}
          value={serviceMode === "photocurate_ai" ? t("aiService.noKeychainNeeded") : hasScoringKey ? t("aiService.saved") : t("scoring.statusNotConfigured")}
        />
        <StatusRow
          icon={<CreditCard size={14} />}
          label={t("aiService.subscriptionStatus")}
          value={serviceMode === "photocurate_ai" && billingStatus
            ? t(billingStatusTranslationKey(billingStatus.status))
            : serviceMode === "photocurate_ai"
            ? t("scoring.statusChecking")
            : t("aiService.notNeeded")}
        />
        <StatusRow
          icon={<Cpu size={14} />}
          label={t("aiService.localSearchModel")}
          value={localModelAvailable == null ? t("scoring.statusChecking") : localModelAvailable ? t("aiService.loaded") : t("aiService.notLoaded")}
        />
        <StatusRow
          icon={<BarChart3 size={14} />}
          label={t("aiService.evaluationData")}
          value={t("aiService.photoCount", { count: evaluatedCount })}
        />
      </div>
    </section>
  );
}

function DifferentiationPanel() {
  const { t } = useI18n();

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-5">
      <h3 className="text-sm font-semibold text-gray-800">{t("aiService.diffTitle")}</h3>
      <div className="mt-4 space-y-3 text-xs leading-5 text-gray-500">
        <div className="flex items-start gap-2">
          <BarChart3 size={14} className="mt-0.5 shrink-0 text-blue-600" />
          <span>{t("aiService.diffProfile")}</span>
        </div>
        <div className="flex items-start gap-2">
          <TrendingUp size={14} className="mt-0.5 shrink-0 text-green-600" />
          <span>{t("aiService.diffInsights")}</span>
        </div>
        <div className="flex items-start gap-2">
          <CreditCard size={14} className="mt-0.5 shrink-0 text-amber-600" />
          <span>{t("aiService.diffBilling")}</span>
        </div>
      </div>
    </section>
  );
}

function StatusRow({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-md bg-gray-50 px-3 py-2">
      <div className="flex items-center gap-2 text-xs text-gray-500">
        <span className="text-gray-500">{icon}</span>
        {label}
      </div>
      <span className="min-w-0 truncate text-right text-xs font-medium text-gray-800">{value}</span>
    </div>
  );
}

function PlanCard({
  plan,
  interval,
  isCurrent,
  busy,
  checkoutAvailable,
  onStartCheckout,
}: {
  plan: (typeof BILLING_PLANS)[number];
  interval: BillingInterval;
  isCurrent: boolean;
  busy: boolean;
  checkoutAvailable: boolean;
  onStartCheckout: () => void;
}) {
  const { t } = useI18n();
  const isFree = plan.id === "free";
  const disabled = isFree || isCurrent || !checkoutAvailable || busy;
  const audience = localizedPlanText(plan.id, "audience", plan.audience, t);
  const price = plan.id === "free"
    ? localizedPlanText(plan.id, "price", getPlanPrice(plan, interval), t)
    : interval === "annual"
    ? localizedPlanText(plan.id, "annual", getPlanPrice(plan, interval), t)
    : localizedPlanText(plan.id, "monthly", getPlanPrice(plan, interval), t);
  const features = [
    localizedPlanText(plan.id, "feature1", plan.features[0] ?? "", t),
    localizedPlanText(plan.id, "feature2", plan.features[1] ?? "", t),
  ].filter(Boolean);
  const cta = localizedPlanText(plan.id, "cta", plan.cta, t);

  return (
    <div
      className={`rounded-md border p-3 ${
        plan.recommended ? "border-blue-200 bg-blue-50" : "border-gray-200 bg-gray-50"
      }`}
    >
      <div className="flex min-h-8 items-start justify-between gap-2">
        <div>
          <p className="text-xs font-semibold text-gray-800">{plan.name}</p>
          <p className="mt-0.5 text-[11px] text-gray-500">{audience}</p>
        </div>
        {plan.recommended && (
          <span className="rounded-full bg-blue-600 px-1.5 py-0.5 text-[10px] font-medium text-white">
            {t("aiService.recommended")}
          </span>
        )}
      </div>
      <p className="mt-3 text-sm font-semibold text-gray-900">{price}</p>
      <p className="mt-1 text-[11px] text-gray-500">
        {t("aiService.hostedCredits", { credits: formatCredits(plan.includedCredits) })}
      </p>
      <ul className="mt-3 space-y-1.5">
        {features.map((feature) => (
          <li key={feature} className="flex gap-1.5 text-[11px] leading-4 text-gray-600">
            <Check size={12} className="mt-0.5 shrink-0 text-green-600" />
            <span>{feature}</span>
          </li>
        ))}
      </ul>
      <button
        type="button"
        onClick={onStartCheckout}
        disabled={disabled}
        className={`mt-3 w-full rounded-md px-3 py-2 text-xs font-medium ${
          plan.recommended
            ? "bg-blue-600 text-white hover:bg-blue-700 disabled:bg-blue-200"
            : "border border-gray-300 bg-white text-gray-700 hover:bg-gray-50 disabled:bg-gray-100 disabled:text-gray-400"
        } disabled:cursor-not-allowed`}
      >
        {busy ? t("aiService.processing") : isCurrent ? t("aiService.currentPlanAction") : cta}
      </button>
    </div>
  );
}

function formatBillingDate(value: string | null | undefined, locale: SupportedLocale, fallback: string) {
  if (!value) return fallback;
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date(value));
}
