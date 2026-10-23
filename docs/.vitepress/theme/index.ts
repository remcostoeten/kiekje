import type { Theme } from "vitepress";
import DefaultTheme from "vitepress/theme";
import ReleaseList from "./ReleaseList.vue";
import "./style.css";

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component("ReleaseList", ReleaseList);
  },
} satisfies Theme;
