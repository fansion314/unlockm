import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
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
    if (!isTauri()) {
      return;
    }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (disposed) {
          return;
        }
        if (event.payload.type === "enter") {
          setDragging(true);
        }
        if (event.payload.type === "leave") {
          setDragging(false);
        }
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
        if (disposed) {
          listener();
        } else {
          unlisten = listener;
        }
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
      if (typeof path === "string") {
        await onSelectPath(path);
      }
    } catch (error) {
      onError(String(error));
    }
  };

  const chooseOutput = async () => {
    try {
      const path = await open({ directory: true, multiple: false });
      if (typeof path === "string") {
        onOutputPath(path);
      }
    } catch (error) {
      onError(String(error));
    }
  };

  return (
    <section className="composer" aria-labelledby="composer-title">
      <div className="composer-head">
        <p className="eyebrow">{t("composer.eyebrow")}</p>
        <h1 id="composer-title">{t("composer.title")}</h1>
        <p>{t("composer.description")}</p>
      </div>
      <div
        className={`drop-field ${dragging ? "is-dragging" : ""} ${selected ? "has-selection" : ""}`}
      >
        <span className="drop-glyph" aria-hidden="true">
          ↟
        </span>
        <strong>{selected ? selected.path.split(/[\\/]/).pop() : t("composer.drop")}</strong>
        <span>{selected ? t("composer.selected") : t("composer.description")}</span>
        <div className="choice-row">
          <button type="button" onClick={() => void choose(false)} disabled={busy}>
            {t("composer.chooseFile")}
          </button>
          <button type="button" onClick={() => void choose(true)} disabled={busy}>
            {t("composer.chooseFolder")}
          </button>
        </div>
      </div>
      <div className="settings-panel">
        <div className="settings-title">
          <span>{t("composer.settings")}</span>
          <button type="button" onClick={() => void chooseOutput()} disabled={busy}>
            {t("composer.chooseOutput")}
          </button>
        </div>
        <p className="output-path">{outputPath ?? t("composer.outputDefault")}</p>
        <label className="setting-row">
          <input
            checked={settings.updateMetadata}
            disabled={busy}
            type="checkbox"
            onChange={(event) => onSettings({ ...settings, updateMetadata: event.target.checked })}
          />
          <span>
            {t("composer.metadata")}
            <small>{t("composer.metadataHint")}</small>
          </span>
        </label>
        {selected?.kind === "directory" ? (
          <label className="setting-row">
            <input
              checked={settings.copyPassthrough}
              disabled={busy}
              type="checkbox"
              onChange={(event) =>
                onSettings({ ...settings, copyPassthrough: event.target.checked })
              }
            />
            <span>{t("composer.copy")}</span>
          </label>
        ) : null}
        <label className="setting-row">
          <input
            checked={settings.overwrite}
            disabled={busy}
            type="checkbox"
            onChange={(event) => onSettings({ ...settings, overwrite: event.target.checked })}
          />
          <span>
            {t("composer.overwrite")}
            <small>{t("composer.overwriteHint")}</small>
          </span>
        </label>
      </div>
      <button
        className="convert-button"
        disabled={!selected || busy}
        type="button"
        onClick={onConvert}
      >
        {t("composer.convert")}
      </button>
    </section>
  );
}
