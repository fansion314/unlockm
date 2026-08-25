import {
  ArrowLeft,
  ArrowSquareOut,
  CaretLeft,
  CaretRight,
  FolderOpen,
  MagnifyingGlass,
  Pause,
  Play,
  PencilSimple,
} from "@phosphor-icons/react";
import { motion } from "motion/react";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
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
import { usePlayback } from "../playback/PlaybackProvider";
import { MetadataEditor } from "../metadata/MetadataEditor";
import {
  clampTitleAlbumRatio,
  MAX_TITLE_ALBUM_RATIO,
  MIN_TITLE_ALBUM_RATIO,
  paginationItems,
  visibleItemNumber,
} from "./layout";

type HistoryDetailProps = {
  layoutId: string;
  reducedMotion?: boolean;
  run: RunSummary;
  onBack: (this: void) => void;
  onRename: (runId: number, displayName: string) => Promise<void>;
  onError: (this: void, error: unknown) => void;
};

function useWideSongLayout(): boolean {
  const query = "(min-width: 821px)";
  const [wide, setWide] = useState(
    () => typeof window.matchMedia !== "function" || window.matchMedia(query).matches,
  );
  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const media = window.matchMedia(query);
    const update = () => setWide(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  return wide;
}

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
    let active = true;
    let objectUrl: string | null = null;
    setUrl(null);
    if (artworkId === null || !visible)
      return () => {
        active = false;
      };
    void Promise.all([desktopApi.getArtworkMime(artworkId), desktopApi.getArtwork(artworkId)])
      .then(([metadata, bytes]) => {
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: metadata.mimeType }));
        if (active) setUrl(objectUrl);
        else URL.revokeObjectURL(objectUrl);
      })
      .catch(() => {
        if (active) setUrl(null);
      });
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

