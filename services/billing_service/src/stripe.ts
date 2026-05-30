import Stripe from "stripe";
import { z } from "zod";
import type Database from "better-sqlite3";
import {
  findAccount,
  findAccountByStripeCustomer,
  updateEntitlement,
  upsertAccount,
} from "./db.js";
import type { BillingInterval, BillingPlanId } from "./plans.js";
import { includedCreditsForPlan, priceEnvName } from "./plans.js";
import type { SubscriptionStatus } from "./types.js";

const stripeApiVersion = "2025-02-24.acacia";

export const checkoutSchema = z.object({
  accountId: z.string().uuid(),
  planId: z.enum(["plus", "pro"]),
  interval: z.enum(["monthly", "annual"]),
});

export const portalSchema = z.object({
  accountId: z.string().uuid(),
});

export function createStripeClient(secretKey: string) {
  return new Stripe(secretKey, {
    apiVersion: stripeApiVersion,
    appInfo: {
      name: "PhotoCurate Billing Service",
      version: "0.1.0",
    },
  });
}

export function isStripeConfigured() {
  return Boolean(process.env.STRIPE_SECRET_KEY);
}

export function priceIdFor(planId: BillingPlanId, interval: BillingInterval) {
  const envName = priceEnvName(planId, interval);
  if (!envName) return null;
  const priceId = process.env[envName]?.trim();
  return priceId || null;
}

export function checkoutConfigured() {
  return Boolean(
    priceIdFor("plus", "monthly") &&
      priceIdFor("plus", "annual") &&
      priceIdFor("pro", "monthly") &&
      priceIdFor("pro", "annual")
  );
}

export async function createCheckoutSession(
  stripe: Stripe,
  db: Database.Database,
  input: z.infer<typeof checkoutSchema>
) {
  const priceId = priceIdFor(input.planId, input.interval);
  if (!priceId) {
    throw new Error(`Missing Stripe price id for ${input.planId}/${input.interval}`);
  }

  const account = upsertAccount(db, input.accountId);
  const session = await stripe.checkout.sessions.create({
    mode: "subscription",
    customer: account?.stripe_customer_id ?? undefined,
    client_reference_id: input.accountId,
    success_url: `${publicAppUrl()}?billing_status=success&session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${publicAppUrl()}?billing_status=cancelled`,
    allow_promotion_codes: true,
    line_items: [{ price: priceId, quantity: 1 }],
    metadata: {
      accountId: input.accountId,
      planId: input.planId,
      interval: input.interval,
    },
    subscription_data: {
      metadata: {
        accountId: input.accountId,
        planId: input.planId,
        interval: input.interval,
        includedCredits: String(includedCreditsForPlan(input.planId)),
      },
    },
  });

  if (!session.url) {
    throw new Error("Stripe did not return a checkout URL");
  }

  return {
    checkoutUrl: session.url,
    sessionId: session.id,
  };
}

export async function createPortalSession(
  stripe: Stripe,
  db: Database.Database,
  accountId: string
) {
  const account = findAccount(db, accountId);
  if (!account?.stripe_customer_id) {
    throw new Error("No Stripe customer is linked to this account");
  }

  const session = await stripe.billingPortal.sessions.create({
    customer: account.stripe_customer_id,
    return_url: `${publicAppUrl()}?billing_status=portal_return`,
    configuration: process.env.STRIPE_PORTAL_CONFIGURATION_ID?.trim() || undefined,
  });

  return { portalUrl: session.url };
}

export async function handleStripeEvent(
  stripe: Stripe,
  db: Database.Database,
  payload: Buffer,
  signature: string | undefined
) {
  const webhookSecret = process.env.STRIPE_WEBHOOK_SECRET;
  if (!webhookSecret) {
    throw new Error("STRIPE_WEBHOOK_SECRET is not configured");
  }
  if (!signature) {
    throw new Error("Missing Stripe signature");
  }

  const event = stripe.webhooks.constructEvent(payload, signature, webhookSecret);

  switch (event.type) {
    case "checkout.session.completed":
      await handleCheckoutCompleted(stripe, db, event.data.object);
      break;
    case "customer.subscription.created":
    case "customer.subscription.updated":
    case "customer.subscription.deleted":
      await handleSubscriptionChanged(stripe, db, event.data.object);
      break;
    default:
      break;
  }

  return { received: true, type: event.type };
}

async function handleCheckoutCompleted(
  stripe: Stripe,
  db: Database.Database,
  session: Stripe.Checkout.Session
) {
  const accountId = session.client_reference_id ?? session.metadata?.accountId;
  const customerId = asId(session.customer);

  if (accountId) {
    upsertAccount(db, accountId, customerId);
  }

  const subscriptionId = asId(session.subscription);
  if (subscriptionId) {
    const subscription = await stripe.subscriptions.retrieve(subscriptionId);
    await handleSubscriptionChanged(stripe, db, subscription);
  }
}

async function handleSubscriptionChanged(
  stripe: Stripe,
  db: Database.Database,
  subscription: Stripe.Subscription
) {
  const customerId = asId(subscription.customer);
  const accountId =
    subscription.metadata?.accountId ||
    (customerId ? findAccountByStripeCustomer(db, customerId)?.account_id : null);

  if (!accountId) {
    return;
  }

  const planId = inferPlanId(subscription);
  updateEntitlement(db, {
    accountId,
    stripeCustomerId: customerId,
    stripeSubscriptionId: subscription.id,
    planId,
    status: mapStripeStatus(subscription.status),
    currentPeriodStart: unixToIso(subscription.current_period_start),
    currentPeriodEnd: unixToIso(subscription.current_period_end),
    trialEndsAt: unixToIso(subscription.trial_end),
  });
}

function inferPlanId(subscription: Stripe.Subscription): BillingPlanId {
  const metadataPlan = subscription.metadata?.planId;
  if (metadataPlan === "plus" || metadataPlan === "pro") {
    return metadataPlan;
  }

  const priceId = subscription.items.data[0]?.price.id;
  if (
    priceId &&
    [priceIdFor("pro", "monthly"), priceIdFor("pro", "annual")].includes(priceId)
  ) {
    return "pro";
  }

  return "plus";
}

function mapStripeStatus(status: Stripe.Subscription.Status): SubscriptionStatus {
  switch (status) {
    case "active":
      return "active";
    case "trialing":
      return "trialing";
    case "past_due":
    case "unpaid":
      return "past_due";
    case "canceled":
    case "incomplete_expired":
      return "canceled";
    default:
      return "inactive";
  }
}

function unixToIso(value: number | null | undefined) {
  if (!value) return null;
  return new Date(value * 1000).toISOString();
}

function asId(
  value: string | Stripe.Customer | Stripe.DeletedCustomer | Stripe.Subscription | null
) {
  if (!value) return null;
  return typeof value === "string" ? value : value.id;
}

function publicAppUrl() {
  return process.env.PUBLIC_APP_URL?.trim() || "photocurate://billing";
}
