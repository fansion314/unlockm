import { ArrowSquareOut, CheckCircle, ImageSquare, MagnifyingGlass } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  ArtworkCandidate,
  HistoryItem,
  OnlineMetadataCandidate,
  OnlineMetadataProviderCapability,
  OnlineMetadataProvider,
  OnlineMetadataStatus,
} from "../../lib/contracts";
import { desktopApi } from "../../lib/ipc";
import { usePlayback } from "../playback/PlaybackProvider";

function onlineErrorCode(reason: unknown) {
  return typeof reason === "object" && reason && "code" in reason && typeof reason.code === "string"
    ? reason.code
    : "provider_unavailable";
}

function onlineErrorKey(provider: OnlineMetadataProvider, reason: unknown) {
  const code = onlineErrorCode(reason);
  if (
    code === "rate_limited" ||
    code === "provider_unavailable" ||
    code === "request_timeout" ||
    code === "provider_malformed_response"
  ) {
    return `${provider === "musicBrainz" ? "musicbrainz" : "spotify"}_${code}`;
  }
  return code;
}

type CoverFeedback = {
  candidateKey: string;
  state: "loading" | "success" | "error";
  source?: string;
  errorKey?: string;
};

function onlineCandidateKey(candidate: OnlineMetadataCandidate) {
  return `${candidate.provider}:${candidate.itemId}:${candidate.album}`;
}

function formatDuration(durationMs: number, unknown: string) {
  if (!durationMs) return unknown;
  const totalSeconds = Math.round(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function CandidateArtwork({ candidate }: { candidate: ArtworkCandidate }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    let objectUrl: string | null = null;
    void Promise.all([
      desktopApi.getArtworkMime(candidate.artworkId),
      desktopApi.getArtwork(candidate.artworkId),
    ])
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
  }, [candidate.artworkId]);
  return url ? <img src={url} alt="" /> : <span aria-hidden="true">♪</span>;
}

// Kept as an isolated preview primitive for explicit callers. The editor does
// not mount it for MusicBrainz results: free artwork lookup must begin only
// when the user chooses Use cover or Fill all.
export function OnlineCover({ candidate }: { candidate: OnlineMetadataCandidate }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!candidate.coverHandle) return;
    let active = true;
    let objectUrl: string | null = null;
    void desktopApi
      .getOnlineMetadataCoverPreview(candidate.provider, candidate.coverHandle)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes]));
        setUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [candidate.coverHandle, candidate.provider]);
  return url ? <img className="metadata-online-cover" src={url} alt="" /> : null;
}

