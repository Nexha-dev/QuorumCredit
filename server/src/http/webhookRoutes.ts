/**
 * Webhook Routes for QuorumCredit
 * 
 * Implements webhook registration and verification endpoints.
 */

import type { IncomingMessage, ServerResponse } from "node:http";
import { request as httpsRequest } from "node:https";
import { request as httpRequest } from "node:http";
import {
  webhookRegistry,
  validateIncomingWebhook,
  createSignedWebhookRequest,
  validateWebhookUrl,
} from "../webhooks/signature.js";
import {
  webhookDeliveryService,
  SUBSCRIBABLE_EVENTS,
  type WebhookSender,
} from "../webhooks/delivery.js";
import { metrics } from "./metricsRegistry.js";
import { verifyToken } from "../auth/tokens.js";
import type { ApiKeyStore } from "../auth/apiKeyStore.js";

export interface WebhookRoutesContext {
  webhookSecret?: string; // Secret for receiving webhooks (if this service receives webhooks)
  /**
   * Secret used to verify `Authorization: Bearer <token>` on webhook routes.
   * Declared here because `requireWebhookAuth` reads it; without the field the
   * handler could never authenticate a token.
   */
  authSecret?: string;
  /** Provisioned API key store backing `X-Api-Key` authentication. */
  apiKeyStore?: ApiKeyStore;
  /**
   * Rate limiter for the test-delivery endpoint. When provided, caps the number
   * of test-delivery requests per IP per minute to prevent request amplification.
   */
  testDeliveryRateLimiter?: {
    isBlocked: (ip: string) => Promise<boolean>;
    recordFailure: (ip: string) => Promise<boolean>;
  };
  /**
   * Maximum concurrent test-delivery requests allowed globally.
   * When the limit is reached, additional requests are rejected with 429.
   */
  maxConcurrentTestDeliveries?: number;
}

interface RegisterWebhookBody {
  url: string;
  events: string[];
}

interface UpdateWebhookBody {
  url?: string;
  events?: string[];
  enabled?: boolean;
}

interface TestWebhookBody {
  event: string;
  data: any;
}

/**
 * Handle webhook-related HTTP requests
 */
export function handleWebhookRequest(
  req: IncomingMessage,
  res: ServerResponse,
  ctx: WebhookRoutesContext
): void {
  const url = new URL(req.url ?? "", "http://internal");

  // Webhook registration endpoints (protected in production)
  if (
    req.method === "POST" &&
    (url.pathname === "/api/webhooks/register" ||
      url.pathname === "/webhooks/subscribe" ||
      url.pathname === "/api/webhooks/subscribe")
  ) {
    void handleRegisterWebhook(req, res, ctx);
    return;
  }

  // Delivery success-rate tracking for a subscription.
  if (req.method === "GET" && url.pathname.match(/^\/api\/webhooks\/[^/]+\/deliveries$/)) {
    const id = url.pathname.split("/")[3] as string;
    void handleListDeliveries(res, id);
    return;
  }

  if (req.method === "GET" && url.pathname.match(/^\/api\/webhooks\/[^/]+\/stats$/)) {
    const id = url.pathname.split("/")[3] as string;
    void handleDeliveryStats(res, id);
    return;
  }

  if (req.method === "GET" && url.pathname === "/api/webhooks") {
    void handleListWebhooks(req, res);
    return;
  }

  if (req.method === "GET" && url.pathname.startsWith("/api/webhooks/")) {
    const id = url.pathname.split("/").pop();
    if (id && id !== "register") {
      void handleGetWebhook(req, res, id);
      return;
    }
  }

  if (req.method === "PUT" && url.pathname.startsWith("/api/webhooks/")) {
    const id = url.pathname.split("/").pop();
    if (id && id !== "register") {
      void handleUpdateWebhook(req, res, id, ctx);
      return;
    }
  }

  if (req.method === "DELETE" && url.pathname.startsWith("/api/webhooks/")) {
    const id = url.pathname.split("/").pop();
    if (id && id !== "register") {
      void handleDeleteWebhook(req, res, id, ctx);
      return;
    }
  }

  if (req.method === "POST" && url.pathname.startsWith("/api/webhooks/") && url.pathname.endsWith("/test")) {
    const id = url.pathname.split("/")[3]; // /api/webhooks/{id}/test
    if (id) {
      void handleTestWebhook(req, res, id, ctx);
      return;
    }
  }

  // Webhook verification endpoint (for receiving webhooks)
  if (req.method === "POST" && url.pathname === "/webhook") {
    void handleIncomingWebhook(req, res, ctx);
    return;
  }

  res.writeHead(404, { "content-type": "application/json" });
  res.end(JSON.stringify({ error: "not found" }));
}

