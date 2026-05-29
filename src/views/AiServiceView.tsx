import { useEffect, useState } from "react";
import { useAppStore } from "@/stores/useAppStore";
import {
  checkLocalModel,
  getAiSettings,
  updateAiSettings,
  validateApiKey,
} from "@/hooks/useInvoke";
import {
  AlertCircle,
  BarChart3,
  Check,
  CreditCard,
  Cpu,
  Images,
  KeyRound,
  Server,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Target,
  TrendingUp,
  Zap,
} from "lucide-react";
import type { AISettings, ScoringProvider } from "@/types";
import {
  AiServiceMode,
  persistAiServiceMode,
  providerConfig,
  PROVIDERS,
  readStoredAiServiceMode,
  resolveInitialAiServiceMode,
} from "@/lib/aiService";

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

  const evaluatedCount = photos.filter((p) => p.latest_evaluation).length;
  const activeProvider = providerConfig(scoringProvider);
  const configuredModel = aiSettings?.scoring_model || activeProvider.defaultModel;
  const hasScoringKey = aiSettings?.scoring_provider === scoringProvider && Boolean(aiSettings?.has_scoring_api_key);
  const providerNeedsBaseUrl = scoringProvider !== "gemini";

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
                  badge="即将开放"
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
              <ManagedAiPanel evaluatedCount={evaluatedCount} />
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

function ManagedAiPanel({ evaluatedCount }: { evaluatedCount: number }) {
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

  return (
    <section className="rounded-lg border border-blue-100 bg-white p-5">
      <div className="grid grid-cols-[1fr_0.8fr] gap-5">
        <div>
          <div className="flex items-center gap-2">
            <Zap size={16} className="text-blue-600" />
            <h3 className="text-sm font-semibold text-gray-800">PhotoCurate AI 托管服务</h3>
          </div>
          <p className="mt-2 text-sm leading-6 text-gray-500">
            差异化不在于替用户调用一次 LLM，而在于把大量作品的评分和点评沉淀成持续画像、报告和训练反馈。
          </p>
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
            <PlanPreview title="Free Trial" description="少量作品分析额度" detail="适合第一次导入照片" />
            <PlanPreview title="Plus" description="月度额度和成长报告" detail="适合持续拍摄用户" />
            <PlanPreview title="Pro" description="高级洞察与作品集建议" detail="AI 分析额度可另购" />
          </div>
        </div>

        <div className="rounded-lg border border-gray-200 bg-gray-50 p-4">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-gray-500">分析额度</span>
            <span className="rounded-full bg-blue-100 px-2 py-0.5 text-[11px] font-medium text-blue-700">
              Coming Soon
            </span>
          </div>
          <div className="mt-4">
            <div className="flex items-end gap-2">
              <span className="text-3xl font-semibold tracking-tight text-gray-900">0</span>
              <span className="pb-1 text-xs text-gray-400">/ 1000 张作品分析</span>
            </div>
            <div className="mt-3 h-2 overflow-hidden rounded-full bg-gray-200">
              <div className="h-full w-0 rounded-full bg-blue-600" />
            </div>
          </div>
          <p className="mt-3 text-xs leading-5 text-gray-500">
            当前已积累 {evaluatedCount} 张结构化评价。未来托管服务会基于这些历史数据生成更完整的周期报告。
          </p>
          <button
            type="button"
            disabled
            className="mt-4 w-full rounded-md bg-gray-200 px-3 py-2 text-sm font-medium text-gray-500"
          >
            加入等待名单
          </button>
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
}: {
  serviceMode: AiServiceMode;
  providerLabel: string;
  configuredModel: string;
  hasScoringKey: boolean;
  evaluatedCount: number;
  localModelAvailable: boolean | null;
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
          value={serviceMode === "photocurate_ai" ? "无需配置" : hasScoringKey ? "已保存" : "未配置"}
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

function PlanPreview({ title, description, detail }: { title: string; description: string; detail: string }) {
  return (
    <div className="rounded-md border border-gray-200 bg-gray-50 p-3">
      <p className="text-xs font-semibold text-gray-800">{title}</p>
      <p className="mt-1 text-xs text-gray-500">{description}</p>
      <p className="mt-2 text-[11px] text-gray-400">{detail}</p>
    </div>
  );
}
