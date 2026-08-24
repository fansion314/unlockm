import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ArtworkCandidate, HistoryItem } from "../../lib/contracts";
import { desktopApi } from "../../lib/ipc";
import { usePlayback } from "../playback/PlaybackProvider";

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
  const titleRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLElement>(null);
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
