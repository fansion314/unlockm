import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/bricolage-grotesque";
import "@fontsource-variable/manrope";
import "./i18n";
import "./index.css";
import App from "./App.tsx";
import { applyTheme, cachedTheme } from "./lib/theme";

applyTheme(cachedTheme());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
