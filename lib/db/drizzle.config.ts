import { defineConfig } from "drizzle-kit";
import path from "path";
import os from "os";

const defaultDbPath = path.join(
  process.env["XDG_DATA_HOME"] ?? path.join(os.homedir(), ".local", "share"),
  "omni",
  "omni.db"
);

export default defineConfig({
  schema: path.join(__dirname, "./src/schema/index.ts"),
  dialect: "sqlite",
  dbCredentials: {
    url: process.env["DB_PATH"] ?? defaultDbPath,
  },
});
