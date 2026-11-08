import { resolve } from "node:path";

const landing = resolve(import.meta.dir, "..");
const docs = resolve(landing, "../docs");
const outDir = resolve(landing, "public/docs");

async function run(command: string[]) {
  const process = Bun.spawn(command, {
    cwd: docs,
    env: { ...Bun.env, DOCS_BASE: "/docs/", DOCS_OUT_DIR: outDir },
    stdout: "inherit",
    stderr: "inherit",
  });
  const code = await process.exited;
  if (code !== 0) {
    throw new Error(`${command.join(" ")} exited with ${code}`);
  }
}

await run(["pnpm", "install", "--frozen-lockfile"]);
await run(["pnpm", "build"]);
