import {
  ArrowRight,
  FileAudio,
  FolderOpen,
  FolderSimple,
  ImageSquare,
  UploadSimple,
} from "@phosphor-icons/react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { RunSettings, ValidatedInput } from "../../lib/contracts";
import { singleDroppedPath } from "./drop";

type ComposerProps = {
  selected: ValidatedInput | null;
  outputPath: string | null;
  settings: RunSettings;
  busy: boolean;
  onSelectPath: (this: void, path: string) => Promise<void>;
  onOutputPath: (this: void, path: string | null) => void;
  onSettings: (this: void, settings: RunSettings) => void;
  onConvert: (this: void) => void;
  onError: (this: void, detail: string) => void;
};

function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

export function Composer({
  selected,
  outputPath,
  settings,
  busy,
  onSelectPath,
  onOutputPath,
  onSettings,
  onConvert,
  onError,
}: ComposerProps) {
  const { t } = useTranslation();
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (disposed) return;
        if (event.payload.type === "enter") setDragging(true);
        if (event.payload.type === "leave") setDragging(false);
        if (event.payload.type === "drop") {
          setDragging(false);
          const path = singleDroppedPath(event.payload.paths);
          if (!path) {
            onError(t("composer.invalidMany"));
            return;
          }
          void onSelectPath(path);
        }
      })
      .then((listener) => {
        if (disposed) listener();
        else unlisten = listener;
      })
      .catch((error: unknown) => onError(String(error)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onError, onSelectPath, t]);

  const choose = async (directory: boolean) => {
    try {
      const path = await open({ directory, multiple: false });
      if (typeof path === "string") await onSelectPath(path);
    } catch (error) {
      onError(String(error));
    }
  };

  const chooseOutput = async () => {
    try {
      const path = await open({ directory: true, multiple: false });
      if (typeof path === "string") onOutputPath(path);
    } catch (error) {
      onError(String(error));
    }
  };

  const updateSettings = (next: Partial<RunSettings>) =>
    onSettings({ ...settings, ...next, overwrite: false });

  return (
    <section className="composer" aria-label={t("composer.regionLabel")}>
      <div
        className={`drop-field ${dragging ? "is-dragging" : ""} ${selected ? "has-selection" : ""}`}
      >
        <span className="drop-glyph" aria-hidden="true">
          <UploadSimple size={30} weight="light" />
        </span>
        <div className="drop-copy">
          <strong>{selected ? basename(selected.path) : t("composer.drop")}</strong>
          <span>{selected ? selected.path : t("composer.dropHint")}</span>
        </div>
        <div className="choice-row">
          <button type="button" onClick={() => void choose(false)} disabled={busy}>
            <FileAudio aria-hidden="true" size={17} />
            {t("composer.chooseFile")}
          </button>
          <button type="button" onClick={() => void choose(true)} disabled={busy}>
            <FolderSimple aria-hidden="true" size={17} />
            {t("composer.chooseFolder")}
          </button>
        </div>
      </div>

      <div className="composer-actions">
        <button
          aria-label={t("composer.chooseOutput")}
          className="output-button"
          type="button"
          onClick={() => void chooseOutput()}
          disabled={busy}
        >
          <FolderOpen aria-hidden="true" size={25} weight="duotone" />
          <span className="output-copy">
            <small>{t("composer.output")}</small>
            <strong>{outputPath ? basename(outputPath) : t("composer.outputDefault")}</strong>
            {outputPath ? <span>{outputPath}</span> : null}
          </span>
        </button>

        <div className="settings-panel" aria-label={t("composer.settings")}>
          <label className="setting-row" title={t("composer.metadataHint")}>
            <span className="setting-icon" aria-hidden="true">
              <ImageSquare size={18} />
            </span>
            <span>{t("composer.metadata")}</span>
            <input
              checked={settings.updateMetadata}
              disabled={busy}
              type="checkbox"
              onChange={(event) => updateSettings({ updateMetadata: event.target.checked })}
            />
          </label>
          {selected?.kind === "directory" ? (
            <label className="setting-row">
              <span className="setting-icon" aria-hidden="true">
                <FolderSimple size={18} />
              </span>
              <span>{t("composer.copy")}</span>
              <input
                checked={settings.copyPassthrough}
                disabled={busy}
                type="checkbox"
                onChange={(event) => updateSettings({ copyPassthrough: event.target.checked })}
              />
            </label>
          ) : null}
        </div>

        <button
          className="convert-button"
          disabled={!selected || busy}
          type="button"
          onClick={onConvert}
        >
          <span>{busy ? t("composer.busy") : t("composer.convert")}</span>
          <ArrowRight aria-hidden="true" size={20} weight="bold" />
        </button>
      </div>
    </section>
  );
}
