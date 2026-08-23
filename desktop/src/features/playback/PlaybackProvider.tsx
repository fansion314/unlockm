import { convertFileSrc } from "@tauri-apps/api/core";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type MutableRefObject,
} from "react";
import type { HistoryItem } from "../../lib/contracts";

type PlaybackState = {
  activeItemId: number | null;
  track: HistoryItem | null;
  playing: boolean;
  currentTime: number;
  duration: number;
  volume: number;
  muted: boolean;
  error: string | null;
  setVolume: (volume: number) => void;
  toggleMute: () => void;
  toggle: (item: HistoryItem) => Promise<void>;
  seek: (time: number) => void;
  refreshTrack: (track: HistoryItem) => void;
  releaseForItem: (itemId: number) => { track: HistoryItem; wasPlaying: boolean } | null;
  releaseForRun: (runId: number) => { track: HistoryItem; wasPlaying: boolean } | null;
  releaseAll: () => { track: HistoryItem; wasPlaying: boolean } | null;
  restoreReleased: (
    snapshot: { track: HistoryItem; wasPlaying: boolean } | null,
    next: HistoryItem,
  ) => Promise<void>;
};
export type PlaybackController = Pick<PlaybackState, "releaseForRun" | "releaseAll">;

const PlaybackContext = createContext<PlaybackState | null>(null);
const clampVolume = (value: number) =>
  Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0.6));

