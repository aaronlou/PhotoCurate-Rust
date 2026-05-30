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
  billingProviderLabel,
  billingStatusLabel,
  formatCredits,
  getBillingPlan,
  getPlanPrice,
  usagePercent,
} from "@/lib/billing";

export default function AiServiceView() {
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
    ? "可用"
    : billingStatus
    ? billingStatusLabel(billingStatus.status)
    : "检测中";

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
        setBillingMessage("无法读取订阅状态，请稍后重试。");
      });
  }, [setAiSettings]);

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
    setKeyStatus({ valid: true, message: "模型设置已保存" });
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
              AI 服务
            </h2>
            <p className="mt-1 text-sm text-gray-500">
              管理 PhotoCurate 的 AI 能力、分析额度、订阅方案和高级 API Key
            </p>
          </div>
          <div className="rounded-md border border-gray-200 bg-gray-50 px-3 py-2 text-right">
            <p className="text-[11px] font-medium text-gray-400">当前模式</p>
            <p className="mt-0.5 text-sm font-semibold text-gray-800">
              {serviceMode === "photocurate_ai" ? "PhotoCurate AI" : `${activeProvider.label} BYOK`}
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
                  <h3 className="text-sm font-semibold text-gray-800">选择 AI 使用方式</h3>
                  <p className="mt-1 text-xs text-gray-500">
                    普通用户使用托管 AI；高级用户可继续用自己的模型账号。
                  </p>
                </div>
                <span className="rounded-full bg-blue-50 px-2 py-1 text-xs font-medium text-blue-700">
                  推荐 PhotoCurate AI
                </span>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <ModeCard
                  active={serviceMode === "photocurate_ai"}
                  icon={<Sparkles size={17} />}
                  title="PhotoCurate AI"
                  badge={managedAiBadge}
                  description="无需申请 API Key。由产品统一提供评分、点评、洞察报告和模型升级。"
                  onClick={() => selectServiceMode("photocurate_ai")}
                />
                <ModeCard
                  active={serviceMode === "byok"}
                  icon={<SlidersHorizontal size={17} />}
                  title="自带 API Key"
                  badge={hasScoringKey ? "已配置" : "需配置"}
                  description="使用 Gemini、Qwen-VL 或自定义 OpenAI-compatible Vision API，费用由服务商收取。"
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
              providerLabel={activeProvider.label}
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
  const valueItems = [
    {
      icon: <Images size={14} />,
      title: "跨作品库分析",
      description: "不只点评单张照片，而是识别你长期作品里的题材偏好、稳定优势和反复短板。",
    },
    {
      icon: <TrendingUp size={14} />,
      title: "成长趋势报告",
      description: "按时间追踪评分、维度和标签变化，帮你看到拍摄习惯是否真的在进步。",
    },
    {
      icon: <Target size={14} />,
      title: "练习路径建议",
      description: "把评价沉淀成可执行训练主题，比如构图、光线、主体表达和后期取舍。",
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
                <h3 className="text-sm font-semibold text-gray-800">PhotoCurate AI 订阅</h3>
              </div>
              <p className="mt-2 text-sm leading-6 text-gray-500">
                普通用户无需申请模型 API Key。订阅后由 PhotoCurate 提供评分、点评、洞察报告和模型升级。
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
                  {interval === "monthly" ? "月付" : "年付"}
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
            <span className="text-xs font-medium text-gray-500">分析额度</span>
            <span className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${
              billingStatus?.canUseManagedAi ? "bg-green-50 text-green-700" : "bg-amber-50 text-amber-700"
            }`}>
              {billingStatus ? billingStatusLabel(billingStatus.status) : "检测中"}
            </span>
          </div>
          <div className="mt-4">
            <div className="flex items-end gap-2">
              <span className="text-3xl font-semibold tracking-tight text-gray-900">
                {formatCredits(remainingCredits)}
              </span>
              <span className="pb-1 text-xs text-gray-400">
                / {formatCredits(includedCredits)} 张可用
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
            本周期已使用 {formatCredits(usedCredits)} 张。当前已积累 {evaluatedCount} 张结构化评价，
            托管服务会基于这些历史数据生成更完整的周期报告。
          </p>
          <div className="mt-4 space-y-2 rounded-md border border-gray-200 bg-white p-3">
            <StatusRow
              icon={<CreditCard size={14} />}
              label="当前方案"
              value={currentPlan.name}
            />
            <StatusRow
              icon={<ReceiptText size={14} />}
              label="支付渠道"
              value={billingProviderLabel(billingStatus)}
            />
            <StatusRow
              icon={<CalendarDays size={14} />}
              label="续订时间"
              value={formatBillingDate(billingStatus?.renewsAt)}
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
              恢复购买
            </button>
            <button
              type="button"
              onClick={onOpenPortal}
              disabled={!canManageBilling || billingBusyAction === "portal"}
              className="flex items-center justify-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <ExternalLink size={13} />
              管理订阅
            </button>
          </div>

          <div className="mt-3 rounded-md border border-gray-200 bg-white px-3 py-2 text-xs leading-5 text-gray-500">
            自带 API Key 模式不需要订阅；托管 AI 会把待分析照片发送到 PhotoCurate 服务端调用模型，
            仅用于生成评分和点评，不会读取你的钥匙串密钥。
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
  return (
    <section className="rounded-lg border border-gray-200 bg-white p-5">
      <div className="mb-4 flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-semibold text-gray-800">高级 API Key 设置</h3>
          <p className="mt-1 text-xs text-gray-500">使用你自己的模型账号，API 费用由对应服务商收取。</p>
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
              onClick={() => onProviderChange(provider.id)}
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
            <label className="mb-1 block text-xs text-gray-500">模型</label>
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
                placeholder={providerNeedsBaseUrl ? activeProvider.defaultBaseUrl || "https://api.example.com/v1" : "Gemini 不需要填写"}
                disabled={!providerNeedsBaseUrl}
                className="w-full rounded-md border border-gray-300 bg-white py-2 pl-8 pr-3 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100 disabled:text-gray-400"
              />
            </div>
          </div>
        </div>

        <div>
          <label className="mb-1 block text-xs text-gray-500">{activeProvider.label} API Key</label>
          <div className="flex gap-2">
            <input
              type="password"
              value={apiKey}
              onChange={(e) => onApiKeyChange(e.target.value)}
              placeholder={hasScoringKey ? "留空则继续使用已保存密钥" : `输入 ${activeProvider.label} API Key`}
              className="flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            <button
              type="button"
              onClick={onValidateKey}
              disabled={!apiKey.trim()}
              className="rounded-md bg-gray-800 px-3 py-2 text-sm text-white hover:bg-gray-900 disabled:opacity-50"
            >
              验证
            </button>
            <button
              type="button"
              onClick={onSaveModelSettings}
              className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 hover:bg-gray-50"
            >
              保存
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
          当前评分: {activeProvider.label}
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
  return (
    <section className="rounded-lg border border-gray-200 bg-white p-5">
      <h3 className="text-sm font-semibold text-gray-800">服务状态</h3>
      <div className="mt-4 space-y-3">
        <StatusRow
          icon={<Sparkles size={14} />}
          label="AI 模式"
          value={serviceMode === "photocurate_ai" ? "PhotoCurate AI" : "自带 API Key"}
        />
        <StatusRow
          icon={<KeyRound size={14} />}
          label="评分模型"
          value={serviceMode === "photocurate_ai" ? "托管模型" : `${providerLabel} / ${configuredModel}`}
        />
        <StatusRow
          icon={<ShieldCheck size={14} />}
          label="密钥状态"
          value={serviceMode === "photocurate_ai" ? "无需钥匙串" : hasScoringKey ? "已保存" : "未配置"}
        />
        <StatusRow
          icon={<CreditCard size={14} />}
          label="订阅状态"
          value={serviceMode === "photocurate_ai" && billingStatus
            ? billingStatusLabel(billingStatus.status)
            : serviceMode === "photocurate_ai"
            ? "检测中"
            : "不需要"}
        />
        <StatusRow
          icon={<Cpu size={14} />}
          label="本地检索模型"
          value={localModelAvailable == null ? "检测中" : localModelAvailable ? "已加载" : "未加载"}
        />
        <StatusRow
          icon={<BarChart3 size={14} />}
          label="评价数据"
          value={`${evaluatedCount} 张`}
        />
      </div>
    </section>
  );
}

function DifferentiationPanel() {
  return (
    <section className="rounded-lg border border-gray-200 bg-white p-5">
      <h3 className="text-sm font-semibold text-gray-800">为什么不是普通 LLM</h3>
      <div className="mt-4 space-y-3 text-xs leading-5 text-gray-500">
        <div className="flex items-start gap-2">
          <BarChart3 size={14} className="mt-0.5 shrink-0 text-blue-600" />
          <span>PhotoCurate 会把单张评分、维度、点评、标签汇总成作品库级别的长期画像。</span>
        </div>
        <div className="flex items-start gap-2">
          <TrendingUp size={14} className="mt-0.5 shrink-0 text-green-600" />
          <span>洞察报告会随着你的拍摄积累更新，关注趋势和反复出现的问题。</span>
        </div>
        <div className="flex items-start gap-2">
          <CreditCard size={14} className="mt-0.5 shrink-0 text-amber-600" />
          <span>未来付费会围绕分析额度、报告深度、作品集建议和高级训练计划展开。</span>
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
  const isFree = plan.id === "free";
  const disabled = isFree || isCurrent || !checkoutAvailable || busy;

  return (
    <div
      className={`rounded-md border p-3 ${
        plan.recommended ? "border-blue-200 bg-blue-50" : "border-gray-200 bg-gray-50"
      }`}
    >
      <div className="flex min-h-8 items-start justify-between gap-2">
        <div>
          <p className="text-xs font-semibold text-gray-800">{plan.name}</p>
          <p className="mt-0.5 text-[11px] text-gray-500">{plan.audience}</p>
        </div>
        {plan.recommended && (
          <span className="rounded-full bg-blue-600 px-1.5 py-0.5 text-[10px] font-medium text-white">
            推荐
          </span>
        )}
      </div>
      <p className="mt-3 text-sm font-semibold text-gray-900">{getPlanPrice(plan, interval)}</p>
      <p className="mt-1 text-[11px] text-gray-500">
        {formatCredits(plan.includedCredits)} 张托管 AI 额度
      </p>
      <ul className="mt-3 space-y-1.5">
        {plan.features.slice(0, 2).map((feature) => (
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
        {busy ? "处理中..." : isCurrent ? "当前方案" : plan.cta}
      </button>
    </div>
  );
}

function formatBillingDate(value?: string | null) {
  if (!value) return "未设置";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date(value));
}