/**
 * Register a new webhook
 */
async function handleRegisterWebhook(req: IncomingMessage, res: ServerResponse, ctx: WebhookRoutesContext): Promise<void> {
  try {
    if (await requireWebhookAuth(req, res, ctx)) return;

    const body = await readJsonBody<RegisterWebhookBody>(req);
    
    if (!body.url || !body.events || !Array.isArray(body.events)) {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "url and events array are required" }));
      return;
    }

    // Validate URL scheme and reject SSRF-prone hosts (issue #1486)
    let validatedUrl: URL;
    try {
      validatedUrl = await validateWebhookUrlSafe(body.url);
    } catch (error) {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: error instanceof Error ? error.message : "invalid URL" }));
      return;
    }

    // Validate events. `SUBSCRIBABLE_EVENTS` are the canonical real-time
    // event names (loan_issued, payment_received, loan_completed,
    // default_occurred); the dot-separated names are kept for backward
    // compatibility with existing registrations.
    const validEvents = [
      ...SUBSCRIBABLE_EVENTS,
      "loan.requested",
      "loan.disbursed",
      "loan.repaid",
      "loan.defaulted",
      "vouch.created",
      "vouch.withdrawn",
      "slash.executed",
      "config.updated",
    ];

    const invalidEvents = body.events.filter(event => !validEvents.includes(event));
    if (invalidEvents.length > 0) {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ 
        error: "invalid events", 
        invalidEvents,
        validEvents 
      }));
      return;
    }

    // Register webhook
    const registration = await webhookRegistry.registerWebhook(validatedUrl.toString(), body.events);

    // Return registration (excluding secret for security)
    const { secret, ...safeRegistration } = registration;
    
    res.writeHead(201, { "content-type": "application/json" });
    res.end(JSON.stringify({
      ...safeRegistration,
      message: "Webhook registered successfully. Save the secret shown below - it will not be shown again.",
      secret, // Only returned once during registration
    }));
  } catch (error) {
    console.error("Error registering webhook:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * List all webhooks
 */
async function handleListWebhooks(_req: IncomingMessage, res: ServerResponse): Promise<void> {
  try {
    const webhooks = await webhookRegistry.listWebhooks();
    
    // Remove secrets from response
    const safeWebhooks = webhooks.map(({ secret, ...rest }) => rest);
    
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(safeWebhooks));
  } catch (error) {
    console.error("Error listing webhooks:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * Get a specific webhook
 */
async function handleGetWebhook(_req: IncomingMessage, res: ServerResponse, id: string): Promise<void> {
  try {
    const webhook = await webhookRegistry.getWebhook(id);
    
    if (!webhook) {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "webhook not found" }));
      return;
    }

    // Remove secret from response
    const { secret, ...safeWebhook } = webhook;
    
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(safeWebhook));
  } catch (error) {
    console.error("Error getting webhook:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * Update a webhook
 */
async function handleUpdateWebhook(req: IncomingMessage, res: ServerResponse, id: string, ctx: WebhookRoutesContext): Promise<void> {
  try {
    if (await requireWebhookAuth(req, res, ctx)) return;

    const webhook = await webhookRegistry.getWebhook(id);
    
    if (!webhook) {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "webhook not found" }));
      return;
    }

    const body = await readJsonBody<UpdateWebhookBody>(req);

    // Validate updates
    if (body.url !== undefined) {
      let validatedUrl: URL;
      try {
        validatedUrl = await validateWebhookUrlSafe(body.url);
      } catch (error) {
        res.writeHead(400, { "content-type": "application/json" });
        res.end(JSON.stringify({ error: error instanceof Error ? error.message : "invalid URL" }));
        return;
      }
      webhook.url = validatedUrl.toString();
    }

    if (body.events !== undefined) {
      if (!Array.isArray(body.events)) {
        res.writeHead(400, { "content-type": "application/json" });
        res.end(JSON.stringify({ error: "events must be an array" }));
        return;
      }
      webhook.events = body.events;
    }

    if (body.enabled !== undefined) {
      if (body.enabled) {
        await webhookRegistry.enableWebhook(id);
      } else {
        await webhookRegistry.disableWebhook(id);
      }
    }

    // Update last used timestamp
    await webhookRegistry.updateLastUsed(id);

    // Return updated webhook (excluding secret)
    const { secret, ...safeWebhook } = webhook;
    
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(safeWebhook));
  } catch (error) {
    console.error("Error updating webhook:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * Delete a webhook
 */
async function handleDeleteWebhook(req: IncomingMessage, res: ServerResponse, id: string, ctx: WebhookRoutesContext): Promise<void> {
  try {
    if (await requireWebhookAuth(req, res, ctx)) return;

    const deleted = await webhookRegistry.deleteWebhook(id);
    
    if (!deleted) {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "webhook not found" }));
      return;
    }

    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ message: "webhook deleted" }));
  } catch (error) {
    console.error("Error deleting webhook:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * List delivery attempts recorded for a webhook subscription.
 */
async function handleListDeliveries(res: ServerResponse, id: string): Promise<void> {
  try {
    const webhook = await webhookRegistry.getWebhook(id);
    if (!webhook) {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "webhook not found" }));
      return;
    }

    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(webhookDeliveryService.listDeliveries(id)));
  } catch (error) {
    console.error("Error listing webhook deliveries:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * Report the delivery success rate for a webhook subscription, per the
 * "track webhook delivery success rates" requirement.
 */
async function handleDeliveryStats(res: ServerResponse, id: string): Promise<void> {
  try {
    const webhook = await webhookRegistry.getWebhook(id);
    if (!webhook) {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "webhook not found" }));
      return;
    }

    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(webhookDeliveryService.stats(id)));
  } catch (error) {
    console.error("Error computing webhook delivery stats:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * Real HTTP(S) sender used to actually deliver webhook payloads to
 * subscriber URLs. Kept separate from WebhookDeliveryService so the retry
 * logic there stays transport-agnostic and unit-testable.
 */
const sendWebhookOverHttp: WebhookSender = (url, headers, body) => {
  return new Promise((resolve) => {
    const target = new URL(url);
    const transport = target.protocol === "http:" ? httpRequest : httpsRequest;
    const payload = JSON.stringify(body);

    const req = transport(
      {
        hostname: target.hostname,
        port: target.port || (target.protocol === "http:" ? 80 : 443),
        path: `${target.pathname}${target.search}`,
        method: "POST",
        headers: { ...headers, "content-length": Buffer.byteLength(payload) },
        timeout: 10_000,
      },
      (res) => {
        res.on("data", () => undefined);
        res.on("end", () => {
          const statusCode = res.statusCode ?? 0;
          resolve({ ok: statusCode >= 200 && statusCode < 300, statusCode });
        });
      }
    );

    req.on("timeout", () => req.destroy(new Error("webhook delivery timed out")));
    req.on("error", (error) => resolve({ ok: false, error: error.message }));
    req.write(payload);
    req.end();
  });
};

/**
 * Dispatch an event to every enabled subscriber registered for it, with
 * exponential-backoff retry (max 5 retries) via WebhookDeliveryService.
 * This is the entry point loan-lifecycle code (issuance, repayment,
 * completion, default) should call in place of relying on third-party apps
 * polling for status.
 */
export async function dispatchEventToSubscribers(event: string, data: unknown): Promise<void> {
  const subscribers = await webhookRegistry.getWebhooksForEvent(event);
  await Promise.all(
    subscribers.map((webhook) =>
      webhookDeliveryService.deliver(webhook, event, data, sendWebhookOverHttp)
    )
  );
}

/**
 * Test a webhook
 */
async function handleTestWebhook(req: IncomingMessage, res: ServerResponse, id: string, ctx: WebhookRoutesContext): Promise<void> {
  try {
    if (await requireWebhookAuth(req, res, ctx)) return;

    const webhook = await webhookRegistry.getWebhook(id);

    if (!webhook) {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "webhook not found" }));
      return;
    }

    if (!webhook.enabled) {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "webhook is disabled" }));
      return;
    }

    const body = await readJsonBody<TestWebhookBody>(req);

    if (!body.event || !body.data) {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "event and data are required" }));
      return;
    }

    // Issue #1491: Rate-limit and cap concurrency for the test-delivery path.
    const clientIp = req.socket.remoteAddress ?? "unknown";
    const rateLimiter = ctx.testDeliveryRateLimiter;
    const maxConcurrent = ctx.maxConcurrentTestDeliveries ?? 10;
    const inFlight = (globalThis as any).__qcTestDeliveriesInFlight ?? 0;
    if (inFlight >= maxConcurrent) {
      metrics.incCounter("qc_webhook_test_delivery_throttled_total");
      res.writeHead(429, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "too many concurrent test deliveries; try again later" }));
      return;
    }
    if (rateLimiter && (await rateLimiter.isBlocked(clientIp))) {
      metrics.incCounter("qc_webhook_test_delivery_throttled_total");
      res.writeHead(429, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "rate limit exceeded; try again later" }));
      return;
    }

    (globalThis as any).__qcTestDeliveriesInFlight = inFlight + 1;
    try {
      // Simulate a test delivery to measure latency and retries without spamming
      // the subscriber URL. We reuse the delivery service's retry/backoff logic
      // against a no-op sender so the endpoint itself is exercised, but the
      // actual outbound HTTP request is skipped.
      const noopSender: WebhookSender = async () => ({ ok: true, statusCode: 200 });
      const record = await webhookDeliveryService.deliver(webhook, body.event, body.data, noopSender, (ms: number) => Promise.resolve());
      metrics.incCounter("qc_webhook_test_delivery_total");
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({
        message: "Test webhook delivered",
        delivery: record,
      }));
    } finally {
      (globalThis as any).__qcTestDeliveriesInFlight = Math.max(0, (globalThis as any).__qcTestDeliveriesInFlight - 1);
      if (rateLimiter) {
        await rateLimiter.recordFailure(clientIp);
      }
    }
  } catch (error) {
    console.error("Error testing webhook:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * Handle incoming webhook (for services that receive webhooks)
 */
async function handleIncomingWebhook(
  req: IncomingMessage,
  res: ServerResponse,
  ctx: WebhookRoutesContext
): Promise<void> {
  try {
    if (!ctx.webhookSecret) {
      res.writeHead(501, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "webhook receiving not configured" }));
      return;
    }

    const body = await readJsonBody<any>(req);
    
    // Extract headers
    const headers: Record<string, string | string[]> = {};
    for (const [key, value] of Object.entries(req.headers)) {
      headers[key.toLowerCase()] = value || "";
    }

    // Validate webhook signature
    const validation = validateIncomingWebhook(body, headers, ctx.webhookSecret);
    
    if (!validation.valid) {
      console.warn("Invalid webhook received:", validation.error);
      res.writeHead(401, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "invalid webhook signature", details: validation.error }));
      return;
    }

    // Process valid webhook
    console.log("Received valid webhook:", validation.payload);
    
    // Here you would process the webhook payload based on the event type
    // For now, we'll just acknowledge receipt
    
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ 
      status: "accepted",
      event: validation.payload!.event,
      webhookId: validation.payload!.webhookId,
      timestamp: validation.payload!.timestamp,
    }));
  } catch (error) {
    console.error("Error handling incoming webhook:", error);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "internal server error" }));
  }
}

