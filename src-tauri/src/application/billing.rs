use chrono::Utc;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::env;
use uuid::Uuid;

use crate::domain::models::{BillingActionResult, BillingStatus, BillingUsage};
use crate::error::{PhotoCurateError, Result};

const FREE_INCLUDED_CREDITS: i64 = 20;
const BILLING_ACCOUNT_ID_KEY: &str = "billing_account_id";
const DEFAULT_BILLING_API_URL: &str = "http://127.0.0.1:8787";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckoutRequest {
    account_id: String,
    plan_id: String,
    interval: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortalRequest {
    account_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckoutResponse {
    checkout_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortalResponse {
    portal_url: String,
}

pub async fn get_billing_status(db: &Pool<Sqlite>) -> Result<BillingStatus> {
    let account_id = get_or_create_account_id(db).await?;
    match configured_provider() {
        "stripe" => fetch_remote_status(&account_id).await.or_else(|err| {
            tracing::warn!("Failed to fetch Stripe billing status: {}", err);
            Ok(local_status(
                account_id,
                "stripe",
                "inactive",
                Some("无法连接 PhotoCurate Billing 服务。请确认服务端已部署，或稍后重试。".into()),
            ))
        }),
        "app_store" => Ok(local_status(
            account_id,
            "app_store",
            "inactive",
            Some("App Store 订阅会通过系统购买和恢复，不读取钥匙串。".into()),
        )),
        "manual" => Ok(local_status(
            account_id,
            "manual",
            "inactive",
            Some("当前为手动开通模式，需要服务端写入订阅权益。".into()),
        )),
        _ => Ok(local_status(
            account_id,
            "none",
            "unconfigured",
            Some("PhotoCurate AI 订阅还未连接支付和权益服务。自带 API Key 模式不受影响。".into()),
        )),
    }
}

pub async fn start_managed_ai_checkout(
    db: &Pool<Sqlite>,
    plan_id: String,
    interval: String,
) -> Result<BillingActionResult> {
    let account_id = get_or_create_account_id(db).await?;
    let provider = configured_provider();

    if provider == "stripe" {
        let client = reqwest::Client::new();
        let url = format!("{}/v1/billing/checkout", billing_api_url());
        let response = client
            .post(url)
            .json(&CheckoutRequest {
                account_id: account_id.clone(),
                plan_id: plan_id.clone(),
                interval: interval.clone(),
            })
            .send()
            .await
            .map_err(|err| PhotoCurateError::Other(format!("无法连接 Billing 服务: {err}")))?;

        if !response.status().is_success() {
            return Err(remote_error(response).await);
        }

        let checkout = response
            .json::<CheckoutResponse>()
            .await
            .map_err(|err| PhotoCurateError::Other(format!("Billing 服务响应无效: {err}")))?;
        let status = fetch_remote_status(&account_id)
            .await
            .unwrap_or_else(|_| local_status(account_id, "stripe", "inactive", None));

        return Ok(BillingActionResult {
            status,
            message: format!(
                "正在打开 {} {}订阅支付页面。",
                plan_label(&plan_id),
                interval_label(&interval)
            ),
            checkout_url: Some(checkout.checkout_url),
        });
    }

    if provider == "app_store" {
        return Ok(BillingActionResult {
            status: local_status(
                account_id,
                "app_store",
                "inactive",
                Some("请通过 App Store 购买面板完成订阅。".into()),
            ),
            message: "App Store 订阅需要在当前页面点击系统购买按钮完成。".into(),
            checkout_url: None,
        });
    }

    Ok(BillingActionResult {
        status: local_status(
            account_id,
            "none",
            "unconfigured",
            Some("支付渠道尚未配置。".into()),
        ),
        message: "请先配置 PHOTOCURATE_BILLING_API_URL 和支付服务端。".into(),
        checkout_url: None,
    })
}

pub async fn restore_managed_ai_purchases(db: &Pool<Sqlite>) -> Result<BillingActionResult> {
    Ok(BillingActionResult {
        status: get_billing_status(db).await?,
        message: if configured_provider() == "app_store" {
            "App Store 恢复购买由系统购买通道处理。".into()
        } else {
            "官网版订阅会通过 Stripe webhook 自动恢复到当前匿名账号；可打开订阅管理页查看。".into()
        },
        checkout_url: None,
    })
}

pub async fn open_billing_portal(db: &Pool<Sqlite>) -> Result<BillingActionResult> {
    let account_id = get_or_create_account_id(db).await?;

    if configured_provider() == "stripe" {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/billing/portal", billing_api_url()))
            .json(&PortalRequest {
                account_id: account_id.clone(),
            })
            .send()
            .await
            .map_err(|err| PhotoCurateError::Other(format!("无法连接 Billing 服务: {err}")))?;

        if !response.status().is_success() {
            return Err(remote_error(response).await);
        }

        let portal = response
            .json::<PortalResponse>()
            .await
            .map_err(|err| PhotoCurateError::Other(format!("Billing 服务响应无效: {err}")))?;

        return Ok(BillingActionResult {
            status: fetch_remote_status(&account_id)
                .await
                .unwrap_or_else(|_| local_status(account_id, "stripe", "inactive", None)),
            message: "正在打开 Stripe 订阅管理页面。".into(),
            checkout_url: Some(portal.portal_url),
        });
    }

    Ok(BillingActionResult {
        status: get_billing_status(db).await?,
        message: "当前支付渠道没有可打开的订阅管理页面。".into(),
        checkout_url: None,
    })
}

async fn fetch_remote_status(account_id: &str) -> Result<BillingStatus> {
    let response = reqwest::get(format!(
        "{}/v1/billing/status/{}",
        billing_api_url(),
        account_id
    ))
    .await
    .map_err(|err| PhotoCurateError::Other(format!("无法连接 Billing 服务: {err}")))?;

    if !response.status().is_success() {
        return Err(remote_error(response).await);
    }

    response
        .json::<BillingStatus>()
        .await
        .map_err(|err| PhotoCurateError::Other(format!("Billing 服务响应无效: {err}")))
}

async fn get_or_create_account_id(db: &Pool<Sqlite>) -> Result<String> {
    if let Some((value,)) =
        sqlx::query_as::<_, (String,)>("SELECT value FROM app_metadata WHERE key = ?1 LIMIT 1")
            .bind(BILLING_ACCOUNT_ID_KEY)
            .fetch_optional(db)
            .await?
    {
        if Uuid::parse_str(&value).is_ok() {
            return Ok(value);
        }
    }

    let account_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO app_metadata (key, value, updated_at)
        VALUES (?1, ?2, CURRENT_TIMESTAMP)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(BILLING_ACCOUNT_ID_KEY)
    .bind(&account_id)
    .execute(db)
    .await?;

    Ok(account_id)
}

async fn remote_error(response: reqwest::Response) -> PhotoCurateError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = if body.trim().is_empty() {
        format!("Billing 服务返回 {status}")
    } else {
        format!("Billing 服务返回 {status}: {body}")
    };

    if status == StatusCode::NOT_FOUND {
        PhotoCurateError::Other("当前订阅账号还没有可管理的 Stripe 客户信息。".into())
    } else {
        PhotoCurateError::Other(message)
    }
}

fn local_status(
    account_id: String,
    provider: &str,
    status: &str,
    message: Option<String>,
) -> BillingStatus {
    BillingStatus {
        provider: provider.into(),
        plan_id: "free".into(),
        status: status.into(),
        account_id,
        renews_at: None,
        trial_ends_at: None,
        usage: BillingUsage {
            period_start: Some(Utc::now()),
            period_end: None,
            included_credits: FREE_INCLUDED_CREDITS,
            used_credits: 0,
            remaining_credits: FREE_INCLUDED_CREDITS,
        },
        can_use_managed_ai: false,
        checkout_available: provider == "stripe",
        restore_available: provider == "app_store",
        billing_portal_available: provider == "stripe",
        message,
    }
}

fn configured_provider() -> &'static str {
    if cfg!(feature = "app-store") {
        return "app_store";
    }

    match env::var("PHOTOCURATE_BILLING_PROVIDER") {
        Ok(provider) if provider == "stripe" => "stripe",
        Ok(provider) if provider == "app_store" => "app_store",
        Ok(provider) if provider == "manual" => "manual",
        _ if env::var("PHOTOCURATE_BILLING_API_URL").is_ok() => "stripe",
        _ => "none",
    }
}

fn billing_api_url() -> String {
    env::var("PHOTOCURATE_BILLING_API_URL")
        .unwrap_or_else(|_| DEFAULT_BILLING_API_URL.into())
        .trim_end_matches('/')
        .to_string()
}

fn plan_label(plan_id: &str) -> &'static str {
    match plan_id {
        "plus" => "Plus",
        "pro" => "Pro",
        _ => "Free Trial",
    }
}

fn interval_label(interval: &str) -> &'static str {
    match interval {
        "annual" => "年付",
        _ => "月付",
    }
}
