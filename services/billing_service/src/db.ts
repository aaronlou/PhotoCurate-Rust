import Database from "better-sqlite3";
import type { BillingPlanId } from "./plans.js";
import { includedCreditsForPlan } from "./plans.js";
import type { BillingStatus, SubscriptionStatus } from "./types.js";

export interface AccountRow {
  account_id: string;
  stripe_customer_id: string | null;
  provider: string;
  plan_id: BillingPlanId;
  status: SubscriptionStatus;
  stripe_subscription_id: string | null;
  current_period_start: string | null;
  current_period_end: string | null;
  trial_ends_at: string | null;
  used_credits: number;
  created_at: string;
  updated_at: string;
}

export interface EntitlementInput {
  accountId: string;
  stripeCustomerId?: string | null;
  stripeSubscriptionId?: string | null;
  planId: BillingPlanId;
  status: SubscriptionStatus;
  currentPeriodStart?: string | null;
  currentPeriodEnd?: string | null;
  trialEndsAt?: string | null;
}

export function openDatabase(path: string) {
  const db = new Database(path);
  db.pragma("journal_mode = WAL");
  db.exec(`
    CREATE TABLE IF NOT EXISTS billing_accounts (
      account_id TEXT PRIMARY KEY,
      stripe_customer_id TEXT UNIQUE,
      provider TEXT NOT NULL DEFAULT 'stripe',
      plan_id TEXT NOT NULL DEFAULT 'free',
      status TEXT NOT NULL DEFAULT 'inactive',
      stripe_subscription_id TEXT UNIQUE,
      current_period_start TEXT,
      current_period_end TEXT,
      trial_ends_at TEXT,
      used_credits INTEGER NOT NULL DEFAULT 0,
      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE INDEX IF NOT EXISTS idx_billing_accounts_customer
      ON billing_accounts(stripe_customer_id);

    CREATE INDEX IF NOT EXISTS idx_billing_accounts_subscription
      ON billing_accounts(stripe_subscription_id);
  `);
  return db;
}

export function findAccount(db: Database.Database, accountId: string) {
  return db
    .prepare("SELECT * FROM billing_accounts WHERE account_id = ?")
    .get(accountId) as AccountRow | undefined;
}

export function findAccountByStripeCustomer(db: Database.Database, customerId: string) {
  return db
    .prepare("SELECT * FROM billing_accounts WHERE stripe_customer_id = ?")
    .get(customerId) as AccountRow | undefined;
}

export function upsertAccount(
  db: Database.Database,
  accountId: string,
  stripeCustomerId?: string | null
) {
  db.prepare(`
    INSERT INTO billing_accounts (account_id, stripe_customer_id)
    VALUES (@accountId, @stripeCustomerId)
    ON CONFLICT(account_id) DO UPDATE SET
      stripe_customer_id = COALESCE(excluded.stripe_customer_id, billing_accounts.stripe_customer_id),
      updated_at = CURRENT_TIMESTAMP
  `).run({ accountId, stripeCustomerId: stripeCustomerId ?? null });

  return findAccount(db, accountId);
}

export function updateEntitlement(db: Database.Database, input: EntitlementInput) {
  db.prepare(`
    INSERT INTO billing_accounts (
      account_id,
      stripe_customer_id,
      provider,
      plan_id,
      status,
      stripe_subscription_id,
      current_period_start,
      current_period_end,
      trial_ends_at,
      used_credits
    )
    VALUES (
      @accountId,
      @stripeCustomerId,
      'stripe',
      @planId,
      @status,
      @stripeSubscriptionId,
      @currentPeriodStart,
      @currentPeriodEnd,
      @trialEndsAt,
      0
    )
    ON CONFLICT(account_id) DO UPDATE SET
      stripe_customer_id = COALESCE(excluded.stripe_customer_id, billing_accounts.stripe_customer_id),
      provider = 'stripe',
      plan_id = excluded.plan_id,
      status = excluded.status,
      stripe_subscription_id = COALESCE(excluded.stripe_subscription_id, billing_accounts.stripe_subscription_id),
      current_period_start = excluded.current_period_start,
      current_period_end = excluded.current_period_end,
      trial_ends_at = excluded.trial_ends_at,
      used_credits = CASE
        WHEN billing_accounts.current_period_start IS NOT excluded.current_period_start THEN 0
        ELSE billing_accounts.used_credits
      END,
      updated_at = CURRENT_TIMESTAMP
  `).run({
    accountId: input.accountId,
    stripeCustomerId: input.stripeCustomerId ?? null,
    stripeSubscriptionId: input.stripeSubscriptionId ?? null,
    planId: input.planId,
    status: input.status,
    currentPeriodStart: input.currentPeriodStart ?? null,
    currentPeriodEnd: input.currentPeriodEnd ?? null,
    trialEndsAt: input.trialEndsAt ?? null,
  });

  return findAccount(db, input.accountId);
}

export function toBillingStatus(row: AccountRow | undefined, checkoutConfigured: boolean): BillingStatus {
  const planId = row?.plan_id ?? "free";
  const includedCredits = includedCreditsForPlan(planId);
  const usedCredits = row?.used_credits ?? 0;
  const active = row?.status === "active" || row?.status === "trialing";
  const remainingCredits = Math.max(0, includedCredits - usedCredits);

  return {
    provider: "stripe",
    planId,
    status: row?.status ?? "inactive",
    accountId: row?.account_id ?? "",
    renewsAt: row?.current_period_end ?? null,
    trialEndsAt: row?.trial_ends_at ?? null,
    usage: {
      periodStart: row?.current_period_start ?? null,
      periodEnd: row?.current_period_end ?? null,
      includedCredits,
      usedCredits,
      remainingCredits,
    },
    canUseManagedAi: active && remainingCredits > 0,
    checkoutAvailable: checkoutConfigured,
    restoreAvailable: false,
    billingPortalAvailable: Boolean(row?.stripe_customer_id),
    message: active
      ? null
      : checkoutConfigured
      ? "尚未检测到有效订阅。完成支付后权益会由服务端 webhook 自动更新。"
      : "Stripe Price ID 尚未配置，暂时无法创建支付页面。",
  };
}

export function defaultBillingStatus(accountId: string, checkoutIsConfigured: boolean): BillingStatus {
  return {
    ...toBillingStatus(undefined, checkoutIsConfigured),
    accountId,
  };
}
