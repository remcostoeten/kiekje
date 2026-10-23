import { defineConfig } from "vitepress";
import { crateVersion, releaseVersions } from "./releases";

const repo = "https://github.com/remcostoeten/kiekje";
const version = crateVersion();

export default defineConfig({
  lang: "en",
  title: "Kiekje",
  titleTemplate: ":title | Kiekje",
  description:
    "A screenshot tray app for Linux. It freezes the screen when you press the shortcut, then you pick what to keep.",
  base: "/kiekje/",
  srcExclude: ["branding/**"],
  cleanUrls: true,
  lastUpdated: true,
  appearance: "force-dark",
  markdown: {
    config(md) {
      md.renderer.rules.table_open = () => '<div class="table-wrap"><table>';
      md.renderer.rules.table_close = () => "</table></div>";
    },
  },
  head: [
    ["link", { rel: "icon", type: "image/png", href: "/kiekje/favicon.png" }],
    ["meta", { name: "theme-color", content: "#0a0a0a" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:title", content: "Kiekje" }],
    ["meta", { property: "og:site_name", content: "Kiekje" }],
    [
      "meta",
      {
        property: "og:image",
        content: "https://remcostoeten.github.io/kiekje/editor-and-gallery.png",
      },
    ],
  ],
  themeConfig: {
    logo: { src: "/kiekje-icon.png", alt: "Kiekje" },
    siteTitle: "Kiekje",
    nav: [
      { text: "Guide", link: "/guide/introduction", activeMatch: "^/guide/" },
      { text: "Reference", link: "/reference/cli", activeMatch: "^/reference/" },
      { text: "Project", link: "/project/architecture", activeMatch: "^/project/" },
      { text: "Releases", link: "/releases/", activeMatch: "^/releases/" },
      {
        text: `v${version}`,
        items: [
          { text: "Release notes", link: `/releases/${version}` },
          { text: "Download", link: `${repo}/releases/latest` },
          { text: "Issues", link: `${repo}/issues` },
        ],
      },
    ],
    sidebar: {
      "/guide/": [
        {
          text: "Start",
          items: [
            { text: "Introduction", link: "/guide/introduction" },
            { text: "Install", link: "/guide/install" },
            { text: "Getting started", link: "/guide/getting-started" },
          ],
        },
        {
          text: "Using Kiekje",
          items: [
            { text: "Capture", link: "/guide/capture" },
            { text: "Annotate", link: "/guide/annotate" },
            { text: "Gallery", link: "/guide/gallery" },
            { text: "Record and share", link: "/guide/record-and-share" },
            { text: "Desktops", link: "/guide/desktops" },
          ],
        },
        {
          text: "Help",
          items: [
            { text: "Troubleshooting", link: "/guide/troubleshooting" },
            { text: "Privacy", link: "/guide/privacy" },
          ],
        },
      ],
      "/reference/": [
        {
          text: "Reference",
          items: [
            { text: "Command line", link: "/reference/cli" },
            { text: "Configuration", link: "/reference/configuration" },
            { text: "Keys", link: "/reference/keys" },
            { text: "Support matrix", link: "/reference/support" },
          ],
        },
      ],
      "/project/": [
        {
          text: "Project",
          items: [
            { text: "Architecture", link: "/project/architecture" },
            { text: "Design system", link: "/project/design" },
            { text: "Contributing", link: "/project/contributing" },
            { text: "Releasing", link: "/project/releasing" },
          ],
        },
        {
          text: "Records",
          items: [
            { text: "Rewrite proposal", link: "/project/proposal" },
            { text: "Lab results", link: "/project/lab-results" },
          ],
        },
      ],
      "/releases/": [
        {
          text: "Releases",
          items: [
            { text: "All releases", link: "/releases/" },
            ...releaseVersions().map((release) => ({
              text: release,
              link: `/releases/${release}`,
            })),
          ],
        },
      ],
    },
    socialLinks: [{ icon: "github", link: repo }],
    search: { provider: "local" },
    editLink: {
      pattern: `${repo}/edit/master/docs/:path`,
      text: "Edit this page",
    },
    outline: { level: [2, 3], label: "On this page" },
    docFooter: { prev: "Previous", next: "Next" },
    lastUpdated: { text: "Updated" },
    footer: {
      message: "MIT licensed. The Geist fonts are under the SIL Open Font License.",
      copyright: "Remco Stoeten",
    },
  },
});
