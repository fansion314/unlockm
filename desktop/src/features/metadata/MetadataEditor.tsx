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
// not mount it for MusicBrainz results: TheAudioDB lookup must begin only when
// the user chooses Use cover or Fill all.
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
  const searchGeneration = useRef(0);
  const onlineProviderRef = useRef<OnlineMetadataProvider>("musicBrainz");
  const mounted = useRef(true);
  const titleRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLElement>(null);
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
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
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
  }, [onClose]);
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
    // This is a local capability read, not a provider request. It lets Rust
    // keep the no-auth primary ordering authoritative while opening the editor
    // remains free of MusicBrainz, TheAudioDB, or Spotify network traffic.
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
  const applyText = (candidate: OnlineMetadataCandidate) => {
    setTitle(candidate.title);
    setArtist(candidate.artists.join(", "));
    setAlbum(candidate.album);
    setOnlineApplied(true);
  };
  const applyCover = async (candidate: OnlineMetadataCandidate) => {
    if (!candidate.coverHandle) return;
    const generation = searchGeneration.current;
    setOnlineBusy(true);
    setOnlineError(null);
    try {
      const bytes = await desktopApi.getOnlineMetadataCoverPreview(
        candidate.provider,
        candidate.coverHandle,
      );
      if (!mounted.current || generation !== searchGeneration.current) return;
      if (onlineProviderRef.current !== candidate.provider) return;
      setCover(new Uint8Array(bytes));
      setExistingArtworkId(undefined);
      setOnlineApplied(true);
    } catch (reason) {
      if (
        mounted.current &&
        generation === searchGeneration.current &&
        onlineProviderRef.current === candidate.provider
      ) {
        setOnlineError(onlineErrorKey(candidate.provider, reason));
      }
    } finally {
      if (
        mounted.current &&
        generation === searchGeneration.current &&
        onlineProviderRef.current === candidate.provider
      ) {
        setOnlineBusy(false);
      }
    }
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
          <h2>{t("app.editMetadata")}</h2>
        </header>
        <div
          className="metadata-modal-body"
          data-layout="vertical-scroll"
          style={{ display: "grid", overflowX: "hidden" }}
          tabIndex={-1}
        >
          <label>
            {t("app.title")}
            <input
              ref={titleRef}
              value={title}
              maxLength={512}
              onChange={(event) => setTitle(event.target.value)}
            />
          </label>
          <section className="metadata-online" aria-label={t("app.onlineMetadata")}>
            <h3>{t("app.onlineMetadata")}</h3>
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
                <p>{t("app.onlineMusicBrainzHint")}</p>
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
                    type="button"
                    disabled={
                      onlineBusy ||
                      Boolean(
                        onlineStatus?.provider === "musicBrainz" &&
                        onlineStatus.retryAtMs &&
                        onlineStatus.retryAtMs > Date.now(),
                      )
                    }
                    onClick={() => void searchOnline()}
                  >
                    {t("app.onlineSearchMusicBrainz")}
                  </button>
                </div>
                {onlineResults.map((candidate) => (
                  <article
                    className="metadata-online-result"
                    key={`${candidate.itemId}-${candidate.album}`}
                  >
                    <div>
                      <strong>{candidate.title}</strong>
                      <small>
                        {candidate.artists.join(", ")} · {candidate.album} ·{" "}
                        {Math.round(candidate.durationMs / 1000)}s · {candidate.attribution}
                      </small>
                    </div>
                    <div>
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
                        type="button"
                        disabled={onlineBusy}
                        onClick={() => {
                          applyText(candidate);
                          void applyCover(candidate);
                        }}
                      >
                        {t("app.onlineApplyAll")}
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          void desktopApi.openOnlineMetadataResult(
                            candidate.provider,
                            candidate.itemId,
                          )
                        }
                      >
                        {t("app.onlineOpenMusicBrainz")}
                      </button>
                    </div>
                  </article>
                ))}
                {onlineSearched && !onlineBusy && onlineResults.length === 0 && !onlineError ? (
                  <small>{t("app.onlineNoResultsMusicBrainz")}</small>
                ) : null}
                {onlineStatus?.provider === "musicBrainz" &&
                onlineStatus.retryAtMs &&
                onlineStatus.retryAtMs > Date.now() ? (
                  <small role="status">{t("errors.musicbrainz_rate_limited")}</small>
                ) : null}
              </>
            ) : !onlineStatus || onlineStatus.state === "notConfigured" ? (
              <p>{t("app.onlineNotConfigured")}</p>
            ) : onlineStatus.state === "credentialUnavailable" ? (
              <p role="alert">{t("app.onlineCredentialUnavailable")}</p>
            ) : onlineStatus.state === "authorizing" ? (
              <button
                type="button"
                onClick={() => void desktopApi.cancelOnlineMetadataLogin("spotify")}
              >
                {t("app.cancelEdit")}
              </button>
            ) : onlineStatus.state === "connected" ? (
              <>
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
                    type="button"
                    disabled={
                      onlineBusy ||
                      Boolean(
                        onlineStatus?.provider === "spotify" &&
                        onlineStatus.retryAtMs &&
                        onlineStatus.retryAtMs > Date.now(),
                      )
                    }
                    onClick={() => void searchOnline()}
                  >
                    {t("app.onlineSearch")}
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      ++searchGeneration.current;
                      setOnlineResults([]);
                      setOnlineSearched(false);
                      setOnlineBusy(false);
                      void desktopApi.disconnectOnlineMetadata("spotify").then(setOnlineStatus);
                    }}
                  >
                    {t("app.onlineDisconnect")}
                  </button>
                </div>
                {onlineResults.map((candidate) => (
                  <article className="metadata-online-result" key={candidate.itemId}>
                    {candidate.coverHandle ? <OnlineCover candidate={candidate} /> : null}
                    <div>
                      <strong>{candidate.title}</strong>
                      <small>
                        {candidate.artists.join(", ")} · {candidate.album} ·{" "}
                        {Math.round(candidate.durationMs / 1000)}s · {candidate.attribution}
                      </small>
                    </div>
                    <div>
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
                        type="button"
                        disabled={onlineBusy}
                        onClick={() => {
                          applyText(candidate);
                          void applyCover(candidate);
                        }}
                      >
                        {t("app.onlineApplyAll")}
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          void desktopApi.openOnlineMetadataResult(
                            candidate.provider,
                            candidate.itemId,
                          )
                        }
                      >
                        {t("app.onlineOpenSpotify")}
                      </button>
                    </div>
                  </article>
                ))}
                {onlineSearched && !onlineBusy && onlineResults.length === 0 && !onlineError ? (
                  <small>{t("app.onlineNoResultsSpotify")}</small>
                ) : null}
              </>
            ) : (
              <button type="button" disabled={onlineBusy} onClick={connectOnline}>
                {onlineStatus.state === "reauthorizationRequired"
                  ? t("app.onlineReconnect")
                  : t("app.onlineConnect")}
              </button>
            )}
            {onlineApplied ? <p role="status">{t("app.onlineAppliedUnsaved")}</p> : null}
            {onlineError ? (
              <p role="alert">{t(`errors.${onlineError}`, { defaultValue: t("app.error") })}</p>
            ) : null}
          </section>
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
                const nextPreview = URL.createObjectURL(file);
                setPreview((current) => {
                  if (current) URL.revokeObjectURL(current);
                  return nextPreview;
                });
                void file.arrayBuffer().then((bytes) => setCover(new Uint8Array(bytes)));
              }}
            />
          </label>
          {preview ? <img className="metadata-cover-preview" src={preview} alt="" /> : null}
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
                    }}
                  >
                    <CandidateArtwork candidate={candidate} />
                    <small>{candidate.album || candidate.title || `#${candidate.artworkId}`}</small>
                  </button>
                ))}
              </div>
            ) : (
              <small>{t("app.noCoverCandidates")}</small>
            )}
          </div>
          {error ? <p role="alert">{error}</p> : null}
        </div>
        <div className="metadata-actions" data-layout="actions-row" style={{ display: "flex" }}>
          <button type="button" onClick={onClose}>
            {t("app.cancelEdit")}
          </button>
          {error ? (
            <button type="button" disabled={busy} onClick={() => void save()}>
              {t("app.retry")}
            </button>
          ) : null}
          <button type="button" disabled={busy} onClick={() => void save()}>
            {t("app.save")}
          </button>
        </div>
      </section>
    </div>
  );
}