export function PlaybackProvider({
  children,
  volume = 0.6,
  onVolumeChange,
  controllerRef,
}: {
  children: ReactNode;
  volume?: number;
  onVolumeChange?: (volume: number) => void;
  controllerRef?: MutableRefObject<PlaybackController | null>;
}) {
  const audio = useRef<HTMLAudioElement>(null);
  const volumeTimer = useRef<number | null>(null);
  const lastAudibleVolume = useRef(clampVolume(volume) || 0.6);
  const onVolumeChangeRef = useRef(onVolumeChange);
  const [activeItemId, setActiveItemId] = useState<number | null>(null);
  const [track, setTrack] = useState<HistoryItem | null>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [currentVolume, setCurrentVolume] = useState(() => clampVolume(volume));
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    onVolumeChangeRef.current = onVolumeChange;
  }, [onVolumeChange]);
  useEffect(() => {
    const next = clampVolume(volume);
    if (next > 0) lastAudibleVolume.current = next;
    if (audio.current) audio.current.volume = next;
    setCurrentVolume(next);
  }, [volume]);
  useEffect(
    () => () => {
      if (volumeTimer.current !== null) window.clearTimeout(volumeTimer.current);
    },
    [],
  );
  const persistVolume = useCallback((next: number) => {
    if (volumeTimer.current !== null) window.clearTimeout(volumeTimer.current);
    volumeTimer.current = window.setTimeout(() => {
      volumeTimer.current = null;
      onVolumeChangeRef.current?.(next);
    }, 220);
  }, []);
  const setVolume = useCallback(
    (next: number) => {
      const clamped = clampVolume(next);
      if (audio.current) audio.current.volume = clamped;
      if (clamped > 0) lastAudibleVolume.current = clamped;
      setCurrentVolume(clamped);
      persistVolume(clamped);
    },
    [persistVolume],
  );
  const toggleMute = useCallback(() => {
    setVolume(currentVolume > 0 ? 0 : lastAudibleVolume.current || 0.6);
  }, [currentVolume, setVolume]);
  const toggle = useCallback(
    async (item: HistoryItem) => {
      const element = audio.current;
      if (!element) return;
      if (activeItemId === item.id) {
        if (element.paused) await element.play();
        else element.pause();
        return;
      }
      element.pause();
      // Set volume before setting src/play: native HTMLAudio defaults to 1.0.
      element.volume = currentVolume;
      setError(null);
      element.src = convertFileSrc(String(item.id), "unlockm-audio");
      element.load();
      setActiveItemId(item.id);
      setTrack(item);
      setCurrentTime(0);
      setDuration((item.durationMs ?? 0) / 1_000);
      try {
        await element.play();
      } catch (reason) {
        setPlaying(false);
        setError(String(reason));
        throw reason;
      }
    },
    [activeItemId, currentVolume],
  );
  const seek = useCallback((time: number) => {
    if (audio.current) audio.current.currentTime = time;
    setCurrentTime(time);
  }, []);
  const refreshTrack = useCallback(
    (next: HistoryItem) => {
      if (activeItemId !== next.id) return;
      const element = audio.current;
      element?.pause();
      setPlaying(false);
      setTrack(next);
    },
    [activeItemId],
  );
  const releaseAll = useCallback(() => {
    const snapshot = track ? { track, wasPlaying: playing } : null;
    const element = audio.current;
    element?.pause();
    element?.removeAttribute("src");
    element?.load();
    setPlaying(false);
    setActiveItemId(null);
    setTrack(null);
    setCurrentTime(0);
    setDuration(0);
    return snapshot;
  }, [playing, track]);
  const releaseForItem = useCallback(
    (itemId: number) => (activeItemId === itemId ? releaseAll() : null),
    [activeItemId, releaseAll],
  );
  const releaseForRun = useCallback(
    (runId: number) => (track?.runId === runId ? releaseAll() : null),
    [releaseAll, track?.runId],
  );
  const restoreReleased = useCallback(
    async (snapshot: { track: HistoryItem; wasPlaying: boolean } | null, next: HistoryItem) => {
      if (!snapshot) return;
      const element = audio.current;
      if (!element) return;
      element.pause();
      element.volume = currentVolume;
      element.src = convertFileSrc(String(next.id), "unlockm-audio");
      element.load();
      setActiveItemId(next.id);
      setTrack(next);
      setCurrentTime(0);
      setDuration((next.durationMs ?? 0) / 1_000);
      setError(null);
      if (snapshot.wasPlaying) {
        try {
          await element.play();
        } catch (reason) {
          setError(String(reason));
        }
      }
    },
    [currentVolume],
  );
  const value = useMemo(
    () => ({
      activeItemId,
      track,
      playing,
      currentTime,
      duration,
      volume: currentVolume,
      muted: currentVolume === 0,
      error,
      setVolume,
      toggleMute,
      toggle,
      seek,
      refreshTrack,
      releaseForItem,
      releaseForRun,
      releaseAll,
      restoreReleased,
    }),
    [
      activeItemId,
      track,
      playing,
      currentTime,
      duration,
      currentVolume,
      error,
      setVolume,
      toggleMute,
      toggle,
      seek,
      refreshTrack,
      releaseForItem,
      releaseForRun,
      releaseAll,
      restoreReleased,
    ],
  );
  useEffect(() => {
    if (!controllerRef) return;
    controllerRef.current = { releaseForRun, releaseAll };
    return () => {
      controllerRef.current = null;
    };
  }, [controllerRef, releaseAll, releaseForRun]);
  return (
    <PlaybackContext.Provider value={value}>
      {children}
      <audio
        className="audio-engine"
        ref={audio}
        onDurationChange={(event) => setDuration(event.currentTarget.duration || 0)}
        onTimeUpdate={(event) => setCurrentTime(event.currentTarget.currentTime)}
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onEnded={() => setPlaying(false)}
        onError={() => setError("playback_unavailable")}
      />
    </PlaybackContext.Provider>
  );
}

// eslint-disable-next-line react-refresh/only-export-components
export function usePlayback(): PlaybackState {
  const value = useContext(PlaybackContext);
  return (
    value ?? {
      activeItemId: null,
      track: null,
      playing: false,
      currentTime: 0,
      duration: 0,
      volume: 0.6,
      muted: false,
      error: null,
      setVolume: () => undefined,
      toggleMute: () => undefined,
      toggle: async () => undefined,
      seek: () => undefined,
      refreshTrack: () => undefined,
      releaseForItem: () => null,
      releaseForRun: () => null,
      releaseAll: () => null,
      restoreReleased: async () => undefined,
    }
  );
}
