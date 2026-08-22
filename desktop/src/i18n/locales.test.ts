import { expect, test } from "vite-plus/test";
import i18n, { localeFromLanguage } from "./index";
import enUS from "./locales/en-US.json";
import jaJP from "./locales/ja-JP.json";
import zhCN from "./locales/zh-CN.json";

function leafKeys(value: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(value).flatMap(([key, child]) => {
    const next = prefix ? `${prefix}.${key}` : key;
    return typeof child === "object" && child !== null
      ? leafKeys(child as Record<string, unknown>, next)
      : [next];
  });
}

test("Chinese, English, and Japanese resources have identical semantic keys", () => {
  expect(leafKeys(zhCN).sort()).toEqual(leafKeys(enUS).sort());
  expect(leafKeys(jaJP).sort()).toEqual(leafKeys(enUS).sort());
});

test("translations interpolate progress and error copy in every locale", async () => {
  await i18n.changeLanguage("en-US");
  expect(i18n.t("app.progress", { completed: 2, total: 5 })).toBe("2 of 5 items");
  expect(i18n.t("errors.busy")).not.toBe("errors.busy");
  await i18n.changeLanguage("zh-CN");
  expect(i18n.t("app.progress", { completed: 2, total: 5 })).toContain("2");
  expect(i18n.t("errors.busy")).not.toBe("errors.busy");
  await i18n.changeLanguage("ja-JP");
  expect(i18n.t("app.progress", { completed: 2, total: 5 })).toBe("5 件中 2 件完了");
  expect(i18n.t("errors.busy")).not.toBe("errors.busy");
});

test("locale detection recognizes Chinese and Japanese and falls back to English", () => {
  expect(localeFromLanguage("zh-Hans-CN")).toBe("zh-CN");
  expect(localeFromLanguage("ja-JP")).toBe("ja-JP");
  expect(localeFromLanguage("ja")).toBe("ja-JP");
  expect(localeFromLanguage("fr-FR")).toBe("en-US");
});
