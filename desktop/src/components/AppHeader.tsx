import { Check, GlobeHemisphereWest, Moon, Sun } from "@phosphor-icons/react";
import {
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import unlockmIcon from "../../src-tauri/icons/source/UnlockM Icon.png";
import { supportedLocales } from "../i18n";
import type { Preferences } from "../lib/contracts";

type AppHeaderProps = {
  locale: Preferences["locale"];
  theme: Preferences["theme"];
  onLocale: (locale: Preferences["locale"]) => void;
  onTheme: (theme: Preferences["theme"]) => void;
};

export function AppHeader({ locale, theme, onLocale, onTheme }: AppHeaderProps) {
  const { t } = useTranslation();
  const [languageOpen, setLanguageOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const closeLanguageMenu = useCallback((restoreFocus = false) => {
    setLanguageOpen(false);
    if (restoreFocus) window.requestAnimationFrame(() => triggerRef.current?.focus());
  }, []);

  useEffect(() => {
    if (!languageOpen) return;
    const close = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) setLanguageOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeLanguageMenu(true);
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", escape);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", escape);
    };
  }, [closeLanguageMenu, languageOpen]);

  useEffect(() => {
    if (!languageOpen) return;
    const selected = supportedLocales.indexOf(locale);
    window.requestAnimationFrame(() => optionRefs.current[selected]?.focus());
  }, [languageOpen, locale]);

  const moveLanguageFocus = (event: ReactKeyboardEvent, index: number) => {
    const last = supportedLocales.length - 1;
    let next = index;
    if (event.key === "ArrowDown") next = index === last ? 0 : index + 1;
    else if (event.key === "ArrowUp") next = index === 0 ? last : index - 1;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = last;
    else return;
    event.preventDefault();
    optionRefs.current[next]?.focus();
  };

  return (
    <header className="topbar">
      <div className="topbar-inner">
        <div className="brand">
          <img src={unlockmIcon} alt="" />
          <span className="brand-product">UnlockM</span>
          <span className="brand-divider" aria-hidden="true" />
          <span className="brand-localized">{t("app.localName", { lng: locale })}</span>
        </div>
        <div className="topbar-actions">
          <div className="language-control" ref={menuRef}>
            <button
              ref={triggerRef}
              aria-expanded={languageOpen}
              aria-haspopup="menu"
              aria-label={t("app.chooseLanguage")}
              className="icon-button"
              title={t("app.chooseLanguage")}
              type="button"
              onClick={() => setLanguageOpen((open) => !open)}
            >
              <GlobeHemisphereWest aria-hidden="true" size={19} weight="regular" />
            </button>
            {languageOpen ? (
              <div className="language-menu" role="menu" aria-label={t("app.chooseLanguage")}>
                {supportedLocales.map((value, index) => (
                  <button
                    ref={(element) => {
                      optionRefs.current[index] = element;
                    }}
                    className="language-option"
                    key={value}
                    role="menuitemradio"
                    aria-checked={locale === value}
                    type="button"
                    tabIndex={locale === value ? 0 : -1}
                    onKeyDown={(event) => moveLanguageFocus(event, index)}
                    onClick={() => {
                      onLocale(value);
                      closeLanguageMenu(true);
                    }}
                  >
                    <span>{t(`app.languages.${value}`)}</span>
                    {locale === value ? <Check aria-hidden="true" size={16} weight="bold" /> : null}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
          <button
            aria-label={theme === "light" ? t("app.useDarkTheme") : t("app.useLightTheme")}
            className="icon-button"
            title={theme === "light" ? t("app.useDarkTheme") : t("app.useLightTheme")}
            type="button"
            onClick={() => onTheme(theme === "light" ? "dark" : "light")}
          >
            {theme === "light" ? (
              <Moon aria-hidden="true" size={19} weight="regular" />
            ) : (
              <Sun aria-hidden="true" size={19} weight="regular" />
            )}
          </button>
        </div>
      </div>
    </header>
  );
}
