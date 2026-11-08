import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "node:path";

import { appVersion } from "./scripts/app-version";

export default defineConfig({
  plugins: [react()],
  define: { KIEKJE_APP_VERSION: JSON.stringify(appVersion()) },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
  },
  resolve: {
    alias: { "@": path.resolve(__dirname, "./src") },
  },
});
