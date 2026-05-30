# PhotoCurate AI Billing

PhotoCurate supports two AI service modes:

- PhotoCurate AI: managed AI scoring provided by the product. This requires a paid entitlement before managed scoring can run.
- BYOK: users bring their own Gemini, Qwen-VL, or OpenAI-compatible Vision API key. This mode does not require a PhotoCurate subscription.

## Plans

| Plan | Price | Included managed AI credits | Target user |
| --- | --- | ---: | --- |
| Free Trial | Free | 20 | First import and evaluation |
| Plus | $6.99/month or $69/year | 1,000/month | Ongoing personal libraries |
| Pro | $14.99/month or $149/year | 5,000/month | Heavy photo library and portfolio users |

One credit maps to one managed AI photo analysis. BYOK calls do not consume managed credits.

## Entitlement States

The app reads billing through `get_billing_status`.

- `unconfigured`: no payment or entitlement backend is connected.
- `inactive`: payment channel is configured, but the user has no active subscription.
- `trialing` or `active`: managed AI can run if credits remain.
- `past_due` or `canceled`: managed AI is blocked, but BYOK remains available.

The frontend only enables managed AI scoring when `canUseManagedAi` is true.

## Payment Channels

Mac App Store builds must use App Store subscription products. The app integrates the Tauri IAP plugin, which calls StoreKit 2 on macOS. Create these auto-renewable subscription product IDs in App Store Connect:

- `com.photocurate.plus.monthly`
- `com.photocurate.plus.annual`
- `com.photocurate.pro.monthly`
- `com.photocurate.pro.annual`

For production entitlement durability, forward StoreKit signed transaction data to the billing service or App Store Server API before granting server-side managed AI access.

Website builds use Stripe through `services/billing_service`. Stripe secrets stay on the service, never in the Tauri app.

### Stripe Service Setup

```bash
cd services/billing_service
cp .env.example .env
npm install
npm run dev
```

Configure `.env`:

- `STRIPE_SECRET_KEY`
- `STRIPE_WEBHOOK_SECRET`
- `PUBLIC_APP_URL`
- `STRIPE_PRICE_PLUS_MONTHLY`
- `STRIPE_PRICE_PLUS_ANNUAL`
- `STRIPE_PRICE_PRO_MONTHLY`
- `STRIPE_PRICE_PRO_ANNUAL`

This follows the same security boundary as the Stripe donation flow in `51.isnap.world`: the app asks a backend to create a Stripe Checkout Session, and the backend keeps `STRIPE_SECRET_KEY`. The difference is that PhotoCurate uses subscription `price_...` IDs and webhooks, because entitlement cannot be granted from the redirect alone.

Point the desktop app at the service:

```bash
PHOTOCURATE_BILLING_PROVIDER=stripe \
PHOTOCURATE_BILLING_API_URL=http://127.0.0.1:8787 \
npm run tauri-dev
```

The billing service exposes:

- `GET /health`
- `GET /v1/billing/status/:accountId`
- `POST /v1/billing/checkout`
- `POST /v1/billing/portal`
- `POST /v1/stripe/webhook`

Stripe webhook events update entitlements in SQLite. Opening checkout is not enough to grant access; the webhook must confirm the subscription and return the authoritative entitlement and quota in `BillingStatus`.

Deployment is intentionally platform-neutral. The service is a standard Node HTTP service:

```bash
npm install
npm run build
npm start
```

Production needs persistent storage for `BILLING_DATABASE_PATH`, or a later migration from SQLite to a managed database. The desktop app only needs the public base URL in `PHOTOCURATE_BILLING_API_URL`.
