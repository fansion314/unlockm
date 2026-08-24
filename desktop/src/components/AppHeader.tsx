import { Check, GlobeHemisphereWest, Moon, Palette, Sun } from "@phosphor-icons/react";
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

type AppHeaderProps = {
  locale: Preferences["locale"];
  theme: Preferences["theme"];
  palette: NonNullable<Preferences["palette"]>;
  onLocale: (locale: Preferences["locale"]) => void;
  onTheme: (theme: Preferences["theme"]) => void;
  onPalette: (palette: NonNullable<Preferences["palette"]>) => void;
};

const palettes = ["blue", "emerald", "violet", "amber"] as const;

export function AppHeader({
  locale,
  theme,
  palette,
  onLocale,
  onTheme,
  onPalette,
}: AppHeaderProps) {
  const { t } = useTranslation();
  const [languageOpen, setLanguageOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const languageMenuRef = useRef<HTMLDivElement>(null);
  const languageTriggerRef = useRef<HTMLButtonElement>(null);
  const languageOptionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const paletteMenuRef = useRef<HTMLDivElement>(null);
  const paletteTriggerRef = useRef<HTMLButtonElement>(null);
  const paletteOptionRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const closeLanguageMenu = useCallback((restoreFocus = false) => {
    setLanguageOpen(false);
    if (restoreFocus) window.requestAnimationFrame(() => languageTriggerRef.current?.focus());
  }, []);
  const closePaletteMenu = useCallback((restoreFocus = false) => {
    setPaletteOpen(false);
    if (restoreFocus) window.requestAnimationFrame(() => paletteTriggerRef.current?.focus());
  }, []);

  useEffect(() => {
    if (!languageOpen && !paletteOpen) return;
    const close = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!languageMenuRef.current?.contains(target) && !paletteMenuRef.current?.contains(target)) {
        setLanguageOpen(false);
        setPaletteOpen(false);
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (languageOpen) closeLanguageMenu(true);
      if (paletteOpen) closePaletteMenu(true);
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", escape);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", escape);
    };
  }, [closeLanguageMenu, closePaletteMenu, languageOpen, paletteOpen]);

  useEffect(() => {
    if (!languageOpen) return;
    const selected = supportedLocales.indexOf(locale);
    window.requestAnimationFrame(() => languageOptionRefs.current[selected]?.focus());
  }, [languageOpen, locale]);
  useEffect(() => {
    if (!paletteOpen) return;
    const selected = palettes.indexOf(palette);
    window.requestAnimationFrame(() => paletteOptionRefs.current[selected]?.focus());
  }, [palette, paletteOpen]);

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
                setPaletteOpen(false);
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
          <div className="palette-control" ref={paletteMenuRef}>
            <button
              ref={paletteTriggerRef}
              aria-expanded={paletteOpen}
              aria-haspopup="menu"
              aria-label={t("app.choosePalette")}
              className="icon-button"
              title={t("app.choosePalette")}
              type="button"
              onClick={() => {
                setLanguageOpen(false);
                setPaletteOpen((open) => !open);
              }}
            >
              <Palette aria-hidden="true" size={19} />
            </button>
            {paletteOpen ? (
              <div
                className="language-menu palette-menu"
                role="menu"
                aria-label={t("app.choosePalette")}
              >
                {palettes.map((value, index) => (
                  <button
                    ref={(element) => {
                      paletteOptionRefs.current[index] = element;
                    }}
                    className="language-option palette-option"
                    key={value}
                    role="menuitemradio"
                    aria-checked={palette === value}
                    type="button"
                    tabIndex={palette === value ? 0 : -1}
                    onKeyDown={(event) =>
                      moveMenuFocus(event, index, palettes.length, paletteOptionRefs)
                    }
                    onClick={() => {
                      onPalette(value);
                      closePaletteMenu(true);
                    }}
                  >
                    <span className={`palette-swatch ${value}`} aria-hidden="true" />
                    <span>
                      {t(`app.palette${value.slice(0, 1).toUpperCase()}${value.slice(1)}`)}
                    </span>
                    <span
                      className="palette-check-slot"
                      aria-hidden="true"
                      data-selected={palette === value}
                    >
                      <Check size={16} weight="bold" />
                    </span>
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
