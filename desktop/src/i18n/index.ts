import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import enUS from "./locales/en-US.json";
import jaJP from "./locales/ja-JP.json";
import zhCN from "./locales/zh-CN.json";

export const supportedLocales = ["zh-CN", "en-US", "ja-JP"] as const;
export type SupportedLocale = (typeof supportedLocales)[number];

export function localeFromLanguage(language: string): SupportedLocale {
  const normalized = language.toLowerCase();
  if (normalized.startsWith("zh")) return "zh-CN";
  if (normalized.startsWith("ja")) return "ja-JP";
  return "en-US";
}

export function detectedLocale(): SupportedLocale {
  return localeFromLanguage(navigator.language);
}

void i18n.use(initReactI18next).init({
  resources: {
    "en-US": { translation: enUS },
    "ja-JP": { translation: jaJP },
    "zh-CN": { translation: zhCN },
  },
  lng: detectedLocale(),
  fallbackLng: "en-US",
  interpolation: { escapeValue: false },
});

export default i18n;
