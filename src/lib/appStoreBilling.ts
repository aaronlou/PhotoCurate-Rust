import {
  getProductStatus,
  getProducts,
  purchase,
  restorePurchases,
  PurchaseState,
  type ProductStatus,
  type Purchase,
} from "@choochmeque/tauri-plugin-iap-api";
import type {
  BillingActionResult,
  BillingInterval,
  BillingPlanId,
  BillingStatus,
} from "@/types";

const PRODUCT_IDS: Record<Exclude<BillingPlanId, "free">, Record<BillingInterval, string>> = {
  plus: {
    monthly: "com.photocurate.plus.monthly",
    annual: "com.photocurate.plus.annual",
  },
  pro: {
    monthly: "com.photocurate.pro.monthly",
    annual: "com.photocurate.pro.annual",
  },
};

const INCLUDED_CREDITS: Record<BillingPlanId, number> = {
  free: 20,
  plus: 1000,
  pro: 5000,
};

export function isAppStoreBillingBuild() {
  return import.meta.env.VITE_APP_STORE === "true";
}

export function appStoreProductIds() {
  return [
    PRODUCT_IDS.plus.monthly,
    PRODUCT_IDS.plus.annual,
    PRODUCT_IDS.pro.monthly,
    PRODUCT_IDS.pro.annual,
  ];
}

export async function getAppStoreBillingStatus(accountId: string): Promise<BillingStatus> {
  const statuses = await Promise.all(
    appStoreProductIds().map(async (productId) => {
      try {
        return await getProductStatus(productId, "subs");
      } catch {
        return null;
      }
    })
  );

  const owned = statuses
    .filter((status): status is ProductStatus => Boolean(status?.isOwned))
    .sort((a, b) => (b.expirationTime ?? b.purchaseTime ?? 0) - (a.expirationTime ?? a.purchaseTime ?? 0))[0];

  if (!owned) {
    return localAppStoreStatus(accountId, "free", "inactive", null, "App Store 订阅尚未激活。");
  }

  return statusFromProductStatus(accountId, owned);
}

export async function startAppStorePurchase(
  currentStatus: BillingStatus | null,
  planId: BillingPlanId,
  interval: BillingInterval
): Promise<BillingActionResult> {
  if (planId === "free") {
    throw new Error("Free Trial 不需要购买");
  }

  const productId = PRODUCT_IDS[planId][interval];
  await getProducts([productId], "subs");
  const result = await purchase(productId, "subs", {
    appAccountToken: currentStatus?.accountId,
  });
  const status = statusFromPurchase(currentStatus?.accountId ?? "", result);

  return {
    status,
    message: "App Store 订阅已完成。若权益未立即刷新，请点击恢复购买。",
    checkoutUrl: null,
  };
}

export async function restoreAppStorePurchases(
  currentStatus: BillingStatus | null
): Promise<BillingActionResult> {
  const restored = await restorePurchases("subs");
  const active = restored.purchases
    .filter((item) => item.purchaseState === PurchaseState.PURCHASED)
    .sort((a, b) => b.purchaseTime - a.purchaseTime)[0];

  if (!active) {
    return {
      status: localAppStoreStatus(
        currentStatus?.accountId ?? "",
        "free",
        "inactive",
        null,
        "没有找到可恢复的 App Store 订阅。"
      ),
      message: "没有找到可恢复的 App Store 订阅。",
      checkoutUrl: null,
    };
  }

  return {
    status: statusFromPurchase(currentStatus?.accountId ?? "", active),
    message: "已恢复 App Store 订阅。",
    checkoutUrl: null,
  };
}

function statusFromProductStatus(accountId: string, productStatus: ProductStatus): BillingStatus {
  const planId = planIdFromProductId(productStatus.productId);
  const active = productStatus.purchaseState === PurchaseState.PURCHASED && productStatus.isOwned;
  return localAppStoreStatus(
    accountId,
    planId,
    active ? "active" : "inactive",
    productStatus.expirationTime ? new Date(productStatus.expirationTime).toISOString() : null,
    null
  );
}

function statusFromPurchase(accountId: string, item: Purchase): BillingStatus {
  const planId = planIdFromProductId(item.productId);
  const active = item.purchaseState === PurchaseState.PURCHASED;
  return localAppStoreStatus(
    accountId,
    planId,
    active ? "active" : "inactive",
    null,
    active ? null : "App Store 购买尚未完成。"
  );
}

function localAppStoreStatus(
  accountId: string,
  planId: BillingPlanId,
  status: BillingStatus["status"],
  renewsAt: string | null,
  message: string | null
): BillingStatus {
  const includedCredits = INCLUDED_CREDITS[planId];
  return {
    provider: "app_store",
    planId,
    status,
    accountId,
    renewsAt,
    trialEndsAt: null,
    usage: {
      periodStart: null,
      periodEnd: renewsAt,
      includedCredits,
      usedCredits: 0,
      remainingCredits: includedCredits,
    },
    canUseManagedAi: status === "active",
    checkoutAvailable: true,
    restoreAvailable: true,
    billingPortalAvailable: false,
    message,
  };
}

function planIdFromProductId(productId: string): BillingPlanId {
  if (productId.startsWith("com.photocurate.pro.")) {
    return "pro";
  }
  if (productId.startsWith("com.photocurate.plus.")) {
    return "plus";
  }
  return "free";
}
