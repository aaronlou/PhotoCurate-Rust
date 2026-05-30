import "dotenv/config";
import Fastify from "fastify";
import cors from "@fastify/cors";
import { z } from "zod";
import { openDatabase, findAccount, toBillingStatus, defaultBillingStatus } from "./db.js";
import {
  checkoutConfigured,
  checkoutSchema,
  createCheckoutSession,
  createPortalSession,
  createStripeClient,
  handleStripeEvent,
  isStripeConfigured,
  portalSchema,
} from "./stripe.js";

const port = Number(process.env.PORT ?? 8787);
const databasePath = process.env.BILLING_DATABASE_PATH?.trim() || "./billing.sqlite";
const db = openDatabase(databasePath);
const stripe = process.env.STRIPE_SECRET_KEY
  ? createStripeClient(process.env.STRIPE_SECRET_KEY)
  : null;

const app = Fastify({
  logger: true,
  bodyLimit: 1024 * 1024,
});

await app.register(cors, {
  origin: true,
});

app.addContentTypeParser(
  "application/json",
  { parseAs: "buffer" },
  (request, body, done) => {
    if (request.url === "/v1/stripe/webhook") {
      done(null, body);
      return;
    }

    try {
      done(null, JSON.parse(body.toString("utf8")));
    } catch (error) {
      done(error as Error);
    }
  }
);

app.get("/health", async () => ({
  ok: true,
  stripeConfigured: isStripeConfigured(),
  checkoutConfigured: checkoutConfigured(),
}));

app.get("/v1/billing/status/:accountId", async (request) => {
  const params = z.object({ accountId: z.string().uuid() }).parse(request.params);
  const account = findAccount(db, params.accountId);
  return account
    ? toBillingStatus(account, checkoutConfigured())
    : defaultBillingStatus(params.accountId, checkoutConfigured());
});

app.post("/v1/billing/checkout", async (request, reply) => {
  if (!stripe) {
    return reply.code(503).send({ error: "Stripe is not configured" });
  }

  const input = checkoutSchema.parse(request.body);
  const session = await createCheckoutSession(stripe, db, input);
  return session;
});

app.post("/v1/billing/portal", async (request, reply) => {
  if (!stripe) {
    return reply.code(503).send({ error: "Stripe is not configured" });
  }

  const input = portalSchema.parse(request.body);
  const session = await createPortalSession(stripe, db, input.accountId);
  return session;
});

app.post("/v1/stripe/webhook", async (request, reply) => {
  if (!stripe) {
    return reply.code(503).send({ error: "Stripe is not configured" });
  }
  if (!Buffer.isBuffer(request.body)) {
    return reply.code(400).send({ error: "Expected raw request body" });
  }

  const result = await handleStripeEvent(
    stripe,
    db,
    request.body,
    stripeSignature(request.headers["stripe-signature"])
  );
  return result;
});

function stripeSignature(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}

try {
  await app.listen({ port, host: "0.0.0.0" });
} catch (error) {
  app.log.error(error);
  process.exit(1);
}
