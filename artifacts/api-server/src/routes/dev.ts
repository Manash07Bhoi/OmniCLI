import { Router, type IRouter } from "express";
import crypto from "crypto";
import {
  DevHashBody,
  DevHashResponse as DevHashResult,
  DevJsonBody,
  DevJsonResponse as DevJsonResult,
  DevBase64Body,
  DevBase64Response as DevBase64Result,
  DevUuidQueryParams as DevUuidParams,
  DevUuidResponse as DevUuidResult,
  DevRegexBody,
  DevRegexResponse as DevRegexResult,
  DevJwtBody,
  DevJwtResponse as DevJwtResult,
} from "@workspace/api-zod";

const router: IRouter = Router();

router.post("/dev/hash", (req, res): void => {
  const parsed = DevHashBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { input, algo = "sha256" } = parsed.data;
  const nodeAlgo =
    algo === "blake3" ? "sha256" : // Node.js doesn't support BLAKE3 natively; use SHA-256 as stand-in
    algo === "sha256" ? "sha256" :
    algo === "md5" ? "md5" :
    algo === "sha1" ? "sha1" : "sha256";

  const digest = crypto.createHash(nodeAlgo).update(input, "utf8").digest("hex");
  res.json(DevHashResult.parse({ input, algo, digest }));
});

router.post("/dev/json", (req, res): void => {
  const parsed = DevJsonBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { input, action = "pretty", query } = parsed.data;

  let parsed2: unknown;
  try {
    parsed2 = JSON.parse(input);
  } catch (e: unknown) {
    if (action === "validate") {
      res.json(DevJsonResult.parse({ valid: false, output: "", error: String(e) }));
      return;
    }
    res.status(400).json({ error: `Invalid JSON: ${String(e)}` });
    return;
  }

  let output: string;
  if (action === "minify") {
    output = JSON.stringify(parsed2);
  } else if (action === "validate") {
    output = "Valid JSON";
  } else {
    // pretty
    output = JSON.stringify(parsed2, null, 2);
  }

  // Simple jq-style key query: .key.subkey
  if (query) {
    try {
      const keys = query.replace(/^\./, "").split(".");
      let val: unknown = parsed2;
      for (const k of keys) {
        if (val !== null && typeof val === "object" && k in (val as Record<string, unknown>)) {
          val = (val as Record<string, unknown>)[k];
        } else {
          val = undefined;
          break;
        }
      }
      output = val === undefined ? "null" : JSON.stringify(val, null, 2);
    } catch {
      output = "Query error";
    }
  }

  res.json(DevJsonResult.parse({ valid: true, output, error: null }));
});

router.post("/dev/base64", (req, res): void => {
  const parsed = DevBase64Body.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { input, decode = false } = parsed.data;
  let output: string;

  if (decode) {
    try {
      output = Buffer.from(input, "base64").toString("utf8");
    } catch {
      output = Buffer.from(input, "base64").toString("binary");
    }
  } else {
    output = Buffer.from(input, "utf8").toString("base64");
  }

  res.json(DevBase64Result.parse({ input, output, decoded: decode }));
});

router.get("/dev/uuid", (req, res): void => {
  const parsed = DevUuidParams.safeParse(req.query);
  const count = parsed.success ? Math.min(parsed.data.count ?? 1, 100) : 1;

  const uuids: string[] = [];
  for (let i = 0; i < count; i++) {
    uuids.push(crypto.randomUUID());
  }

  res.json(DevUuidResult.parse({ uuids }));
});

router.post("/dev/regex", (req, res): void => {
  const parsed = DevRegexBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { pattern, text, flags = "g" } = parsed.data;

  let re: RegExp;
  try {
    re = new RegExp(pattern, flags.includes("g") ? flags : flags + "g");
  } catch (e: unknown) {
    res.status(400).json({ error: `Invalid regex: ${String(e)}` });
    return;
  }

  const matches: { match: string; start: number; end: number; groups: string[] }[] = [];
  let m: RegExpExecArray | null;

  // Reset lastIndex for global regexes
  re.lastIndex = 0;
  while ((m = re.exec(text)) !== null) {
    matches.push({
      match: m[0],
      start: m.index,
      end: m.index + m[0].length,
      groups: m.slice(1).map((g) => g ?? ""),
    });
    if (matches.length >= 200) break; // cap
    if (!re.global) break;
  }

  res.json(
    DevRegexResult.parse({
      pattern,
      matched: matches.length > 0,
      matchCount: matches.length,
      matches,
      error: null,
    })
  );
});

router.post("/dev/jwt", (req, res): void => {
  const parsed = DevJwtBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { token } = parsed.data;
  const parts = token.split(".");

  if (parts.length !== 3) {
    res.status(400).json({ error: "Invalid JWT: expected 3 dot-separated parts" });
    return;
  }

  function base64UrlDecode(s: string): string {
    const padded = s.replace(/-/g, "+").replace(/_/g, "/").padEnd(s.length + ((4 - (s.length % 4)) % 4), "=");
    return Buffer.from(padded, "base64").toString("utf8");
  }

  let header: string;
  let payload: string;
  let isExpired: boolean | null = null;
  let expiresAt: string | null = null;
  let error: string | null = null;

  try {
    header = JSON.stringify(JSON.parse(base64UrlDecode(parts[0]!)), null, 2);
  } catch {
    header = parts[0]!;
    error = "Could not decode header";
  }

  try {
    const payloadObj = JSON.parse(base64UrlDecode(parts[1]!));
    payload = JSON.stringify(payloadObj, null, 2);

    if (payloadObj.exp) {
      const expMs = payloadObj.exp * 1000;
      isExpired = Date.now() > expMs;
      expiresAt = new Date(expMs).toISOString();
    }
  } catch {
    payload = parts[1]!;
    error = error ?? "Could not decode payload";
  }

  res.json(
    DevJwtResult.parse({
      valid: error === null,
      header: header!,
      payload: payload!,
      signature: parts[2]!,
      isExpired,
      expiresAt,
      error,
    })
  );
});

export default router;
