import { createContentLoader } from "vitepress";
import { compareVersions } from "../releases";

export type Release = {
  version: string;
  url: string;
  date: string;
  summary: string;
};

declare const data: Release[];
export { data };

function isoDate(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString().slice(0, "YYYY-MM-DD".length);
  }
  return typeof value === "string" ? value : "";
}

function firstParagraph(content: string): string {
  return (
    content
      .split("\n")
      .map((line) => line.trim())
      .find((line) => line !== "" && !line.startsWith("#")) ?? ""
  );
}

export default createContentLoader("releases/*.md", {
  excerpt: (file) => {
    file.excerpt = firstParagraph(file.content);
  },
  transform: (pages): Release[] =>
    pages
      .filter((page) => page.frontmatter.date !== undefined)
      .map((page) => ({
        version: String(page.frontmatter.title),
        url: page.url,
        date: isoDate(page.frontmatter.date),
        summary: page.excerpt ?? "",
      }))
      .sort((left, right) => compareVersions(right.version, left.version)),
});