export function HistoryDetail({
  layoutId,
  reducedMotion = false,
  run,
  onBack,
  onRename,
  onError,
}: HistoryDetailProps) {
  const { i18n, t } = useTranslation();
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [page, setPage] = useState(1);
  const [pageCount, setPageCount] = useState(0);
  const [filteredTotal, setFilteredTotal] = useState(0);
  const [musicTotal, setMusicTotal] = useState(run.musicCount);
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
  const [repairPending, setRepairPending] = useState(false);
  const [editing, setEditing] = useState<HistoryItem | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(
    run.displayName || run.inputPath.split(/[\\/]/).pop() || run.inputPath,
  );
  const [renameBusy, setRenameBusy] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [titleAlbumRatio, setTitleAlbumRatio] = useState(60);
  const requestSequence = useRef(0);
  const draggingSeparatorPointer = useRef<number | null>(null);
  const draggingSeparatorTarget = useRef<HTMLElement | null>(null);
  const stopWindowSeparatorDrag = useRef<(() => void) | null>(null);
  const renameInputRef = useRef<HTMLInputElement>(null);
  const playback = usePlayback();
  const wideSongLayout = useWideSongLayout();

  useEffect(() => {
    if (!renaming) return;
    window.requestAnimationFrame(() => {
      renameInputRef.current?.focus();
      renameInputRef.current?.select();
    });
  }, [renaming]);

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
        setRepairPending(result.repairPending ?? false);
      })
      .catch(() => {
        if (requestSequence.current === sequence) setLoadError(true);
      })
      .finally(() => {
        if (requestSequence.current === sequence) setLoading(false);
      });
  }, [query, reloadToken, run.id]);

  useEffect(() => {
    if (!repairPending) return;
    const timer = window.setTimeout(() => setReloadToken((value) => value + 1), 250);
    return () => window.clearTimeout(timer);
  }, [repairPending]);

  const pages = useMemo(() => {
    return paginationItems(page, pageCount);
  }, [page, pageCount]);

  const runDuration = run.finishedAtMs === null ? null : run.finishedAtMs - run.createdAtMs;
  const mediaRowStyle = {
    "--title-track": `${titleAlbumRatio}fr`,
    "--album-track": `${100 - titleAlbumRatio}fr`,
  } as CSSProperties;
  const updateTitleAlbumRatioFromPointer = (target: HTMLElement, clientX: number) => {
    const grid = target.closest<HTMLElement>(".song-layout-grid");
    const titleCell = grid?.querySelector<HTMLElement>(".song-title-track");
    const albumCell = grid?.querySelector<HTMLElement>(".song-album-track");
    if (!titleCell || !albumCell) return;
    const start = titleCell.getBoundingClientRect().left;
    const end = albumCell.getBoundingClientRect().right;
    if (end <= start) return;
    setTitleAlbumRatio(clampTitleAlbumRatio(((clientX - start) / (end - start)) * 100));
  };
  const finishSeparatorDrag = (target: HTMLElement, pointerId: number) => {
    if (draggingSeparatorPointer.current !== pointerId) return;
    if (
      typeof target.hasPointerCapture === "function" &&
      target.hasPointerCapture(pointerId) &&
      typeof target.releasePointerCapture === "function"
    ) {
      target.releasePointerCapture(pointerId);
    }
    draggingSeparatorPointer.current = null;
    draggingSeparatorTarget.current = null;
    stopWindowSeparatorDrag.current?.();
    stopWindowSeparatorDrag.current = null;
  };
  const startSeparatorDrag = (target: HTMLElement, pointerId: number, clientX: number) => {
    draggingSeparatorPointer.current = pointerId;
    draggingSeparatorTarget.current = target;
    target.focus({ preventScroll: true });
    if (typeof target.setPointerCapture === "function") {
      target.setPointerCapture(pointerId);
    }
    updateTitleAlbumRatioFromPointer(target, clientX);

    const onWindowPointerMove = (event: PointerEvent) => {
      if (draggingSeparatorPointer.current === event.pointerId && draggingSeparatorTarget.current) {
        updateTitleAlbumRatioFromPointer(draggingSeparatorTarget.current, event.clientX);
      }
    };
    const onWindowPointerEnd = (event: PointerEvent) => {
      if (draggingSeparatorPointer.current === event.pointerId && draggingSeparatorTarget.current) {
        finishSeparatorDrag(draggingSeparatorTarget.current, event.pointerId);
      }
    };
    stopWindowSeparatorDrag.current?.();
    stopWindowSeparatorDrag.current = () => {
      window.removeEventListener("pointermove", onWindowPointerMove);
      window.removeEventListener("pointerup", onWindowPointerEnd);
      window.removeEventListener("pointercancel", onWindowPointerEnd);
    };
    window.addEventListener("pointermove", onWindowPointerMove);
    window.addEventListener("pointerup", onWindowPointerEnd);
    window.addEventListener("pointercancel", onWindowPointerEnd);
  };
  useEffect(
    () => () => {
      stopWindowSeparatorDrag.current?.();
    },
    [],
  );
  const startRename = () => {
    setRenameValue(run.displayName || run.inputPath.split(/[\\/]/).pop() || run.inputPath);
    setRenameError(null);
    setRenaming(true);
  };
  const cancelRename = () => {
    if (renameBusy) return;
    setRenameError(null);
    setRenaming(false);
  };
  const saveRename = async () => {
    const next = renameValue.trim();
    if (!next) {
      setRenameError(t("app.renameRequired"));
      return;
    }
    setRenameBusy(true);
    setRenameError(null);
    try {
      await onRename(run.id, next);
      setRenaming(false);
    } catch (error) {
      setRenameError(t("app.renameFailed"));
      onError(error);
    } finally {
      setRenameBusy(false);
    }
  };

  return (
    <section className="detail-view" aria-labelledby="detail-title">
      <button className="back-button detail-back-pill" type="button" onClick={onBack}>
        <ArrowLeft aria-hidden="true" size={18} />
        {t("app.backHome")}
      </button>

      <motion.header
        className="detail-header"
        layoutId={layoutId}
        transition={{ layout: pageTransition(reducedMotion).layout }}
      >
        <div className="detail-title-block">
          {renaming ? (
            <form
              className="detail-title-edit"
              onSubmit={(event) => {
                event.preventDefault();
                void saveRename();
              }}
            >
              <input
                ref={renameInputRef}
                id="detail-title"
                aria-label={t("app.rename")}
                disabled={renameBusy}
                maxLength={120}
                value={renameValue}
                onChange={(event) => setRenameValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    cancelRename();
                  }
                }}
              />
              <div>
                <button type="submit" disabled={renameBusy}>
                  {t("app.save")}
                </button>
                <button type="button" disabled={renameBusy} onClick={cancelRename}>
                  {t("app.cancelEdit")}
                </button>
              </div>
              {renameError ? <span role="alert">{renameError}</span> : null}
            </form>
          ) : (
            <h1 id="detail-title">{run.displayName || run.inputPath.split(/[\\/]/).pop()}</h1>
          )}
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
        <div className="detail-header-actions">
          {!renaming ? (
            <button
              aria-label={t("app.rename")}
              className="icon-button detail-reveal"
              title={t("app.rename")}
              type="button"
              onClick={startRename}
            >
              <PencilSimple aria-hidden="true" size={19} />
            </button>
          ) : null}
          <button
            aria-label={t("app.revealRunOutput")}
            className="icon-button detail-reveal"
            title={t("app.revealRunOutput")}
            type="button"
            onClick={() => void desktopApi.revealRunOutput(run.id).catch(onError)}
          >
            <FolderOpen aria-hidden="true" size={19} />
          </button>
        </div>
      </motion.header>

      <div className="detail-toolbar" data-reduced-motion={reducedMotion}>
        <label
          className={`compact-filter search-control ${searchInput ? "is-expanded has-clear" : ""}`}
          data-has-clear={Boolean(searchInput)}
        >
          <MagnifyingGlass aria-hidden="true" size={18} />
          <input
            aria-label={t("app.searchSongs")}
            placeholder={t("app.searchSongs")}
            type="search"
            value={searchInput}
            onChange={(event) => setSearchInput(event.target.value)}
          />
          {searchInput ? (
            <button
              className="filter-clear"
              aria-label={t("app.clearSearch")}
              type="button"
              onClick={() => {
                setSearchInput("");
                setSearch("");
                setPage(1);
              }}
            >
              ×
            </button>
          ) : null}
        </label>
        <label className={`compact-filter ${artist ? "is-expanded" : ""}`}>
          <span>{t("app.artist")}</span>
          <span className="filter-field" data-has-clear={Boolean(artist)}>
            <input
              list="artist-options"
              value={artist}
              placeholder={t("app.allArtists")}
              onChange={(event) => {
                setArtist(event.target.value);
                setPage(1);
              }}
            />
            {artist ? (
              <button
                className="filter-clear"
                aria-label={t("app.clearArtist")}
                type="button"
                onClick={() => {
                  setArtist("");
                  setPage(1);
                }}
              >
                ×
              </button>
            ) : null}
          </span>
          <datalist id="artist-options">
            {artists.map((value) => (
              <option key={value} value={value} />
            ))}
          </datalist>
        </label>
        <label className={`compact-filter ${album ? "is-expanded" : ""}`}>
          <span>{t("app.album")}</span>
          <span className="filter-field" data-has-clear={Boolean(album)}>
            <input
              list="album-options"
              value={album}
              placeholder={t("app.allAlbums")}
              onChange={(event) => {
                setAlbum(event.target.value);
                setPage(1);
              }}
            />
            {album ? (
              <button
                className="filter-clear"
                aria-label={t("app.clearAlbum")}
                type="button"
                onClick={() => {
                  setAlbum("");
                  setPage(1);
                }}
              >
                ×
              </button>
            ) : null}
          </span>
          <datalist id="album-options">
            {albums.map((value) => (
              <option key={value} value={value} />
            ))}
          </datalist>
        </label>
        <label className="compact-select">
          <span>{t("app.result")}</span>
          <span className="filter-field" data-has-clear={Boolean(outcome)}>
            <select
              value={outcome}
              onChange={(event) => {
                setOutcome(event.target.value as OutcomeFilter | "");
                setPage(1);
              }}
            >
              <option value="">{t("app.allResults")}</option>
              {(["converted", "copied", "skipped", "failed", "conflict", "cancelled"] as const).map(
                (value) => (
                  <option key={value} value={value}>
                    {t(`app.${value}`)}
                  </option>
                ),
              )}
            </select>
            {outcome ? (
              <button
                className="filter-clear"
                aria-label={t("app.clearOutcome")}
                type="button"
                onClick={() => {
                  setOutcome("");
                  setPage(1);
                }}
              >
                ×
              </button>
            ) : null}
          </span>
        </label>
        <label className="compact-select">
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
      </div>

      <div className="song-list" aria-busy={loading} style={mediaRowStyle}>
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
        {!loading && !loadError && items.length > 0 ? (
          <div className="song-list-header song-layout-grid">
            <span className="song-list-count" aria-live="polite">
              {t("app.filteredSongs", { count: filteredTotal })}
            </span>
            <span className="song-title-track">{t("app.columnSong")}</span>
            {wideSongLayout ? (
              <div
                aria-label={t("app.adjustTitleAlbumColumns")}
                aria-orientation="vertical"
                aria-valuemax={MAX_TITLE_ALBUM_RATIO}
                aria-valuemin={MIN_TITLE_ALBUM_RATIO}
                aria-valuenow={titleAlbumRatio}
                className="title-album-separator"
                onKeyDown={(event) => {
                  switch (event.key) {
                    case "ArrowLeft":
                      event.preventDefault();
                      setTitleAlbumRatio((value) => clampTitleAlbumRatio(value - 5));
                      break;
                    case "ArrowRight":
                      event.preventDefault();
                      setTitleAlbumRatio((value) => clampTitleAlbumRatio(value + 5));
                      break;
                    case "Home":
                      event.preventDefault();
                      setTitleAlbumRatio(MIN_TITLE_ALBUM_RATIO);
                      break;
                    case "End":
                      event.preventDefault();
                      setTitleAlbumRatio(MAX_TITLE_ALBUM_RATIO);
                      break;
                  }
                }}
                role="separator"
                tabIndex={0}
                title={t("app.adjustTitleAlbumColumns")}
                onPointerDown={(event) => {
                  event.preventDefault();
                  startSeparatorDrag(event.currentTarget, event.pointerId, event.clientX);
                }}
              />
            ) : (
              <span className="title-album-spacer" aria-hidden="true" />
            )}
            <span className="song-album-track">{t("app.columnAlbum")}</span>
            <span className="duration-cell">{t("app.columnDuration")}</span>
            <span className="format-cell">{t("app.columnFormat")}</span>
            <span className="file-facts">{t("app.columnFacts")}</span>
            <span className="song-actions-heading">{t("app.columnActions")}</span>
          </div>
        ) : null}
        {!loading && !loadError
          ? items.map((item, index) => {
              const title = itemTitle(item);
              const active = playback.activeItemId === item.id;
              const playable =
                item.destinationPath !== null &&
                item.outputFormat !== null &&
                ["converted", "copied", "skipped"].includes(item.outcome);
              const toggleTrack = () => void playback.toggle(item).catch(onError);
              return (
                <article
                  className={`music-item song-layout-grid ${active ? "is-active" : ""}`}
                  key={item.id}
                  onClick={(event) => {
                    if (
                      !playable ||
                      (event.target as HTMLElement).closest(
                        "button, input, select, a, [role='separator']",
                      )
                    ) {
                      return;
                    }
                    toggleTrack();
                  }}
                >
                  <button
                    aria-label={
                      active && playback.playing
                        ? t("app.pauseSong", { title })
                        : t("app.playSong", { title })
                    }
                    className="track-index"
                    disabled={!playable}
                    title={
                      active && playback.playing
                        ? t("app.pauseSong", { title })
                        : t("app.playSong", { title })
                    }
                    type="button"
                    onClick={toggleTrack}
                  >
                    <span className="track-number">
                      {String(visibleItemNumber(page, query.pageSize, index)).padStart(2, "0")}
                    </span>
                    <span className="track-toggle" aria-hidden="true">
                      {active && playback.playing ? (
                        <Pause weight="fill" />
                      ) : (
                        <Play weight="fill" />
                      )}
                    </span>
                  </button>
                  <VisibleArtwork artworkId={item.artworkId} title={title} />
                  <div className="music-copy song-title-track">
                    <strong>{title}</strong>
                    {item.artist ? (
                      <button
                        className="metadata-link"
                        title={item.artist}
                        type="button"
                        onPointerDown={(event) => event.stopPropagation()}
                        onClick={(event) => {
                          event.stopPropagation();
                          setArtist(item.artist!);
                          setPage(1);
                        }}
                      >
                        {item.artist}
                      </button>
                    ) : (
                      <span className="metadata-value">{t("app.unknownArtist")}</span>
                    )}
                  </div>
                  <span className="title-album-spacer" aria-hidden="true" />
                  {item.album ? (
                    <button
                      className="album-cell song-album-track metadata-link"
                      title={item.album}
                      type="button"
                      onPointerDown={(event) => event.stopPropagation()}
                      onClick={(event) => {
                        event.stopPropagation();
                        setAlbum(item.album!);
                        setPage(1);
                      }}
                    >
                      {item.album}
                    </button>
                  ) : (
                    <span className="album-cell song-album-track metadata-value">
                      {t("app.unknownAlbum")}
                    </span>
                  )}
                  <span className="duration-cell">
                    {formatDuration(item.durationMs, i18n.language)}
                  </span>
                  <div className="format-cell">
                    <strong>{item.outputFormat?.toUpperCase() || t("app.unknownFormat")}</strong>
                    {!(["converted", "skipped"] as string[]).includes(item.outcome) ? (
                      <small
                        className={`outcome outcome-${item.outcome}`}
                        title={t(`app.${item.outcome}`)}
                      >
                        {t(`app.${item.outcome}`)}
                      </small>
                    ) : null}
                  </div>
                  <div className="file-facts">
                    <span>{formatBytes(item.outputBytes, i18n.language)}</span>
                    {item.bitrateKbps ? <small>{item.bitrateKbps} kbps</small> : null}
                  </div>
                  <div className="song-actions">
                    <button
                      aria-label={t("app.editSong", { title })}
                      className="icon-button subtle"
                      disabled={!playable}
                      title={t("app.editSong", { title })}
                      type="button"
                      onClick={() => setEditing(item)}
                    >
                      <PencilSimple aria-hidden="true" size={17} />
                    </button>
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
          {pages.map((value, index) =>
            value === "ellipsis" ? (
              <span aria-hidden="true" className="pagination-ellipsis" key={`ellipsis-${index}`}>
                …
              </span>
            ) : (
              <button
                aria-current={value === page ? "page" : undefined}
                className={value === page ? "is-current" : ""}
                key={value}
                type="button"
                onClick={() => setPage(value)}
              >
                {value}
              </button>
            ),
          )}
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
      {editing ? (
        <MetadataEditor
          item={editing}
          onClose={() => setEditing(null)}
          onSaved={(updated) => {
            playback.refreshTrack(updated);
            setReloadToken((value) => value + 1);
          }}
        />
      ) : null}
    </section>
  );
}
