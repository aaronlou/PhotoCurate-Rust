export type BillingPlanId = "free" | "plus" | "pro";
export type BillingInterval = "monthly" | "annual";

export interface PlanConfig {
  includedCredits: number;
  stripePriceEnv?: Partial<Record<BillingInterval, string>>;
}

export const PLAN_CONFIG: Record<BillingPlanId, PlanConfig> = {
  free: {
    includedCredits: 20,
  },
  plus: {
    includedCredits: 1000,
    stripePriceEnv: {
      monthly: "STRIPE_PRICE_PLUS_MONTHLY",
      annual: "STRIPE_PRICE_PLUS_ANNUAL",
    },
  },
  pro: {
    includedCredits: 5000,
    stripePriceEnv: {
      monthly: "STRIPE_PRICE_PRO_MONTHLY",
      annual: "STRIPE_PRICE_PRO_ANNUAL",
    },
  },
};

export function priceEnvName(planId: BillingPlanId, interval: BillingInterval) {
  return PLAN_CONFIG[planId].stripePriceEnv?.[interval] ?? null;
}

export function includedCreditsForPlan(planId: BillingPlanId) {
  return PLAN_CONFIG[planId].includedCredits;
}
