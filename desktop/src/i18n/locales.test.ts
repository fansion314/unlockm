import { expect, test } from "vite-plus/test";
import i18n from "./index";
import enUS from "./locales/en-US.json";
import zhCN from "./locales/zh-CN.json";

function leafKeys(value: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(value).flatMap(([key, child]) => {
    const next = prefix ? `${prefix}.${key}` : key;
    return typeof child === "object" && child !== null
      ? leafKeys(child as Record<string, unknown>, next)
      : [next];
  });
}

test("Chinese and English resources have identical semantic keys", () => {
  expect(leafKeys(zhCN).sort()).toEqual(leafKeys(enUS).sort());
});

test("translations interpolate progress and error copy in both locales", async () => {
  await i18n.changeLanguage("en-US");
  expect(i18n.t("app.progress", { completed: 2, total: 5 })).toBe("2 of 5 items");
  expect(i18n.t("errors.busy")).not.toBe("errors.busy");
  await i18n.changeLanguage("zh-CN");
  expect(i18n.t("app.progress", { completed: 2, total: 5 })).toContain("2");
  expect(i18n.t("errors.busy")).not.toBe("errors.busy");
});