/**
 * Helper function to read JSON request body
 */
function readJsonBody<T>(req: IncomingMessage): Promise<T> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => {
      try {
        resolve(chunks.length > 0 ? JSON.parse(Buffer.concat(chunks).toString("utf8")) : {} as T);
      } catch (e) {
        reject(e);
      }
    });
    req.on("error", reject);
  });
}

// ── Auth helpers (issue #1487) ─────────────────────────────────────────────────

function extractBearerToken(req: IncomingMessage): string | undefined {
  const authHeader = req.headers["authorization"];
  if (!authHeader || typeof authHeader !== "string") return undefined;
  const match = authHeader.match(/^Bearer\s+(.+)$/i);
  return match ? match[1] : undefined;
}

function extractApiKey(req: IncomingMessage): string | undefined {
  return (req.headers["x-api-key"] as string | undefined) ?? undefined;
}

async function requireWebhookAuth(req: IncomingMessage, res: ServerResponse, ctx: WebhookRoutesContext): Promise<boolean> {
  const token = extractBearerToken(req);
  const apiKey = extractApiKey(req);

  if (!token && !apiKey) {
    res.writeHead(401, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "missing credentials; provide Authorization: Bearer <token> or X-Api-Key" }));
    return true;
  }

  if (token && ctx.authSecret) {
    const result = verifyToken(ctx.authSecret, token);
    if (result.valid) return false;
  }

  if (apiKey && ctx.apiKeyStore) {
    if (ctx.apiKeyStore.isValid(apiKey)) return false;
  }

  res.writeHead(401, { "content-type": "application/json" });
  res.end(JSON.stringify({ error: "invalid credentials" }));
  return true;
}

// ── SSRF validation helpers (issue #1486) ─────────────────────────────────────

function getDevModeFlag(): boolean {
  return process.env.NODE_ENV !== "production";
}

async function validateWebhookUrlSafe(rawUrl: string): Promise<URL> {
  return validateWebhookUrl(rawUrl, { allowPrivateHosts: getDevModeFlag() });
}