export function MetadataEditor({
  item,
  onClose,
  onSaved,
}: {
  item: HistoryItem;
  onClose: () => void;
  onSaved: (updated: HistoryItem) => void;
}) {
  const { t } = useTranslation();
  const playback = usePlayback();
  const [title, setTitle] = useState(item.title ?? "");
  const [artist, setArtist] = useState(item.artist ?? "");
  const [album, setAlbum] = useState(item.album ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cover, setCover] = useState<Uint8Array | undefined>();
  const [preview, setPreview] = useState<string | null>(null);
  const [candidates, setCandidates] = useState<ArtworkCandidate[]>([]);
  const [existingArtworkId, setExistingArtworkId] = useState<number | undefined>();
  const [candidateSearch, setCandidateSearch] = useState(item.album ?? "");
  const [onlineStatus, setOnlineStatus] = useState<OnlineMetadataStatus | null>(null);
  const [onlineQuery, setOnlineQuery] = useState(
    [item.title, item.artist, item.album].filter(Boolean).join(" "),
  );
  const [onlineResults, setOnlineResults] = useState<OnlineMetadataCandidate[]>([]);
  const [onlineProvider, setOnlineProvider] = useState<OnlineMetadataProvider>("musicBrainz");
  const [onlineBusy, setOnlineBusy] = useState(false);
  const [onlineError, setOnlineError] = useState<string | null>(null);
  const [onlineApplied, setOnlineApplied] = useState(false);
  const [onlineSearched, setOnlineSearched] = useState(false);
  const [coverBusyKey, setCoverBusyKey] = useState<string | null>(null);
  const [coverFeedback, setCoverFeedback] = useState<CoverFeedback | null>(null);
  const searchGeneration = useRef(0);
  const onlineProviderRef = useRef<OnlineMetadataProvider>("musicBrainz");
  const mounted = useRef(true);
  const titleRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);
  useEffect(() => {
    const mountedRef = mounted;
    const generationRef = searchGeneration;
    return () => {
      mountedRef.current = false;
      ++generationRef.current;
    };
  }, []);
  useEffect(() => {
    titleRef.current?.focus();
  }, []);
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
      }
      if (event.key !== "Tab" || !dialogRef.current) return;
      const focusable = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable.at(-1)!;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, []);
  useEffect(
    () => () => {
      if (preview) URL.revokeObjectURL(preview);
    },
    [preview],
  );
  useEffect(() => {
    const timer = window.setTimeout(() => {
      void desktopApi
        .listArtworkCandidates(item.id, candidateSearch, 12)
        .then(setCandidates)
        .catch(() => undefined);
    }, 180);
    return () => window.clearTimeout(timer);
  }, [candidateSearch, item.id]);
  useEffect(() => {
    void desktopApi
      .onlineMetadataStatus("musicBrainz")
      .then(setOnlineStatus)
      .catch(() => undefined);
  }, []);
  useEffect(() => {
    const retryAtMs = onlineStatus?.retryAtMs;
    if (!retryAtMs) return;
    const timer = window.setTimeout(
      () =>
        setOnlineStatus((current) =>
          current?.retryAtMs === retryAtMs ? { ...current, retryAtMs: null } : current,
        ),
      Math.max(0, retryAtMs - Date.now()),
    );
    return () => window.clearTimeout(timer);
  }, [onlineStatus?.retryAtMs]);
  useEffect(() => {
    // This is a local capability read, not a provider request. It lets Rust
    // keep the no-auth primary ordering authoritative while opening the editor
    // remains free of MusicBrainz, CAA, TheAudioDB, or Spotify network traffic.
    void Promise.resolve(desktopApi.onlineMetadataProviderCapabilities())
      .then((capabilities: OnlineMetadataProviderCapability[] | undefined) => {
        const defaultProvider = capabilities
          ?.filter((capability) => !capability.requiresAuthorization)
          .sort((left, right) => left.primaryOrder - right.primaryOrder)[0]?.provider;
        if (defaultProvider && onlineProviderRef.current === "musicBrainz") {
          onlineProviderRef.current = defaultProvider;
          setOnlineProvider(defaultProvider);
        }
      })
      .catch(() => undefined);
  }, []);
  const searchOnline = async () => {
    const generation = ++searchGeneration.current;
    const provider = onlineProvider;
    setCoverBusyKey(null);
    setCoverFeedback((current) => (current?.state === "success" ? current : null));
    setOnlineBusy(true);
    setOnlineError(null);
    setOnlineSearched(true);
    try {
      const results = await desktopApi.searchOnlineMetadata(provider, onlineQuery);
      if (generation === searchGeneration.current && onlineProviderRef.current === provider) {
        setOnlineResults(results);
      }
    } catch (reason) {
      if (generation === searchGeneration.current && onlineProviderRef.current === provider) {
        setOnlineError(onlineErrorKey(provider, reason));
        const status = await desktopApi.onlineMetadataStatus(provider).catch(() => undefined);
        if (status) setOnlineStatus(status);
      }
    } finally {
      if (generation === searchGeneration.current && onlineProviderRef.current === provider) {
        setOnlineBusy(false);
      }
    }
  };
  const selectOnlineProvider = (provider: OnlineMetadataProvider) => {
    ++searchGeneration.current;
    onlineProviderRef.current = provider;
    setOnlineProvider(provider);
    setOnlineResults([]);
    setOnlineSearched(false);
    setOnlineError(null);
    setOnlineBusy(false);
    setCoverBusyKey(null);
    setCoverFeedback((current) => (current?.state === "success" ? current : null));
    void Promise.resolve(desktopApi.onlineMetadataStatus(provider))
      .then((status) => {
        if (onlineProviderRef.current === provider) setOnlineStatus(status);
      })
      .catch(() => undefined);
  };
  const connectOnline = () => {
    setOnlineError(null);
    setOnlineStatus({ provider: "spotify", state: "authorizing", retryAtMs: null });
    void desktopApi
      .startOnlineMetadataLogin("spotify")
      .then(setOnlineStatus)
      .catch(async (reason) => {
        setOnlineError(onlineErrorCode(reason));
        const status = await desktopApi.onlineMetadataStatus("spotify").catch(() => undefined);
        if (status) setOnlineStatus(status);
      });
  };
  const isRateLimited = (provider: OnlineMetadataProvider) =>
    onlineStatus?.provider === provider && Boolean(onlineStatus.retryAtMs);
  const applyText = (candidate: OnlineMetadataCandidate) => {
    setTitle(candidate.title);
    setArtist(candidate.artists.join(", "));
    setAlbum(candidate.album);
    setOnlineApplied(true);
  };
  const applyCover = async (candidate: OnlineMetadataCandidate) => {
    const candidateKey = onlineCandidateKey(candidate);
    if (!candidate.coverHandle) {
      setCoverFeedback({ candidateKey, state: "error" });
      return;
    }
    const generation = searchGeneration.current;
    setCoverBusyKey(candidateKey);
    setCoverFeedback({ candidateKey, state: "loading" });
    setOnlineBusy(true);
    setOnlineError(null);
    try {
      const bytes = await desktopApi.getOnlineMetadataCoverPreview(
        candidate.provider,
        candidate.coverHandle,
      );
      if (!mounted.current || generation !== searchGeneration.current) return;
      if (onlineProviderRef.current !== candidate.provider) return;
      const coverBytes = new Uint8Array(bytes);
      const nextPreview = URL.createObjectURL(new Blob([coverBytes]));
      setCover(coverBytes);
      setPreview(nextPreview);
      setExistingArtworkId(undefined);
      setOnlineApplied(true);
      setCoverFeedback({
        candidateKey,
        state: "success",
        source: candidate.artworkSource ?? candidate.attribution,
      });
    } catch (reason) {
      if (
        mounted.current &&
        generation === searchGeneration.current &&
        onlineProviderRef.current === candidate.provider
      ) {
        const errorKey =
          onlineErrorCode(reason) === "online_artwork_not_found"
            ? "online_artwork_not_found"
            : "online_cover_failed";
        setOnlineError(errorKey);
        setCoverFeedback({ candidateKey, state: "error", errorKey });
      }
    } finally {
      if (
        mounted.current &&
        generation === searchGeneration.current &&
        onlineProviderRef.current === candidate.provider
      ) {
        setOnlineBusy(false);
        setCoverBusyKey(null);
      }
    }
  };
  const renderOnlineCandidate = (candidate: OnlineMetadataCandidate) => {
    const candidateKey = onlineCandidateKey(candidate);
    const feedback = coverFeedback?.candidateKey === candidateKey ? coverFeedback : null;
    const metadataSource =
      candidate.provider === "musicBrainz" ? t("app.onlineMusicBrainz") : "Spotify";
    const artworkSource = candidate.artworkSource ?? t("app.onlineArtworkUnavailable");
    return (
      <article
        className="metadata-online-result"
        key={candidateKey}
        role="listitem"
        aria-label={t("app.onlineResultLabel", { title: candidate.title })}
      >
        <div className="metadata-online-result-heading">
          <div>
            <strong>{candidate.title}</strong>
            <small>{metadataSource}</small>
          </div>
          {candidate.provider === "spotify" && candidate.coverHandle ? (
            <OnlineCover candidate={candidate} />
          ) : null}
        </div>
        <dl className="metadata-online-fields">
          <div>
            <dt>{t("app.title")}</dt>
            <dd>{candidate.title}</dd>
          </div>
          <div>
            <dt>{t("app.artist")}</dt>
            <dd>{candidate.artists.join(", ") || t("app.onlineUnknownValue")}</dd>
          </div>
          <div>
            <dt>{t("app.album")}</dt>
            <dd>{candidate.album || t("app.onlineUnknownValue")}</dd>
          </div>
          <div>
            <dt>{t("app.onlineDuration")}</dt>
            <dd>{formatDuration(candidate.durationMs, t("app.onlineUnknownValue"))}</dd>
          </div>
          <div>
            <dt>{t("app.onlineMetadataSource")}</dt>
            <dd>{metadataSource}</dd>
          </div>
          <div>
            <dt>{t("app.onlineArtworkSource")}</dt>
            <dd>{artworkSource}</dd>
          </div>
        </dl>
        <div className="metadata-online-result-actions">
          <button type="button" onClick={() => applyText(candidate)}>
            {t("app.onlineApplyText")}
          </button>
          <button
            type="button"
            disabled={!candidate.coverHandle || onlineBusy}
            onClick={() => void applyCover(candidate)}
          >
            {t("app.onlineApplyCover")}
          </button>
          <button
            className="metadata-button-primary"
            type="button"
            disabled={!candidate.coverHandle || onlineBusy}
            onClick={() => {
              applyText(candidate);
              void applyCover(candidate);
            }}
          >
            {t("app.onlineApplyAll")}
          </button>
          <button
            className="metadata-button-quiet"
            type="button"
            onClick={() =>
              void desktopApi.openOnlineMetadataResult(candidate.provider, candidate.itemId)
            }
          >
            <ArrowSquareOut aria-hidden="true" size={16} />
            {candidate.provider === "musicBrainz"
              ? t("app.onlineOpenMusicBrainz")
              : t("app.onlineOpenSpotify")}
          </button>
        </div>
        {feedback?.state === "loading" ? (
          <p className="metadata-result-feedback" role="status">
            {t("app.onlineCoverLoading")}
          </p>
        ) : feedback?.state === "success" ? (
          <p className="metadata-result-feedback is-success" role="status">
            <CheckCircle aria-hidden="true" size={17} />
            {t("app.onlineCoverAppliedSource", { source: feedback.source })}
          </p>
        ) : feedback?.state === "error" ? (
          <p className="metadata-result-feedback is-error" role="alert">
            {feedback.errorKey
              ? t(`errors.${feedback.errorKey}`, { defaultValue: t("app.onlineCoverUnavailable") })
              : t("app.onlineCoverUnavailable")}
          </p>
        ) : null}
      </article>
    );
  };
  const save = async () => {
    let released = null;
    setBusy(true);
    setError(null);
    try {
      released = playback.releaseForItem(item.id);
      const updated = await desktopApi.updateRunItemMetadata(item.id, {
        title: title || null,
        artist: artist || null,
        album: album || null,
        cover,
        existingArtworkId,
      });
      onSaved(updated);
      await playback.restoreReleased(released, updated);
      onClose();
    } catch (reason) {
      await playback.restoreReleased(released, item);
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };
  const isSearching = onlineBusy && !coverBusyKey;
  const canShowOnlineResults =
    onlineProvider === "musicBrainz" || onlineStatus?.state === "connected";
  return (
    <div className="metadata-modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        ref={dialogRef}
        className="metadata-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("app.editMetadata")}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="metadata-modal-header">
          <div>
            <h2>{t("app.editMetadata")}</h2>
            <p>{t("app.editMetadataHint")}</p>
          </div>
        </header>
        <div className="metadata-modal-body" data-layout="split-workspace" tabIndex={-1}>
          <div className="metadata-editor-pane">
            <section className="metadata-editor-section" aria-labelledby="metadata-fields-title">
              <header className="metadata-pane-heading">
                <h3 id="metadata-fields-title">{t("app.metadataFields")}</h3>
                <p>{t("app.metadataDraftHint")}</p>
              </header>
              <label>
                {t("app.title")}
                <input
                  ref={titleRef}
                  value={title}
                  maxLength={512}
                  onChange={(event) => setTitle(event.target.value)}
                />
              </label>
              <label>
                {t("app.artist")}
                <input
                  value={artist}
                  maxLength={512}
                  onChange={(event) => setArtist(event.target.value)}
                />
              </label>
              <label>
                {t("app.album")}
                <input
                  value={album}
                  maxLength={512}
                  onChange={(event) => setAlbum(event.target.value)}
                />
              </label>
            </section>

            <section className="metadata-editor-section" aria-labelledby="cover-draft-title">
              <header className="metadata-pane-heading">
                <h3 id="cover-draft-title">{t("app.coverDraft")}</h3>
                <p>{t("app.coverDraftHint")}</p>
              </header>
              <div className="metadata-cover-stage" data-has-preview={Boolean(preview)}>
                {preview ? (
                  <img
                    className="metadata-cover-preview"
                    src={preview}
                    alt={t("app.coverPreview")}
                  />
                ) : (
                  <div className="metadata-cover-placeholder" aria-hidden="true">
                    <ImageSquare size={34} />
                  </div>
                )}
                <div className="metadata-cover-copy">
                  <strong>
                    {preview
                      ? t("app.coverDraftReady")
                      : existingArtworkId
                        ? t("app.localCoverSelected", { id: existingArtworkId })
                        : t("app.coverDraftEmpty")}
                  </strong>
                  <small>
                    {coverFeedback?.state === "success"
                      ? t("app.onlineCoverAppliedSource", { source: coverFeedback.source })
                      : t("app.coverDraftSaveHint")}
                  </small>
                </div>
              </div>
              {coverFeedback?.state === "loading" ? (
                <p className="metadata-cover-status" role="status">
                  {t("app.onlineCoverLoading")}
                </p>
              ) : coverFeedback?.state === "success" ? (
                <p className="metadata-cover-status is-success" role="status">
                  <CheckCircle aria-hidden="true" size={17} />
                  {t("app.onlineCoverAppliedUnsaved")}
                </p>
              ) : null}
              <label>
                {t("app.cover")}
                <input
                  aria-label={t("app.uploadCover")}
                  type="file"
                  accept="image/png,image/jpeg,image/gif,image/webp"
                  onChange={(event) => {
                    const file = event.currentTarget.files?.[0];
                    if (!file) return;
                    if (file.size > 10 * 1024 * 1024) {
                      setError(t("app.coverTooLarge"));
                      return;
                    }
                    setPreview(URL.createObjectURL(file));
                    setExistingArtworkId(undefined);
                    setCoverFeedback(null);
                    void file.arrayBuffer().then((bytes) => setCover(new Uint8Array(bytes)));
                  }}
                />
              </label>
              <div className="metadata-candidates">
                <label>
                  {t("app.localCoverCandidates")}
                  <input
                    value={candidateSearch}
                    onChange={(event) => setCandidateSearch(event.target.value)}
                    placeholder={t("app.searchLocalCover")}
                  />
                </label>
                {candidates.length ? (
                  <div className="metadata-candidate-grid">
                    {candidates.map((candidate) => (
                      <button
                        type="button"
                        key={candidate.artworkId}
                        aria-label={t("app.useCoverCandidate", { id: candidate.artworkId })}
                        aria-pressed={existingArtworkId === candidate.artworkId}
                        onClick={() => {
                          setExistingArtworkId(candidate.artworkId);
                          setCover(undefined);
                          setPreview(null);
                          setCoverFeedback(null);
                        }}
                      >
                        <CandidateArtwork candidate={candidate} />
                        <small>
                          {candidate.album || candidate.title || `#${candidate.artworkId}`}
                        </small>
                      </button>
                    ))}
                  </div>
                ) : (
                  <small>{t("app.noCoverCandidates")}</small>
                )}
              </div>
              {error ? <p role="alert">{error}</p> : null}
            </section>
          </div>

          <section className="metadata-online" aria-label={t("app.onlineMetadata")}>
            <header className="metadata-pane-heading">
              <h3>{t("app.onlineMetadata")}</h3>
              <p>{t("app.onlineSearchPanelHint")}</p>
            </header>
            <label>
              {t("app.onlineProvider")}
              <select
                value={onlineProvider}
                onChange={(event) =>
                  selectOnlineProvider(event.target.value as OnlineMetadataProvider)
                }
              >
                <option value="musicBrainz">{t("app.onlineMusicBrainz")}</option>
                <option value="spotify">{t("app.onlineSpotify")}</option>
              </select>
            </label>
            {onlineProvider === "musicBrainz" ? (
              <>
                <p className="metadata-online-note">{t("app.onlineMusicBrainzHint")}</p>
                <div className="metadata-online-search-controls">
                  <label>
                    {t("app.onlineSearchMusicBrainz")}
                    <input
                      value={onlineQuery}
                      maxLength={200}
                      onChange={(event) => setOnlineQuery(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          event.preventDefault();
                          void searchOnline();
                        }
                      }}
                    />
                  </label>
                  <div className="metadata-online-actions">
                    <button
                      className="metadata-button-primary"
                      type="button"
                      disabled={onlineBusy || isRateLimited("musicBrainz")}
                      onClick={() => void searchOnline()}
                    >
                      <MagnifyingGlass aria-hidden="true" size={17} />
                      {t("app.onlineSearchMusicBrainz")}
                    </button>
                  </div>
                </div>
              </>
            ) : !onlineStatus || onlineStatus.state === "notConfigured" ? (
              <p className="metadata-online-state">{t("app.onlineNotConfigured")}</p>
            ) : onlineStatus.state === "credentialUnavailable" ? (
              <p className="metadata-online-state is-error" role="alert">
                {t("app.onlineCredentialUnavailable")}
              </p>
            ) : onlineStatus.state === "authorizing" ? (
              <button
                type="button"
                onClick={() => void desktopApi.cancelOnlineMetadataLogin("spotify")}
              >
                {t("app.cancelEdit")}
              </button>
            ) : onlineStatus.state === "connected" ? (
              <>
                <div className="metadata-online-search-controls">
                  <label>
                    {t("app.onlineSearch")}
                    <input
                      value={onlineQuery}
                      maxLength={200}
                      onChange={(event) => setOnlineQuery(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          event.preventDefault();
                          void searchOnline();
                        }
                      }}
                    />
                  </label>
                  <div className="metadata-online-actions">
                    <button
                      className="metadata-button-primary"
                      type="button"
                      disabled={onlineBusy || isRateLimited("spotify")}
                      onClick={() => void searchOnline()}
                    >
                      <MagnifyingGlass aria-hidden="true" size={17} />
                      {t("app.onlineSearch")}
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        ++searchGeneration.current;
                        setOnlineResults([]);
                        setOnlineSearched(false);
                        setOnlineBusy(false);
                        setCoverBusyKey(null);
                        void desktopApi.disconnectOnlineMetadata("spotify").then(setOnlineStatus);
                      }}
                    >
                      {t("app.onlineDisconnect")}
                    </button>
                  </div>
                </div>
              </>
            ) : (
              <button
                className="metadata-button-primary metadata-connect-button"
                type="button"
                disabled={onlineBusy}
                onClick={connectOnline}
              >
                {onlineStatus.state === "reauthorizationRequired"
                  ? t("app.onlineReconnect")
                  : t("app.onlineConnect")}
              </button>
            )}
            {isRateLimited(onlineProvider) ? (
              <p className="metadata-online-state" role="status">
                {t(
                  onlineProvider === "musicBrainz"
                    ? "errors.musicbrainz_rate_limited"
                    : "errors.spotify_rate_limited",
                )}
              </p>
            ) : null}
            {onlineApplied ? (
              <p className="metadata-online-state is-success" role="status">
                <CheckCircle aria-hidden="true" size={17} />
                {t("app.onlineAppliedUnsaved")}
              </p>
            ) : null}
            {onlineError ? (
              <p className="metadata-online-state is-error" role="alert">
                {t(`errors.${onlineError}`, { defaultValue: t("app.error") })}
              </p>
            ) : null}
            {canShowOnlineResults ? (
              <section className="metadata-online-results" aria-label={t("app.onlineResults")}>
                <header className="metadata-results-heading">
                  <h4>{t("app.onlineResults")}</h4>
                  {onlineSearched && !isSearching ? (
                    <span>{t("app.onlineResultCount", { count: onlineResults.length })}</span>
                  ) : null}
                </header>
                {isSearching ? (
                  <div className="metadata-results-loading" role="status">
                    <span>{t("app.onlineSearching")}</span>
                    <div aria-hidden="true" />
                    <div aria-hidden="true" />
                    <div aria-hidden="true" />
                  </div>
                ) : onlineResults.length ? (
                  <div className="metadata-online-result-list" role="list">
                    {onlineResults.map(renderOnlineCandidate)}
                  </div>
                ) : onlineSearched && !onlineError ? (
                  <p className="metadata-results-empty">
                    {t(
                      onlineProvider === "musicBrainz"
                        ? "app.onlineNoResultsMusicBrainz"
                        : "app.onlineNoResultsSpotify",
                    )}
                  </p>
                ) : (
                  <div className="metadata-results-prompt">
                    <MagnifyingGlass aria-hidden="true" size={24} />
                    <p>{t("app.onlineSearchPrompt")}</p>
                  </div>
                )}
              </section>
            ) : null}
          </section>
        </div>
        <div className="metadata-actions" data-layout="actions-row">
          <button type="button" onClick={onClose}>
            {t("app.cancelEdit")}
          </button>
          {error ? (
            <button type="button" disabled={busy} onClick={() => void save()}>
              {t("app.retry")}
            </button>
          ) : null}
          <button
            className="metadata-button-primary"
            type="button"
            disabled={busy}
            onClick={() => void save()}
          >
            {t("app.save")}
          </button>
        </div>
      </section>
    </div>
  );
}
