import { FolderOpen, Trash, X } from "@phosphor-icons/react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { appReducer, initialAppModel } from "./app/conversion-state";
import { pageTransition, pageVariants as createPageVariants } from "./app/page-motion";
import { AppHeader } from "./components/AppHeader";
import { Composer } from "./features/conversion/Composer";
import { HistoryDetail, VisibleArtwork } from "./features/history/HistoryDetail";
import type { Preferences, RunSettings, ValidatedInput } from "./lib/contracts";
import { conversionRequest } from "./lib/conversion-request";
import { errorMessageKey, normalizeDesktopError } from "./lib/errors";
import { formatTimestamp } from "./lib/format";
import { desktopApi } from "./lib/ipc";
import { persistPreferences } from "./lib/preferences";
import { applyTheme, cachedTheme } from "./lib/theme";
import "./App.css";
import { localeFromLanguage } from "./i18n";

const defaultSettings: RunSettings = {
  updateMetadata: false,
  copyPassthrough: true,
  overwrite: false,
};

function currentLocale(language: string): Preferences["locale"] {
  return localeFromLanguage(language);
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

function App() {
  const { i18n, t } = useTranslation();
  const [model, dispatch] = useReducer(appReducer, initialAppModel);
  const [selected, setSelected] = useState<ValidatedInput | null>(null);
  const [outputPath, setOutputPath] = useState<string | null>(null);
  const [settings, setSettings] = useState<RunSettings>(defaultSettings);
  const [theme, setTheme] = useState<Preferences["theme"]>(cachedTheme);
  const [locale, setLocale] = useState<Preferences["locale"]>(() => currentLocale(i18n.language));
  const [detailRunId, setDetailRunId] = useState<number | null>(null);
  const [direction, setDirection] = useState<1 | -1>(1);
  const [loadingMoreRuns, setLoadingMoreRuns] = useState(false);
  const nextRequestId = useRef(0);
  const startInFlight = useRef(false);
  const preferencesBootstrapped = useRef(false);
  const reducedMotion = useReducedMotion();

  const reportError = useCallback(
    (error: unknown) => {
      const normalized = normalizeDesktopError(error);
      dispatch({
        type: "uiError",
        message: t(errorMessageKey(normalized.code), { defaultValue: t("errors.unknown") }),
        detail: normalized.detail || null,
      });
    },
    [t],
  );

  const boot = useCallback(async () => {
    try {
      const bootstrap = await desktopApi.bootstrap();
      dispatch({ type: "booted", bootstrap });
      if (!preferencesBootstrapped.current) {
        preferencesBootstrapped.current = true;
        setSettings({ ...bootstrap.preferences.defaultSettings, overwrite: false });
        setTheme(bootstrap.preferences.theme);
        setLocale(bootstrap.preferences.locale);
        applyTheme(bootstrap.preferences.theme);
        await i18n.changeLanguage(bootstrap.preferences.locale);
      }
    } catch (error) {
      const normalized = normalizeDesktopError(error);
      dispatch({
        type: "bootFailed",
        message: i18n.t(errorMessageKey(normalized.code), {
          defaultValue: i18n.t("errors.unknown"),
        }),
        detail: normalized.detail || null,
      });
    }
  }, [i18n]);

  useEffect(() => void boot(), [boot]);

  useEffect(() => {
    if (model.active?.requestId !== null || model.active?.runId === null) return;
    const timer = window.setInterval(() => void boot(), 1_000);
    return () => window.clearInterval(timer);
  }, [boot, model.active?.requestId, model.active?.runId]);

  const persist = useCallback(
    async (
      locale: Preferences["locale"],
      nextTheme: Preferences["theme"],
      nextSettings: RunSettings,
    ) => {
      await persistPreferences(
        (preferences) => desktopApi.setPreferences(preferences),
        locale,
        nextTheme,
        nextSettings,
      );
    },
    [],
  );

  const selectPath = useCallback(
    async (path: string) => {
      try {
        const input = await desktopApi.validateInput(path);
        setSelected(input);
        setOutputPath(input.defaultOutputPath);
      } catch (error) {
        reportError(error);
      }
    },
    [reportError],
  );

  const changeLocale = useCallback(
    async (nextLocale: Preferences["locale"]) => {
      setLocale(nextLocale);
      try {
        await i18n.changeLanguage(nextLocale);
        await persist(nextLocale, theme, settings);
      } catch (error) {
        reportError(error);
      }
    },
    [i18n, persist, reportError, settings, theme],
  );

  const changeTheme = useCallback(
    (nextTheme: Preferences["theme"]) => {
      setTheme(nextTheme);
      applyTheme(nextTheme);
      void persist(locale, nextTheme, settings).catch(reportError);
    },
    [locale, persist, reportError, settings],
  );

  const convert = useCallback(() => {
    if (!selected || model.active || startInFlight.current) return;
    const requestId = ++nextRequestId.current;
    startInFlight.current = true;
    dispatch({ type: "conversionStarted", requestId });
    void persist(locale, theme, settings).catch(reportError);
    void desktopApi
      .startConversion(conversionRequest(selected, outputPath, settings), (event) =>
        dispatch({ type: "conversionEvent", event }),
      )
      .then(async (run) => {
        dispatch({ type: "conversionResolved", requestId, run });
        await boot();
      })
      .catch((error) => {
        const normalized = normalizeDesktopError(error);
        dispatch({
          type: "conversionFailed",
          requestId,
          message: t(errorMessageKey(normalized.code), { defaultValue: t("errors.unknown") }),
          detail: normalized.detail || null,
        });
      })
      .finally(() => {
        startInFlight.current = false;
      });
  }, [boot, locale, model.active, outputPath, persist, reportError, selected, settings, t, theme]);

  const cancel = useCallback(() => {
    const runId = model.active?.runId;
    if (runId !== null && runId !== undefined) void desktopApi.cancelConversion(runId);
  }, [model.active]);

  const clearHistory = useCallback(() => {
    if (!window.confirm(t("app.clearConfirm"))) return;
    void desktopApi.clearHistory().then(boot).catch(reportError);
  }, [boot, reportError, t]);

  const removeRun = useCallback(
    (runId: number) => {
      if (!window.confirm(t("app.deleteConfirm"))) return;
      void desktopApi.deleteRun(runId).then(boot).catch(reportError);
    },
    [boot, reportError, t],
  );

  const loadMoreRuns = useCallback(async () => {
    const cursor = model.bootstrap?.history.nextCursor;
    if (cursor === null || cursor === undefined || loadingMoreRuns) return;
    setLoadingMoreRuns(true);
    try {
      dispatch({ type: "runsAppended", page: await desktopApi.listRuns(cursor, 30) });
    } catch (error) {
      reportError(error);
    } finally {
      setLoadingMoreRuns(false);
    }
  }, [loadingMoreRuns, model.bootstrap?.history.nextCursor, reportError]);

  const runs = (model.bootstrap?.history.runs ?? []).filter(
    (run) => run.id !== model.active?.runId,
  );
  const detailRun = runs.find((run) => run.id === detailRunId) ?? null;
  const busy = model.active !== null;
  const transition = pageTransition(reducedMotion ?? false);
  const variants = createPageVariants(reducedMotion ?? false);

  return (
    <>
      <AppHeader locale={locale} theme={theme} onLocale={changeLocale} onTheme={changeTheme} />
      <main className="app-shell">
        {model.boot === "loading" ? <p className="boot-status">{t("app.loading")}</p> : null}
        <AnimatePresence custom={direction} mode="wait">
          {detailRun ? (
            <motion.div
              key={`detail-${detailRun.id}`}
              animate="center"
              className="view-motion"
              custom={direction}
              exit="exit"
              initial="enter"
              transition={transition}
              variants={variants}
            >
              <HistoryDetail
                layoutId={`run-card-${detailRun.id}`}
                run={detailRun}
                onBack={() => {
                  setDirection(-1);
                  setDetailRunId(null);
                }}
                onError={reportError}
                reducedMotion={reducedMotion ?? false}
              />
            </motion.div>
          ) : (
            <motion.div
              key="home"
              animate="center"
              className="home-view view-motion"
              custom={direction}
              exit="exit"
              initial="enter"
              transition={transition}
              variants={variants}
            >
              <Composer
                busy={busy}
                outputPath={outputPath}
                selected={selected}
                settings={settings}
                onConvert={convert}
                onError={(detail) => reportError(detail)}
                onOutputPath={setOutputPath}
                onSelectPath={selectPath}
                onSettings={(next) => setSettings({ ...next, overwrite: false })}
              />

              <section className="history-panel" aria-label={t("app.history")}>
                <div className="history-heading">
                  <div>
                    <h1>{t("app.history")}</h1>
                    <span>{t("app.historyCount", { count: runs.length })}</span>
                  </div>
                  <button
                    aria-label={t("app.clearHistory")}
                    className="icon-button subtle"
                    title={t("app.clearHistory")}
                    type="button"
                    disabled={busy || runs.length === 0}
                    onClick={clearHistory}
                  >
                    <Trash aria-hidden="true" size={18} />
                  </button>
                </div>

                {busy && model.active ? (
                  <section className="live-run" aria-live="polite">
                    <span className="status-mark" aria-hidden="true" />
                    <strong>{t("app.converting")}</strong>
                    <span>
                      {t("app.progress", {
                        completed: model.active.completed,
                        total: model.active.total,
                      })}
                    </span>
                    <button disabled={model.active.runId === null} type="button" onClick={cancel}>
                      {t("app.cancel")}
                    </button>
                  </section>
                ) : null}

                <div className="run-list">
                  {model.boot === "loading"
                    ? Array.from({ length: 3 }, (_, index) => (
                        <div className="run-row run-skeleton" key={`skeleton-${index}`} />
                      ))
                    : runs.map((run) => (
                        <motion.article
                          className={`run-row status-${run.status}`}
                          key={run.id}
                          layoutId={`run-card-${run.id}`}
                          transition={{ layout: transition.layout }}
                        >
                          <button
                            aria-label={t("app.openDetails", { name: basename(run.inputPath) })}
                            className="run-detail-button"
                            type="button"
                            onClick={() => {
                              setDirection(1);
                              setDetailRunId(run.id);
                            }}
                          >
                            <VisibleArtwork
                              artworkId={run.representativeArtworkId}
                              title={basename(run.inputPath)}
                            />
                            <span className="run-identity">
                              <strong>{basename(run.inputPath)}</strong>
                              <span title={run.outputPath}>{run.outputPath}</span>
                            </span>
                            <span className="run-count">
                              <strong>{run.musicCount}</strong>
                              <small>{t("app.songs")}</small>
                            </span>
                            <span className={`run-result result-${run.status}`}>
                              {t(`app.${run.status}`)}
                            </span>
                            <time className="run-date">
                              {formatTimestamp(run.updatedAtMs, i18n.language)}
                            </time>
                          </button>
                          <div className="run-actions">
                            <button
                              aria-label={t("app.revealOutput", { name: basename(run.inputPath) })}
                              className="icon-button subtle"
                              title={t("app.revealOutput", { name: basename(run.inputPath) })}
                              type="button"
                              onClick={() =>
                                void desktopApi.revealRunOutput(run.id).catch(reportError)
                              }
                            >
                              <FolderOpen aria-hidden="true" size={18} />
                            </button>
                            <button
                              aria-label={t("app.delete")}
                              className="icon-button subtle danger"
                              title={t("app.delete")}
                              type="button"
                              onClick={() => removeRun(run.id)}
                            >
                              <Trash aria-hidden="true" size={18} />
                            </button>
                          </div>
                        </motion.article>
                      ))}
                  {runs.length === 0 && !busy && model.boot !== "loading" ? (
                    <div className="empty-history">
                      <span>{t("app.emptyHistory")}</span>
                    </div>
                  ) : null}
                  {model.bootstrap?.history.nextCursor !== null &&
                  model.bootstrap?.history.nextCursor !== undefined ? (
                    <button
                      className="load-more"
                      disabled={loadingMoreRuns}
                      type="button"
                      onClick={() => void loadMoreRuns()}
                    >
                      {t("app.loadMore")}
                    </button>
                  ) : null}
                  {loadingMoreRuns ? (
                    <div aria-hidden="true" className="run-row run-skeleton pagination-skeleton" />
                  ) : null}
                </div>
              </section>
            </motion.div>
          )}
        </AnimatePresence>
      </main>

      {model.error ? (
        <section className="error-banner" role="alert">
          <p>{model.error.message}</p>
          {model.error.detail ? (
            <details>
              <summary>{t("app.diagnostics")}</summary>
              <code>{model.error.detail}</code>
            </details>
          ) : null}
          <button
            className="icon-button"
            type="button"
            aria-label={t("app.dismiss")}
            onClick={() => dispatch({ type: "clearError" })}
          >
            <X aria-hidden="true" size={17} />
          </button>
        </section>
      ) : null}
    </>
  );
}

export default App;
