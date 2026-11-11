import { signRequest } from "@worldcoin/idkit/signing";
import type { RpContext } from "@worldcoin/idkit";

import {
  WORLD_ENVIRONMENTS,
  isLabel,
  worldAction,
  type SelfieCheckPurpose,
  type SignResponse,
  type WorldEnvironment,
} from "@/lib/selfie-check";

/**
 * POST /api/worldid/sign
 *
 * Signs a World ID request as the registered relying party, so the widget can raise a Selfie
 * Check. Ported from the Sepolia app on `main`.
 *
 * `WORLD_SIGNING_KEY` only proves who is asking. It never signs an attestation: that happens in
 * the OutLayer worker, with a key nobody holds.
 *
 * in:  { purpose: SelfieCheckPurpose }
 * out: { app_id, action, environment, rp_context }
 */

/** The RP signature's lifetime: long enough to pick up a phone, too short to reuse later. */
const RP_SIGNATURE_TTL_SECONDS = 120;

/** A bad request from the caller. Its message is safe to return. */
class BadRequestError extends Error {}

/** A missing or malformed environment variable. Its message names the key, never the value. */
class ConfigError extends Error {}

export async function POST(request: Request) {
  let purpose: SelfieCheckPurpose;
  try {
    purpose = parsePurpose(await request.json());
  } catch (error) {
    const message = error instanceof BadRequestError ? error.message : "body must be valid JSON";
    return Response.json({ error: message }, { status: 400 });
  }

  try {
    // Read inside the handler, so a missing key fails here with its name attached.
    const appId = required("WORLD_APP_ID", process.env.WORLD_APP_ID);
    const environment = required("WORLD_ENVIRONMENT", process.env.WORLD_ENVIRONMENT);
    const rpId = required("WORLD_RP_ID", process.env.WORLD_RP_ID);
    const signingKeyHex = required("WORLD_SIGNING_KEY", process.env.WORLD_SIGNING_KEY);

    if (!appId.startsWith("app_")) {
      throw new ConfigError("WORLD_APP_ID must start with 'app_'");
    }
    if (!isEnvironment(environment)) {
      throw new ConfigError(`WORLD_ENVIRONMENT must be one of ${WORLD_ENVIRONMENTS.join(", ")}`);
    }
    if (!/^(0x)?[0-9a-fA-F]{64}$/.test(signingKeyHex)) {
      // Checked here, so a malformed key is a config error we can log safely, rather than an
      // exception from `signRequest` whose message might quote the key.
      throw new ConfigError("WORLD_SIGNING_KEY must be 32 hex bytes");
    }

    // `signRequest` hashes the action into the signature, so the widget must request exactly this
    // string. The client uses the one returned here rather than recomputing it.
    const action = worldAction(purpose);
    const signature = signRequest({ signingKeyHex, action, ttl: RP_SIGNATURE_TTL_SECONDS });

    // `signRequest` returns an `RpSignature`; the widget wants an `RpContext`, with different field
    // names and the `rp_id` added.
    const rpContext: RpContext = {
      rp_id: rpId,
      nonce: signature.nonce,
      created_at: signature.createdAt,
      expires_at: signature.expiresAt,
      signature: signature.sig,
    };

    const body: SignResponse = {
      app_id: appId as `app_${string}`,
      action,
      environment,
      rp_context: rpContext,
    };
    return Response.json(body);
  } catch (error) {
    if (error instanceof ConfigError) {
      console.error(`[worldid/sign] configuration: ${error.message}`);
      return Response.json({ error: "the server is not configured for World ID" }, { status: 500 });
    }
    // Only the error's name: anything thrown inside the signer may quote its input.
    console.error(
      `[worldid/sign] signing failed: ${error instanceof Error ? error.name : "unknown"}`,
    );
    return Response.json({ error: "could not sign the verification request" }, { status: 500 });
  }
}

function required(key: string, value: string | undefined): string {
  if (!value) throw new ConfigError(`${key} is unset`);
  return value;
}

function isEnvironment(value: string): value is WorldEnvironment {
  return (WORLD_ENVIRONMENTS as readonly string[]).includes(value);
}

/**
 * The estate label ends up in the World action, the namespace a nullifier is scoped to, so junk is
 * refused here rather than producing a proof nothing on chain can match.
 */
function parsePurpose(body: unknown): SelfieCheckPurpose {
  if (typeof body !== "object" || body === null) {
    throw new BadRequestError("body must be a JSON object");
  }
  const { purpose } = body as { purpose?: unknown };
  if (typeof purpose !== "object" || purpose === null) {
    throw new BadRequestError("purpose is required");
  }
  const { kind, estateLabel, heirLabel } = purpose as Record<string, unknown>;
  if (!isLabel(estateLabel)) {
    throw new BadRequestError("estateLabel must be a single lowercase segment");
  }
  if (kind === "checkin" || kind === "withdraw") return { kind, estateLabel };
  if (kind === "claim") {
    if (!isLabel(heirLabel)) {
      throw new BadRequestError("heirLabel must be a single lowercase segment");
    }
    return { kind, estateLabel, heirLabel };
  }
  throw new BadRequestError("kind must be 'checkin', 'withdraw' or 'claim'");
}
