import { Check, GlobeHemisphereWest, Palette } from "@phosphor-icons/react";
import {
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
  type MutableRefObject,
} from "react";
import { useTranslation } from "react-i18next";
import unlockmIcon from "../../src-tauri/icons/source/UnlockM Icon.png";
import { supportedLocales } from "../i18n";
import type { Preferences } from "../lib/contracts";
import { themePresets, type ThemeId } from "../lib/theme-presets";

type AppHeaderProps = {
  locale: Preferences["locale"];
  themeId: ThemeId;
  onLocale: (locale: Preferences["locale"]) => void;
  onThemeId: (themeId: ThemeId) => void;
};

export function AppHeader({ locale, themeId, onLocale, onThemeId }: AppHeaderProps) {
  const { t } = useTranslation();
  const [languageOpen, setLanguageOpen] = useState(false);
  const [themeOpen, setThemeOpen] = useState(false);
  const languageMenuRef = useRef<HTMLDivElement>(null);
  const languageTriggerRef = useRef<HTMLButtonElement>(null);
  const languageOptionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const themeMenuRef = useRef<HTMLDivElement>(null);
  const themeTriggerRef = useRef<HTMLButtonElement>(null);
  const themeOptionRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const closeLanguageMenu = useCallback((restoreFocus = false) => {
    setLanguageOpen(false);
    if (restoreFocus) window.requestAnimationFrame(() => languageTriggerRef.current?.focus());
  }, []);
  const closeThemeMenu = useCallback((restoreFocus = false) => {
    setThemeOpen(false);
    if (restoreFocus) window.requestAnimationFrame(() => themeTriggerRef.current?.focus());
  }, []);

  useEffect(() => {
    if (!languageOpen && !themeOpen) return;
    const close = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!languageMenuRef.current?.contains(target) && !themeMenuRef.current?.contains(target)) {
        setLanguageOpen(false);
        setThemeOpen(false);
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (languageOpen) closeLanguageMenu(true);
      if (themeOpen) closeThemeMenu(true);
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", escape);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", escape);
    };
  }, [closeLanguageMenu, closeThemeMenu, languageOpen, themeOpen]);

  useEffect(() => {
    if (!languageOpen) return;
    const selected = supportedLocales.indexOf(locale);
    window.requestAnimationFrame(() => languageOptionRefs.current[selected]?.focus());
  }, [languageOpen, locale]);

  useEffect(() => {
    if (!themeOpen) return;
    const selected = themePresets.findIndex((preset) => preset.id === themeId);
    window.requestAnimationFrame(() => themeOptionRefs.current[selected]?.focus());
  }, [themeId, themeOpen]);

  const moveMenuFocus = (
    event: ReactKeyboardEvent,
    index: number,
    length: number,
    refs: MutableRefObject<Array<HTMLButtonElement | null>>,
  ) => {
    const last = length - 1;
    let next = index;
    if (event.key === "ArrowDown") next = index === last ? 0 : index + 1;
    else if (event.key === "ArrowUp") next = index === 0 ? last : index - 1;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = last;
    else return;
    event.preventDefault();
    refs.current[next]?.focus();
  };

  const moveThemeFocus = (event: ReactKeyboardEvent, index: number) => {
    const length = themePresets.length;
    const columns =
      typeof window.matchMedia === "function" && window.matchMedia("(max-width: 520px)").matches
        ? 1
        : 2;
    const column = index % columns;
    const lastInColumn = (() => {
      let candidate = length - 1;
      while (candidate % columns !== column) candidate -= 1;
      return candidate;
    })();
    let next = index;
    if (event.key === "ArrowRight") next = (index + 1) % length;
    else if (event.key === "ArrowLeft") next = (index - 1 + length) % length;
    else if (event.key === "ArrowDown") next = index + columns < length ? index + columns : column;
    else if (event.key === "ArrowUp") next = index - columns >= 0 ? index - columns : lastInColumn;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = length - 1;
    else return;
    event.preventDefault();
    themeOptionRefs.current[next]?.focus();
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
          <div className="language-control" ref={languageMenuRef}>
            <button
              ref={languageTriggerRef}
              aria-expanded={languageOpen}
              aria-haspopup="menu"
              aria-label={t("app.chooseLanguage")}
              className="icon-button"
              title={t("app.chooseLanguage")}
              type="button"
              onClick={() => {
                setThemeOpen(false);
                setLanguageOpen((open) => !open);
              }}
            >
              <GlobeHemisphereWest aria-hidden="true" size={19} weight="regular" />
            </button>
            {languageOpen ? (
              <div className="language-menu" role="menu" aria-label={t("app.chooseLanguage")}>
                {supportedLocales.map((value, index) => (
                  <button
                    ref={(element) => {
                      languageOptionRefs.current[index] = element;
                    }}
                    className="language-option"
                    key={value}
                    role="menuitemradio"
                    aria-checked={locale === value}
                    type="button"
                    tabIndex={locale === value ? 0 : -1}
                    onKeyDown={(event) =>
                      moveMenuFocus(event, index, supportedLocales.length, languageOptionRefs)
                    }
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

          <div className="theme-control" ref={themeMenuRef}>
            <button
              ref={themeTriggerRef}
              aria-expanded={themeOpen}
              aria-haspopup="menu"
              aria-label={t("app.chooseTheme")}
              className="icon-button"
              title={t("app.chooseTheme")}
              type="button"
              onClick={() => {
                setLanguageOpen(false);
                setThemeOpen((open) => !open);
              }}
            >
              <Palette aria-hidden="true" size={19} weight="duotone" />
            </button>
            {themeOpen ? (
              <div className="theme-menu" role="menu" aria-label={t("app.chooseTheme")}>
                {themePresets.map((preset, index) => (
                  <button
                    ref={(element) => {
                      themeOptionRefs.current[index] = element;
                    }}
                    className="theme-option"
                    key={preset.id}
                    role="menuitemradio"
                    aria-checked={themeId === preset.id}
                    type="button"
                    tabIndex={themeId === preset.id ? 0 : -1}
                    onKeyDown={(event) => moveThemeFocus(event, index)}
                    onClick={() => {
                      onThemeId(preset.id);
                      closeThemeMenu(true);
                    }}
                  >
                    <span className="theme-preview" aria-hidden="true">
                      {preset.preview.map((color, colorIndex) => (
                        <span
                          key={`${preset.id}-${colorIndex}`}
                          style={{ backgroundColor: color }}
                        />
                      ))}
                    </span>
                    <span className="theme-option-copy">
                      <strong>{t(preset.labelKey)}</strong>
                      <small>{t(preset.descriptionKey)}</small>
                    </span>
                    <span className="theme-scheme">{t(`app.themeScheme.${preset.scheme}`)}</span>
                    <span
                      className="theme-check-slot"
                      aria-hidden="true"
                      data-selected={themeId === preset.id}
                    >
                      <Check size={16} weight="bold" />
                    </span>
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        </div>
      </div>
    </header>
  );
}
