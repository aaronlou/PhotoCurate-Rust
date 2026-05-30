import type { BillingPlanId } from "./plans.js";

export type BillingProvider = "none" | "app_store" | "stripe" | "manual";
export type SubscriptionStatus =
  | "unconfigured"
  | "inactive"
  | "trialing"
  | "active"
  | "past_due"
  | "canceled";

export interface BillingUsage {
  periodStart: string | null;
  periodEnd: string | null;
  includedCredits: number;
  usedCredits: number;
  remainingCredits: number;
}

export interface BillingStatus {
  provider: BillingProvider;
  planId: BillingPlanId;
  status: SubscriptionStatus;
  accountId: string;
  renewsAt: string | null;
  trialEndsAt: string | null;
  usage: BillingUsage;
  canUseManagedAi: boolean;
  checkoutAvailable: boolean;
  restoreAvailable: boolean;
  billingPortalAvailable: boolean;
  message: string | null;
}
