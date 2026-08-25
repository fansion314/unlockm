import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/bricolage-grotesque";
import "@fontsource-variable/manrope";
import "./i18n";
import "./index.css";
import App from "./App.tsx";
import { applyTheme, cachedThemeId } from "./lib/theme";
import { listenDisplayScale } from "./lib/display-scale";

applyTheme(cachedThemeId());
void listenDisplayScale();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
