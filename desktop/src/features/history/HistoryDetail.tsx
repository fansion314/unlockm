import { useVirtualizer } from "@tanstack/react-virtual";
import { motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { HistoryItem, RunSummary } from "../../lib/contracts";
import { desktopApi } from "../../lib/ipc";
import { formatDuration } from "../../lib/format";
import { errorMessageKey } from "../../lib/errors";
import { pageTransition } from "../../app/page-motion";

type HistoryDetailProps = {
  layoutId: string;
  reducedMotion?: boolean;
  run: RunSummary;
  onBack: (this: void) => void;
};

export function Artwork({
  artworkId,
  title,
  visible = true,
}: {
  artworkId: number | null;
  title: string | null;
  visible?: boolean;
}) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (artworkId === null || !visible) {
      return;
    }
    let active = true;
    let objectUrl: string | null = null;
    void Promise.all([desktopApi.getArtworkMime(artworkId), desktopApi.getArtwork(artworkId)])
      .then(([metadata, bytes]) => {
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: metadata.mimeType }));
        if (active) {
          setUrl(objectUrl);
        } else {
          URL.revokeObjectURL(objectUrl);
        }
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
      }
    };
  }, [artworkId, visible]);
  return url ? (
    <img alt="" className="cover" src={url} />
  ) : (
    <span className="cover cover-placeholder">{title?.slice(0, 1) ?? "♪"}</span>
  );
}

export function VisibleArtwork({
  artworkId,
  title,
}: {
  artworkId: number | null;
  title: string | null;
}) {
  const target = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === "undefined");
  useEffect(() => {
    if (typeof IntersectionObserver === "undefined") {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setVisible(true);
          observer.disconnect();
        }
      },
      {
        rootMargin: "160px",
      },
    );
    if (target.current) observer.observe(target.current);
    return () => observer.disconnect();
  }, []);
  return (
    <span ref={target}>
      <Artwork artworkId={artworkId} title={title} visible={visible} />
    </span>
  );
}

export function HistoryDetail({
  layoutId,
  reducedMotion = false,
  run,
  onBack,
}: HistoryDetailProps) {
  const { i18n, t } = useTranslation();
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [cursor, setCursor] = useState<number | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [filter, setFilter] = useState("all");
  const [companionCount, setCompanionCount] = useState(0);
  const [musicTotal, setMusicTotal] = useState(0);
  const scrollerRef = useRef<HTMLDivElement>(null);

  const load = useCallback(
    async (nextCursor: number | null, replace = false) => {
      setLoading(true);
      setLoadError(null);
      try {
        const page = await desktopApi.getRunItems(run.id, nextCursor, 100);
        setItems((current) => (replace ? page.items : [...current, ...page.items]));
        setCursor(page.nextCursor);
        setHasMore(page.nextCursor !== null);
        setCompanionCount(page.companionCount);
        setMusicTotal(page.musicTotal);
      } catch (error) {
        setLoadError(String(error));
      } finally {
        setLoading(false);
      }
    },
    [run.id],
  );

  useEffect(() => {
    void load(null, true);
  }, [load]);

  const visibleItems = filter === "all" ? items : items.filter((item) => item.outcome === filter);
  const virtualizer = useVirtualizer({
    count: visibleItems.length,
    getScrollElement: () => scrollerRef.current,
    estimateSize: () => 96,
    overscan: 8,
  });

  return (
    <section className="detail-view" aria-labelledby="detail-title">
      <button className="back-button" type="button" onClick={onBack}>
        ← {t("app.back")}
      </button>
      <motion.header
        className="detail-header"
        layoutId={layoutId}
        transition={{ layout: pageTransition(reducedMotion).layout }}
      >
        <div>
          <p className="eyebrow">{t("app.details")}</p>
          <h1 id="detail-title">{run.inputPath.split(/[\\/]/).pop()}</h1>
          <p>
            {run.completedItems}/{run.totalItems} · {t(`app.${run.status}`)}
          </p>
        </div>
        <div className="detail-count">
          <strong>{musicTotal}</strong>
          <span>{t("app.items")}</span>
        </div>
      </motion.header>
      <div className="detail-controls">
        <select
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          aria-label={t("app.diagnostics")}
        >
          <option value="all">{t("app.items")}</option>
          <option value="converted">{t("app.completed")}</option>
          <option value="failed">{t("app.failed")}</option>
          <option value="conflict">{t("app.partial")}</option>
          <option value="cancelled">{t("app.cancelled")}</option>
        </select>
        {companionCount > 0 ? <span>{t("app.companions", { count: companionCount })}</span> : null}
      </div>
      <div className="detail-scroller" ref={scrollerRef}>
        {loading && items.length === 0 ? (
          <div className="detail-skeletons">
            {Array.from({ length: 5 }, (_, index) => (
              <div className="music-skeleton" key={index} />
            ))}
          </div>
        ) : null}
        {loadError ? (
          <p className="empty-history">
            {t("app.loadFailed")}{" "}
            <button type="button" onClick={() => void load(null, true)}>
              {t("app.retry")}
            </button>
          </p>
        ) : null}
        {visibleItems.length === 0 && !loading && !loadError ? (
          <p className="empty-history">{t("app.noItems")}</p>
        ) : null}
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((virtual) => {
            const item = visibleItems[virtual.index];
            return (
              <article
                className="music-item"
                key={item.id}
                ref={virtualizer.measureElement}
                style={{
                  position: "absolute",
                  transform: `translateY(${virtual.start}px)`,
                  width: "100%",
                }}
              >
                <Artwork artworkId={item.artworkId} title={item.title} />
                <div className="music-copy">
                  <strong>{item.title ?? item.relativePath.split(/[\\/]/).pop()}</strong>
                  <span>
                    {item.artist ?? "—"} · {item.album ?? "—"}
                  </span>
                </div>
                <time>{formatDuration(item.durationMs, i18n.language)}</time>
                <span className={`outcome outcome-${item.outcome}`}>
                  {item.errorCode
                    ? t(errorMessageKey(item.errorCode), { defaultValue: t("errors.unknown") })
                    : t(`app.${item.outcome}`)}
                </span>
                <details>
                  <summary>{t("app.diagnostics")}</summary>
                  <p>
                    <b>{t("app.source")}</b> {item.sourcePath}
                  </p>
                  <p>
                    <b>{t("app.output")}</b> {item.destinationPath ?? "—"}
                  </p>
                  {item.errorDetail ? <p>{item.errorDetail}</p> : null}
                  {item.warnings.map((warning) => (
                    <p key={`${warning.code}-${warning.detail}`}>
                      {t(errorMessageKey(warning.code), { defaultValue: warning.code })}:{" "}
                      {warning.detail}
                    </p>
                  ))}
                </details>
              </article>
            );
          })}
        </div>
        {hasMore ? (
          <button
            className="load-more"
            disabled={loading}
            type="button"
            onClick={() => void load(cursor)}
          >
            {t("app.loadMore")}
          </button>
        ) : null}
        {loading && items.length > 0 ? (
          <div className="pagination-skeleton" aria-hidden="true" />
        ) : null}
      </div>
    </section>
  );
}
