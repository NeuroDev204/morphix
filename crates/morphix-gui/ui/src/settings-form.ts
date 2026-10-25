// Pure mapping between the settings form and the Settings the backend stores. No Tauri imports, so vitest can run it without a webview.
import { S } from "./strings";

export type QualityChoice = "small" | "balanced" | "high";

// Field names mirror the serde JSON of Rust `Settings`, which the backend reads and writes as-is.
export interface Settings {
  readonly quality: QualityChoice;
  readonly output_dir: string | null;
  readonly default_resize_percent: number;
}

// Strings in the form, so the input is text until it is validated.
export interface SettingsForm {
  readonly quality: QualityChoice;
  readonly outputDir: string;
  readonly resizePercent: string;
}

export type SettingsParse =
  | { readonly ok: true; readonly settings: Settings }
  | { readonly ok: false; readonly message: string };

// WHY same bounds as settings::validate in Rust: the form rejects first so the reason sits next to the field; the backend stays the authority.
const RESIZE_MIN: number = 1;
const RESIZE_MAX: number = 100;

export function formFromSettings(settings: Settings): SettingsForm {
  return {
    quality: settings.quality,
    outputDir: settings.output_dir ?? "",
    resizePercent: String(settings.default_resize_percent),
  };
}

export function settingsFromForm(form: SettingsForm): SettingsParse {
  const text: string = form.resizePercent.trim();
  // WHY digits only: Number("") is 0 and Number("12.5") is not a whole percent, so both must be refused explicitly.
  const percent: number = /^\d+$/.test(text) ? Number(text) : Number.NaN;
  if (!(percent >= RESIZE_MIN && percent <= RESIZE_MAX)) {
    return { ok: false, message: S.resizeRangeError };
  }
  return {
    ok: true,
    settings: {
      quality: form.quality,
      // WHY null for blank: null means "save next to the source file", the default the backend expects.
      output_dir: form.outputDir.trim() === "" ? null : form.outputDir,
      default_resize_percent: percent,
    },
  };
}
