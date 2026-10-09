import type { Response } from "express";

/**
 * Shape returned for a request that failed schema validation.
 *
 * Zod's `message` is a JSON-encoded array of issues, which is awkward for API
 * clients: it is a string that has to be parsed again, and the useful part (the
 * field and the reason) is nested inside. This helper keeps the 400 status,
 * gives `error` a one-line human-readable summary, and adds the full list of
 * issues under `issues` for a client that wants to highlight fields.
 */
export interface ValidationIssue {
  path: string;
  message: string;
}

interface ZodLikeError {
  issues: { path: PropertyKey[]; message: string }[];
}

export function validationIssues(error: ZodLikeError): ValidationIssue[] {
  return error.issues.map((issue) => ({
    path: issue.path.map(String).join(".") || "(body)",
    message: issue.message,
  }));
}

/**
 * Guard for generated query schemas that use `zod.coerce.string()` on a
 * required field: coercion turns a *missing* value into the string
 * `"undefined"` (and an empty one into `""`), so `safeParse` succeeds and the
 * route proceeds with a nonsensical value — e.g. `GET /api/search` searched for
 * the literal word "undefined". The OpenAPI spec marks these fields required;
 * this helper enforces that before the schema runs.
 */
export function missingRequiredParams(
  values: Record<string, unknown>,
  names: string[]
): string[] {
  return names.filter((n) => {
    const v = values[n];
    return typeof v !== "string" || v.trim() === "";
  });
}

/** Send a 400 with a concise `error` and a structured `issues` array. */
export function invalidRequest(res: Response, error: ZodLikeError): void {
  const issues = validationIssues(error);
  const first = issues[0];
  res.status(400).json({
    error: first ? `Invalid request: ${first.path} ${first.message}` : "Invalid request",
    issues,
  });
}
