import {
  ArrowLineLeft,
  ArrowLineRight,
  Pause,
  Play,
  SpeakerHigh,
  SpeakerSlash,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { formatDuration } from "../../lib/format";
import { usePlayback } from "./PlaybackProvider";
import { desktopApi } from "../../lib/ipc";
import "../../styles/player.css";

function PlayerArtwork({
  artworkId,
  title,
  playing,
}: {
  artworkId: number | null;
  title: string;
  playing: boolean;
}) {
  const [loaded, setLoaded] = useState<{ artworkId: number; url: string } | null>(null);
  useEffect(() => {
    let active = true;
    let objectUrl: string | null = null;
    if (artworkId === null)
      return () => {
        active = false;
      };
    void Promise.all([desktopApi.getArtworkMime(artworkId), desktopApi.getArtwork(artworkId)])
      .then(([metadata, bytes]) => {
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: metadata.mimeType }));
        if (active) setLoaded({ artworkId, url: objectUrl });
        else URL.revokeObjectURL(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [artworkId]);
  const url = loaded?.artworkId === artworkId ? loaded.url : null;
  return url ? (
    <img
      className={`player-cover ${playing ? "is-playing" : ""}`}
      style={{ animationPlayState: playing ? "running" : "paused" }}
      src={url}
      alt=""
    />
  ) : (
    <span className="player-cover player-cover-placeholder" aria-hidden="true">
      {title.slice(0, 1) || "♪"}
    </span>
  );
}

export function PersistentPlayer() {
  const { i18n, t } = useTranslation();
  const playback = usePlayback();
  const [collapsed, setCollapsed] = useState(false);
  if (!playback.track) return null;
  const title = playback.track.title || playback.track.relativePath;
  return (
    <aside
      className={`persistent-player ${collapsed ? "is-collapsed" : ""}`}
      data-collapsed={collapsed}
      aria-label={t("app.playSong", { title })}
    >
      <div className="collapsed-artwork">
        <PlayerArtwork
          key={playback.track.artworkId ?? `empty-${playback.track.id}`}
          artworkId={playback.track.artworkId}
          title={title}
          playing={playback.playing}
        />
        <button
          className="play-button collapsed-play"
          aria-hidden={!collapsed}
          aria-label={
            playback.playing ? t("app.pauseSong", { title }) : t("app.playSong", { title })
          }
          title={playback.playing ? t("app.pauseSong", { title }) : t("app.playSong", { title })}
          disabled={!collapsed}
          tabIndex={collapsed ? 0 : -1}
          type="button"
          onClick={() => void playback.toggle(playback.track!)}
        >
          {playback.playing ? <Pause weight="fill" /> : <Play weight="fill" />}
        </button>
      </div>
      <div
        className={`player-expanded ${collapsed ? "is-collapsed-content" : ""}`}
        aria-hidden={collapsed}
        inert={collapsed}
      >
        <div className="persistent-copy">
          <strong>{title}</strong>
          <span>
            {[playback.track.artist, playback.track.album].filter(Boolean).join(" · ") || "-"}
          </span>
        </div>
        <div className="player-progress">
          <button
            className="play-button"
            aria-label={
              playback.playing ? t("app.pauseSong", { title }) : t("app.playSong", { title })
            }
            title={playback.playing ? t("app.pauseSong", { title }) : t("app.playSong", { title })}
            type="button"
            onClick={() => void playback.toggle(playback.track!)}
          >
            {playback.playing ? <Pause weight="fill" /> : <Play weight="fill" />}
          </button>
          <input
            aria-label={t("app.seekSong", { title })}
            type="range"
            min={0}
            max={Math.max(playback.duration, 1)}
            step={0.1}
            value={Math.min(playback.currentTime, Math.max(playback.duration, 1))}
            onChange={(event) => playback.seek(Number(event.target.value))}
          />
          <span className="player-time">
            {formatDuration(playback.currentTime * 1000, i18n.language)} /{" "}
            {formatDuration(playback.duration * 1000, i18n.language)}
          </span>
        </div>
        <button
          className="player-mute"
          type="button"
          aria-label={playback.muted ? t("app.unmute") : t("app.mute")}
          onClick={playback.toggleMute}
        >
          {playback.muted ? (
            <SpeakerSlash aria-hidden="true" />
          ) : (
            <SpeakerHigh aria-hidden="true" />
          )}
        </button>
        <input
          aria-label={t("app.volume")}
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={playback.volume}
          onChange={(event) => playback.setVolume(Number(event.target.value))}
        />
        {playback.error ? (
          <span className="player-error" role="status">
            {t("app.playbackUnavailable")}
          </span>
        ) : null}
      </div>
      <button
        className="icon-button player-collapse-toggle"
        data-collapse-direction={collapsed ? "expand-right" : "collapse-left"}
        type="button"
        onClick={() => setCollapsed((value) => !value)}
        aria-label={collapsed ? t("app.expandPlayer") : t("app.collapsePlayer")}
        title={collapsed ? t("app.expandPlayer") : t("app.collapsePlayer")}
      >
        {collapsed ? (
          <ArrowLineRight aria-hidden="true" size={19} weight="bold" />
        ) : (
          <ArrowLineLeft aria-hidden="true" size={19} weight="bold" />
        )}
      </button>
    </aside>
  );
}
