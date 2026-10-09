import { defineConfig } from "drizzle-kit";
import fs from "fs";
import path from "path";
import os from "os";

const defaultDbPath = path.join(
  process.env["XDG_DATA_HOME"] ?? path.join(os.homedir(), ".local", "share"),
  "omni",
  "omni.db"
);

const dbPath = process.env["DB_PATH"] ?? defaultDbPath;

// `push` opens the database immediately, and SQLite cannot create a database in
// a directory that does not exist — on a fresh machine `~/.local/share/omni`
// has not been created yet (the CLI creates it on first run). Without this, the
// documented setup step fails with "Cannot open database because the directory
// does not exist".
fs.mkdirSync(path.dirname(dbPath), { recursive: true });

export default defineConfig({
  schema: path.join(__dirname, "./src/schema/index.ts"),
  dialect: "sqlite",
  dbCredentials: {
    url: dbPath,
  },
});
