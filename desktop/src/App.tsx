import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import unlockmIcon from "../src-tauri/icons/source/UnlockM Icon.png";
import { Composer } from "./features/conversion/Composer";
import { HistoryDetail, VisibleArtwork } from "./features/history/HistoryDetail";
import { appReducer, initialAppModel } from "./app/conversion-state";
import { pageTransition, pageVariants as createPageVariants } from "./app/page-motion";
import type { RunSettings, ValidatedInput } from "./lib/contracts";
import { desktopApi } from "./lib/ipc";
import { conversionRequest } from "./lib/conversion-request";
import { errorMessageKey, normalizeDesktopError } from "./lib/errors";
import { formatTimestamp } from "./lib/format";
import { persistPreferences } from "./lib/preferences";
import "./App.css";
import "./i18n";

const defaultSettings: RunSettings = {
  updateMetadata: false,
  copyPassthrough: true,
  overwrite: false,
};

function App() {
  const { i18n, t } = useTranslation();
  const [model, dispatch] = useReducer(appReducer, initialAppModel);
  const [selected, setSelected] = useState<ValidatedInput | null>(null);
  const [outputPath, setOutputPath] = useState<string | null>(null);
  const [settings, setSettings] = useState<RunSettings>(defaultSettings);
  const [detailRunId, setDetailRunId] = useState<number | null>(null);
  const [direction, setDirection] = useState<1 | -1>(1);
  const [loadingMoreRuns, setLoadingMoreRuns] = useState(false);
  const nextRequestId = useRef(0);
  const startInFlight = useRef(false);
  const alternateLocale = i18n.language === "zh-CN" ? "en-US" : "zh-CN";
  const reducedMotion = useReducedMotion();
  const reportError = useCallback(
    (error: unknown) => {
      const normalized = normalizeDesktopError(error);
      const message = t(errorMessageKey(normalized.code), { defaultValue: t("errors.unknown") });
      dispatch({
        type: "uiError",
        message,
        detail: normalized.detail || null,
      });
    },
    [t],
  );

  const boot = useCallback(async () => {
    try {
      const bootstrap = await desktopApi.bootstrap();
      dispatch({ type: "booted", bootstrap });
      setSettings(bootstrap.preferences.defaultSettings);
      await i18n.changeLanguage(bootstrap.preferences.locale);
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

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    // A webview reload loses its old IPC Channel. For a recovered active run,
    // poll the persisted bootstrap state until the Rust coordinator has made
    // it terminal, then refresh history and clear the live row.
    if (model.active?.requestId !== null || model.active?.runId === null) {
      return;
    }
    const timer = window.setInterval(() => void boot(), 1_000);
    return () => window.clearInterval(timer);
  }, [boot, model.active?.requestId, model.active?.runId]);

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

  const selectError = useCallback(
    (detail: string) => {
      reportError(detail);
    },
    [reportError],
  );

  const changeLocale = useCallback(async () => {
    const locale = alternateLocale;
    await i18n.changeLanguage(locale);
    try {
      await persistPreferences(
        (preferences) => desktopApi.setPreferences(preferences),
        locale,
        settings,
      );
    } catch (error) {
      reportError(error);
    }
  }, [alternateLocale, i18n, reportError, settings]);

  const convert = useCallback(() => {
    if (!selected || model.active || startInFlight.current) {
      return;
    }
    const requestId = ++nextRequestId.current;
    startInFlight.current = true;
    dispatch({ type: "conversionStarted", requestId });
    void persistPreferences(
      (preferences) => desktopApi.setPreferences(preferences),
      i18n.language === "zh-CN" ? "zh-CN" : "en-US",
      settings,
    ).catch(reportError);
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
  }, [boot, i18n.language, model.active, outputPath, reportError, selected, settings, t]);

  const cancel = useCallback(() => {
    const runId = model.active?.runId;
    if (runId !== null && runId !== undefined) {
      void desktopApi.cancelConversion(runId);
    }
  }, [model.active]);

  const clearHistory = useCallback(() => {
    if (!window.confirm(t("app.clearConfirm"))) {
      return;
    }
    void desktopApi.clearHistory().then(boot).catch(reportError);
  }, [boot, reportError, t]);

  const removeRun = useCallback(
    (runId: number) => {
      if (!window.confirm(t("app.deleteConfirm"))) {
        return;
      }
      void desktopApi.deleteRun(runId).then(boot).catch(reportError);
    },
    [boot, reportError, t],
  );

  const loadMoreRuns = useCallback(async () => {
    const cursor = model.bootstrap?.history.nextCursor;
    if (cursor === null || cursor === undefined || loadingMoreRuns) {
      return;
    }
    setLoadingMoreRuns(true);
    try {
      const page = await desktopApi.listRuns(cursor, 30);
      dispatch({ type: "runsAppended", page });
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
  const pageTransitionVariants = createPageVariants(reducedMotion ?? false);
  const transition = pageTransition(reducedMotion ?? false);

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand">
          <img src={unlockmIcon} alt="UnlockM" />
          <span>UnlockM</span>
        </div>
        <button className="locale-switch" type="button" onClick={() => void changeLocale()}>
          {t("app.language")}
        </button>
      </header>
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
            variants={pageTransitionVariants}
          >
            <HistoryDetail
              layoutId={`run-card-${detailRun.id}`}
              run={detailRun}
              onBack={() => {
                setDirection(-1);
                setDetailRunId(null);
              }}
              reducedMotion={reducedMotion ?? false}
            />
          </motion.div>
        ) : (
          <motion.div
            key="home"
            animate="center"
            className="app-grid view-motion"
            custom={direction}
            exit="exit"
            initial="enter"
            transition={transition}
            variants={pageTransitionVariants}
          >
            <Composer
              busy={busy}
              outputPath={outputPath}
              selected={selected}
              settings={settings}
              onConvert={convert}
              onError={selectError}
              onOutputPath={setOutputPath}
              onSelectPath={selectPath}
              onSettings={setSettings}
            />
            <aside className="history-panel" aria-label={t("app.history")}>
              <div className="history-heading">
                <div>
                  <p className="eyebrow">{t("app.history")}</p>
                  <h2>{runs.length}</h2>
                </div>
                <button type="button" disabled={busy || runs.length === 0} onClick={clearHistory}>
                  {t("app.clearHistory")}
                </button>
              </div>
              {busy && model.active ? (
                <section className="live-run" aria-live="polite">
                  <span className="status-dot" aria-hidden="true" />
                  <strong>
                    {t("app.progress", {
                      completed: model.active.completed,
                      total: model.active.total,
                    })}
                  </strong>
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
                          aria-label={t("app.openDetails", {
                            name: run.inputPath.split(/[\\/]/).pop(),
                          })}
                          className="run-detail-button"
                          type="button"
                          onClick={() => {
                            setDirection(1);
                            setDetailRunId(run.id);
                          }}
                        >
                          <VisibleArtwork
                            artworkId={run.representativeArtworkId}
                            title={run.inputPath.split(/[\\/]/).pop() ?? null}
                          />
                          <span className="run-copy">
                            <strong>{run.inputPath.split(/[\\/]/).pop()}</strong>
                            <span>{t(`app.${run.status}`)}</span>
                          </span>
                          <small>
                            {run.completedItems}/{run.totalItems}
                          </small>
                          <time className="run-date">
                            {formatTimestamp(run.updatedAtMs, i18n.language)}
                          </time>
                        </button>
                        <button
                          aria-label={t("app.delete")}
                          className="run-delete"
                          type="button"
                          onClick={() => removeRun(run.id)}
                        >
                          ×
                        </button>
                      </motion.article>
                    ))}
                {runs.length === 0 && !busy ? (
                  <p className="empty-history">{t("app.emptyHistory")}</p>
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
                  <div
                    aria-hidden="true"
                    className="run-row run-skeleton pagination-skeleton history-pagination-skeleton"
                  />
                ) : null}
              </div>
            </aside>
          </motion.div>
        )}
      </AnimatePresence>
      {model.error ? (
        <section className="error-banner" role="alert">
          <p>{model.error.message}</p>
          {model.error.detail ? (
            <details>
              <summary>{t("app.diagnostics")}</summary>
              <code>{model.error.detail}</code>
            </details>
          ) : null}
        </section>
      ) : null}
    </main>
  );
}

export default App;
