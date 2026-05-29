import type { ScoringProvider } from "@/types";

export type AiServiceMode = "photocurate_ai" | "byok";

export const SERVICE_MODE_STORAGE_KEY = "photocurate.aiServiceMode";

export const PROVIDERS: Array<{
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

export function providerConfig(provider: ScoringProvider) {
  return PROVIDERS.find((p) => p.id === provider) ?? PROVIDERS[0];
}

export function readStoredAiServiceMode(): AiServiceMode | null {
  if (typeof window === "undefined") return null;

  const value = window.localStorage.getItem(SERVICE_MODE_STORAGE_KEY);
  return value === "photocurate_ai" || value === "byok" ? value : null;
}

export function persistAiServiceMode(mode: AiServiceMode) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(SERVICE_MODE_STORAGE_KEY, mode);
}

export function resolveInitialAiServiceMode(hasScoringKey: boolean): AiServiceMode {
  return readStoredAiServiceMode() ?? (hasScoringKey ? "byok" : "photocurate_ai");
}
