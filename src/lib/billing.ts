import type { BillingInterval, BillingPlanId, BillingStatus, SubscriptionStatus } from "@/types";
import type { TranslationKey } from "@/lib/i18n";

export interface BillingPlan {
  id: BillingPlanId;
  name: string;
  audience: string;
  monthlyPrice: string;
  annualPrice: string;
  includedCredits: number;
  recommended?: boolean;
  features: string[];
  limits: string[];
  cta: string;
}

export const BILLING_PLANS: BillingPlan[] = [
  {
    id: "free",
    name: "Free Trial",
    audience: "第一次整理作品库",
    monthlyPrice: "免费",
    annualPrice: "免费",
    includedCredits: 20,
    features: ["20 张 PhotoCurate AI 体验额度", "可继续使用自带 API Key", "基础图库、评分展示和导出"],
    limits: ["托管 AI 额度用完后需升级", "不包含周期洞察报告"],
    cta: "当前可用",
  },
  {
    id: "plus",
    name: "Plus",
    audience: "持续拍摄用户",
    monthlyPrice: "$6.99/月",
    annualPrice: "$69/年",
    includedCredits: 1000,
    recommended: true,
    features: ["每月 1000 张托管 AI 分析", "作品库洞察和成长趋势", "自动模型升级和稳定队列"],
    limits: ["适合个人作品库", "不含团队协作"],
    cta: "升级 Plus",
  },
  {
    id: "pro",
    name: "Pro",
    audience: "重度摄影和作品集用户",
    monthlyPrice: "$14.99/月",
    annualPrice: "$149/年",
    includedCredits: 5000,
    features: ["每月 5000 张托管 AI 分析", "高级作品集建议", "更高优先级队列和深度报告"],
    limits: ["超大规模图库可后续接入额度包", "团队功能单独规划"],
    cta: "升级 Pro",
  },
];

export function getBillingPlan(planId: BillingPlanId) {
  return BILLING_PLANS.find((plan) => plan.id === planId) ?? BILLING_PLANS[0];
}

export function getPlanPrice(plan: BillingPlan, interval: BillingInterval) {
  return interval === "annual" ? plan.annualPrice : plan.monthlyPrice;
}

export function formatCredits(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

export function billingStatusLabel(status: SubscriptionStatus) {
  switch (status) {
    case "active":
      return "订阅有效";
    case "trialing":
      return "试用中";
    case "past_due":
      return "付款待处理";
    case "canceled":
      return "已取消";
    case "inactive":
      return "未订阅";
    case "unconfigured":
      return "待配置";
  }
}

export function billingProviderLabel(status: BillingStatus | null) {
  if (!status) return "检测中";
  if (status.provider === "app_store") return "App Store";
  if (status.provider === "stripe") return "Stripe";
  if (status.provider === "manual") return "手动开通";
  return "未配置";
}

export function usagePercent(status: BillingStatus | null) {
  if (!status || status.usage.includedCredits <= 0) {
    return 0;
  }
  return Math.min(100, Math.round((status.usage.usedCredits / status.usage.includedCredits) * 100));
}

export function billingStatusTranslationKey(status: SubscriptionStatus): TranslationKey {
  return `billing.status.${status}` as TranslationKey;
}

export function billingProviderTranslationKey(status: BillingStatus | null): TranslationKey | null {
  if (!status) return "billing.provider.checking";
  if (status.provider === "manual") return "billing.provider.manual";
  if (status.provider === "none") return "billing.provider.unconfigured";
  return null;
}
