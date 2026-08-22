import {
  ArrowLeft,
  ArrowSquareOut,
  CaretLeft,
  CaretRight,
  FolderOpen,
  MagnifyingGlass,
  Pause,
  Play,
  X,
} from "@phosphor-icons/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { motion } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { pageTransition } from "../../app/page-motion";
import type {
  HistoryItem,
  ItemQuery,
  ItemSort,
  OutcomeFilter,
  RunSummary,
  SortDirection,
} from "../../lib/contracts";
import { formatBytes, formatDuration, formatTimestamp } from "../../lib/format";
import { desktopApi } from "../../lib/ipc";

type HistoryDetailProps = {
  layoutId: string;
  reducedMotion?: boolean;
  run: RunSummary;
  onBack: (this: void) => void;
  onError: (this: void, error: unknown) => void;
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
    if (artworkId === null || !visible) return;
    let active = true;
    let objectUrl: string | null = null;
    void Promise.all([desktopApi.getArtworkMime(artworkId), desktopApi.getArtwork(artworkId)])
      .then(([metadata, bytes]) => {
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: metadata.mimeType }));
        if (active) setUrl(objectUrl);
        else URL.revokeObjectURL(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [artworkId, visible]);
  return url ? (
    <img alt="" className="cover" src={url} />
  ) : (
    <span className="cover cover-placeholder">{title?.slice(0, 1) || "♪"}</span>
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
      { rootMargin: "160px" },
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

function itemTitle(item: HistoryItem): string {
  return item.title || item.relativePath.split(/[\\/]/).pop() || item.relativePath;
}

function audioUrl(itemId: number): string {
  return convertFileSrc(String(itemId), "unlockm-audio");
}

export function HistoryDetail({
  layoutId,
  reducedMotion = false,
  run,
  onBack,
  onError,
}: HistoryDetailProps) {
  const { i18n, t } = useTranslation();
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [page, setPage] = useState(1);
  const [pageCount, setPageCount] = useState(0);
  const [filteredTotal, setFilteredTotal] = useState(0);
  const [musicTotal, setMusicTotal] = useState(run.musicCount);
  const [companionCount, setCompanionCount] = useState(0);
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [artist, setArtist] = useState("");
  const [album, setAlbum] = useState("");
  const [outcome, setOutcome] = useState<OutcomeFilter | "">("");
  const [sort, setSort] = useState<ItemSort>("ordinal");
  const [direction, setDirection] = useState<SortDirection>("asc");
  const [artists, setArtists] = useState<string[]>([]);
  const [albums, setAlbums] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);
  const requestSequence = useRef(0);

  const audioRef = useRef<HTMLAudioElement>(null);
  const [activeItemId, setActiveItemId] = useState<number | null>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [audioDuration, setAudioDuration] = useState(0);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setSearch(searchInput.trim());
      setPage(1);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [searchInput]);

  useEffect(() => {
    let active = true;
    void Promise.all([
      desktopApi.listRunItemFacets(run.id, "artist", null, null, 200),
      desktopApi.listRunItemFacets(run.id, "album", null, null, 200),
    ])
      .then(([artistPage, albumPage]) => {
        if (!active) return;
        setArtists(artistPage.values);
        setAlbums(albumPage.values);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [run.id]);

  const query = useMemo<ItemQuery>(
    () => ({
      page,
      pageSize: 25,
      search: search || null,
      artist: artist || null,
      album: album || null,
      outcome: outcome || null,
      sort,
      direction,
    }),
    [album, artist, direction, outcome, page, search, sort],
  );

  useEffect(() => {
    const sequence = ++requestSequence.current;
    setLoading(true);
    setLoadError(false);
    void desktopApi
      .queryRunItems(run.id, query)
      .then((result) => {
        if (requestSequence.current !== sequence) return;
        setItems(result.items);
        setPage(result.page);
        setPageCount(result.pageCount);
        setFilteredTotal(result.filteredTotal);
        setMusicTotal(result.musicTotal);
        setCompanionCount(result.companionCount);
      })
      .catch(() => {
        if (requestSequence.current === sequence) setLoadError(true);
      })
      .finally(() => {
        if (requestSequence.current === sequence) setLoading(false);
      });
  }, [query, reloadToken, run.id]);

  const stopAudio = useCallback(() => {
    const audio = audioRef.current;
    if (audio) {
      audio.pause();
      audio.removeAttribute("src");
    }
    setActiveItemId(null);
    setPlaying(false);
    setCurrentTime(0);
    setAudioDuration(0);
  }, []);

  useEffect(() => stopAudio, [stopAudio]);

  useEffect(() => {
    if (activeItemId !== null && !items.some((item) => item.id === activeItemId)) stopAudio();
  }, [activeItemId, items, stopAudio]);

  const togglePlayback = useCallback(
    async (item: HistoryItem) => {
      const audio = audioRef.current;
      if (!audio) return;
      if (activeItemId === item.id) {
        if (audio.paused) {
          await audio.play();
          setPlaying(true);
        } else {
          audio.pause();
          setPlaying(false);
        }
        return;
      }
      audio.pause();
      audio.src = audioUrl(item.id);
      audio.load();
      setActiveItemId(item.id);
      setCurrentTime(0);
      setAudioDuration((item.durationMs ?? 0) / 1_000);
      try {
        await audio.play();
        setPlaying(true);
      } catch (error) {
        setPlaying(false);
        onError({ code: "playback_unavailable", detail: String(error) });
      }
    },
    [activeItemId, onError],
  );

  const resetFilters = () => {
    setSearchInput("");
    setSearch("");
    setArtist("");
    setAlbum("");
    setOutcome("");
    setSort("ordinal");
    setDirection("asc");
    setPage(1);
  };

  const pages = useMemo(() => {
    if (pageCount <= 1) return [];
    const start = Math.max(1, Math.min(page - 2, pageCount - 4));
    const end = Math.min(pageCount, Math.max(page + 2, 5));
    return Array.from({ length: end - start + 1 }, (_, index) => start + index);
  }, [page, pageCount]);

  const runDuration = run.finishedAtMs === null ? null : run.finishedAtMs - run.createdAtMs;

  return (
    <section className="detail-view" aria-labelledby="detail-title">
      <button className="back-button" type="button" onClick={onBack}>
        <ArrowLeft aria-hidden="true" size={18} />
        {t("app.back")}
      </button>

      <motion.header
        className="detail-header"
        layoutId={layoutId}
        transition={{ layout: pageTransition(reducedMotion).layout }}
      >
        <div className="detail-title-block">
          <h1 id="detail-title">{run.inputPath.split(/[\\/]/).pop()}</h1>
          <span title={run.inputPath}>
            {t("app.source")}: {run.inputPath}
          </span>
          <span title={run.outputPath}>
            {t("app.output")}: {run.outputPath}
          </span>
        </div>
        <dl className="detail-facts">
          <div>
            <dt>{t("app.songs")}</dt>
            <dd>{musicTotal}</dd>
          </div>
          <div>
            <dt>{t("app.result")}</dt>
            <dd>{t(`app.${run.status}`)}</dd>
          </div>
          <div>
            <dt>{t("app.duration")}</dt>
            <dd>{formatDuration(runDuration, i18n.language)}</dd>
          </div>
          <div>
            <dt>{t("app.finishedAt")}</dt>
            <dd>{formatTimestamp(run.updatedAtMs, i18n.language)}</dd>
          </div>
        </dl>
        <button
          aria-label={t("app.revealRunOutput")}
          className="icon-button detail-reveal"
          title={t("app.revealRunOutput")}
          type="button"
          onClick={() => void desktopApi.revealRunOutput(run.id).catch(onError)}
        >
          <FolderOpen aria-hidden="true" size={19} />
        </button>
      </motion.header>

      <div className="detail-toolbar">
        <label className="search-control">
          <MagnifyingGlass aria-hidden="true" size={18} />
          <input
            aria-label={t("app.searchSongs")}
            placeholder={t("app.searchSongs")}
            type="search"
            value={searchInput}
            onChange={(event) => setSearchInput(event.target.value)}
          />
        </label>
        <label>
          <span>{t("app.artist")}</span>
          <input
            list="artist-options"
            value={artist}
            placeholder={t("app.allArtists")}
            onChange={(event) => {
              setArtist(event.target.value);
              setPage(1);
            }}
          />
          <datalist id="artist-options">
            {artists.map((value) => (
              <option key={value} value={value} />
            ))}
          </datalist>
        </label>
        <label>
          <span>{t("app.album")}</span>
          <input
            list="album-options"
            value={album}
            placeholder={t("app.allAlbums")}
            onChange={(event) => {
              setAlbum(event.target.value);
              setPage(1);
            }}
          />
          <datalist id="album-options">
            {albums.map((value) => (
              <option key={value} value={value} />
            ))}
          </datalist>
        </label>
        <label>
          <span>{t("app.result")}</span>
          <select
            value={outcome}
            onChange={(event) => {
              setOutcome(event.target.value as OutcomeFilter | "");
              setPage(1);
            }}
          >
            <option value="">{t("app.allResults")}</option>
            {(["converted", "skipped", "failed", "conflict", "cancelled"] as const).map((value) => (
              <option key={value} value={value}>
                {t(`app.${value}`)}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("app.sort")}</span>
          <select
            value={`${sort}:${direction}`}
            onChange={(event) => {
              const [nextSort, nextDirection] = event.target.value.split(":") as [
                ItemSort,
                SortDirection,
              ];
              setSort(nextSort);
              setDirection(nextDirection);
              setPage(1);
            }}
          >
            <option value="ordinal:asc">{t("app.sortOriginal")}</option>
            <option value="title:asc">{t("app.sortTitle")}</option>
            <option value="artist:asc">{t("app.sortArtist")}</option>
            <option value="album:asc">{t("app.sortAlbum")}</option>
            <option value="size:desc">{t("app.sortSize")}</option>
          </select>
        </label>
        <button
          aria-label={t("app.clearFilters")}
          className="icon-button toolbar-clear"
          title={t("app.clearFilters")}
          type="button"
          onClick={resetFilters}
        >
          <X aria-hidden="true" size={17} />
        </button>
      </div>

      <div className="list-summary" aria-live="polite">
        <span>{t("app.filteredSongs", { count: filteredTotal })}</span>
        {companionCount > 0 ? <span>{t("app.companions", { count: companionCount })}</span> : null}
      </div>

      <audio
        className="audio-engine"
        ref={audioRef}
        onDurationChange={(event) => setAudioDuration(event.currentTarget.duration || 0)}
        onEnded={() => setPlaying(false)}
        onError={() => {
          setPlaying(false);
          onError({
            code: "playback_unavailable",
            detail: t("app.playbackUnavailable"),
          });
        }}
        onPause={() => setPlaying(false)}
        onPlay={() => setPlaying(true)}
        onTimeUpdate={(event) => setCurrentTime(event.currentTarget.currentTime)}
      />

      <div className="song-list" aria-busy={loading}>
        {loading ? (
          <div className="detail-skeletons" aria-hidden="true">
            {Array.from({ length: 6 }, (_, index) => (
              <div className="music-skeleton" key={index} />
            ))}
          </div>
        ) : null}
        {loadError && !loading ? (
          <div className="empty-history">
            <span>{t("app.loadFailed")}</span>
            <button type="button" onClick={() => setReloadToken((value) => value + 1)}>
              {t("app.retry")}
            </button>
          </div>
        ) : null}
        {!loading && !loadError && items.length === 0 ? (
          <div className="empty-history">{t("app.noItems")}</div>
        ) : null}
        {!loading && !loadError
          ? items.map((item) => {
              const title = itemTitle(item);
              const active = activeItemId === item.id;
              const playable =
                item.destinationPath !== null &&
                item.outputFormat !== null &&
                ["converted", "skipped"].includes(item.outcome);
              const duration = active ? audioDuration : (item.durationMs ?? 0) / 1_000;
              const elapsed = active ? currentTime : 0;
              return (
                <article className="music-item" key={item.id}>
                  <Artwork artworkId={item.artworkId} title={title} />
                  <div className="music-copy">
                    <strong>{title}</strong>
                    <span>{item.artist || t("app.unknownArtist")}</span>
                    <small>{item.album || t("app.unknownAlbum")}</small>
                  </div>
                  <div className="file-facts">
                    <strong>{item.outputFormat?.toUpperCase() || t("app.unknownFormat")}</strong>
                    <span>{formatBytes(item.outputBytes, i18n.language)}</span>
                    {!(["converted", "skipped"] as string[]).includes(item.outcome) ? (
                      <small className={`outcome outcome-${item.outcome}`}>
                        {t(`app.${item.outcome}`)}
                      </small>
                    ) : null}
                  </div>
                  <div className="track-player">
                    <button
                      aria-label={
                        active && playing
                          ? t("app.pauseSong", { title })
                          : t("app.playSong", { title })
                      }
                      className="play-button"
                      disabled={!playable}
                      title={
                        active && playing
                          ? t("app.pauseSong", { title })
                          : t("app.playSong", { title })
                      }
                      type="button"
                      onClick={() => void togglePlayback(item)}
                    >
                      {active && playing ? (
                        <Pause aria-hidden="true" size={17} weight="fill" />
                      ) : (
                        <Play aria-hidden="true" size={17} weight="fill" />
                      )}
                    </button>
                    <input
                      aria-label={t("app.seekSong", { title })}
                      className="seek-control"
                      disabled={!active || duration <= 0}
                      max={Math.max(duration, 1)}
                      min={0}
                      step={0.1}
                      type="range"
                      value={Math.min(elapsed, Math.max(duration, 1))}
                      onChange={(event) => {
                        const value = Number(event.target.value);
                        if (audioRef.current && active) audioRef.current.currentTime = value;
                        setCurrentTime(value);
                      }}
                    />
                    <span>
                      {formatDuration(elapsed * 1_000, i18n.language)} /{" "}
                      {formatDuration(duration * 1_000, i18n.language)}
                    </span>
                  </div>
                  <div className="song-actions">
                    <button
                      aria-label={t("app.revealSong", { title })}
                      className="icon-button subtle"
                      disabled={item.destinationPath === null}
                      title={t("app.revealSong", { title })}
                      type="button"
                      onClick={() => void desktopApi.revealRunItem(item.id).catch(onError)}
                    >
                      <FolderOpen aria-hidden="true" size={17} />
                    </button>
                    <button
                      aria-label={t("app.openSong", { title })}
                      className="icon-button subtle"
                      disabled={item.destinationPath === null}
                      title={t("app.openSong", { title })}
                      type="button"
                      onClick={() => void desktopApi.openRunItem(item.id).catch(onError)}
                    >
                      <ArrowSquareOut aria-hidden="true" size={17} />
                    </button>
                  </div>
                </article>
              );
            })
          : null}
      </div>

      {pageCount > 1 ? (
        <nav className="pagination" aria-label={t("app.pagination")}>
          <button
            aria-label={t("app.previousPage")}
            disabled={page <= 1}
            type="button"
            onClick={() => setPage((value) => Math.max(1, value - 1))}
          >
            <CaretLeft aria-hidden="true" size={17} />
          </button>
          {pages.map((value) => (
            <button
              aria-current={value === page ? "page" : undefined}
              className={value === page ? "is-current" : ""}
              key={value}
              type="button"
              onClick={() => setPage(value)}
            >
              {value}
            </button>
          ))}
          <button
            aria-label={t("app.nextPage")}
            disabled={page >= pageCount}
            type="button"
            onClick={() => setPage((value) => Math.min(pageCount, value + 1))}
          >
            <CaretRight aria-hidden="true" size={17} />
          </button>
        </nav>
      ) : null}
    </section>
  );
}
