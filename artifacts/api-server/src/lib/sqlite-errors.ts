/**
 * better-sqlite3 reports constraint failures as `SqliteError` with a `code`
 * such as `SQLITE_CONSTRAINT_UNIQUE`. Routes use this to answer with a real
 * client error (409) instead of letting the driver error become a 500.
 */
export function isUniqueViolation(err: unknown): boolean {
  if (typeof err !== "object" || err === null) return false;
  const code = (err as { code?: unknown }).code;
  const message = String((err as { message?: unknown }).message ?? "");
  if (typeof code === "string" && code.startsWith("SQLITE_CONSTRAINT")) {
    return code.includes("UNIQUE") || code.includes("PRIMARYKEY") || /unique/i.test(message);
  }
  return /UNIQUE constraint failed/i.test(message);
}
