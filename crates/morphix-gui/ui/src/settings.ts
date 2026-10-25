// Settings window: quality, default output folder and default resize percent. Saves go through the backend's validated save_settings.
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  formFromSettings,
  settingsFromForm,
  type QualityChoice,
  type Settings,
  type SettingsForm,
} from "./settings-form";
import { S } from "./strings";

interface QualityOption {
  readonly value: QualityChoice;
  readonly label: string;
}

const QUALITY_OPTIONS: readonly QualityOption[] = [
  { value: "small", label: S.qualitySmall },
  { value: "balanced", label: S.qualityBalanced },
  { value: "high", label: S.qualityHigh },
];

// WHY draft apart from saved: the page only adopts a value once the backend confirms the write, so a failed save never shows what is not on disk.
let savedSettings: Settings | null = null;
let draft: SettingsForm | null = null;
let errorMessage: string | null = null;
let savedNotice: boolean = false;

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string = "",
  text: string = "",
): HTMLElementTagNameMap[K] {
  const node: HTMLElementTagNameMap[K] = document.createElement(tag);
  if (className !== "") {
    node.className = className;
  }
  // WHY textContent: folder paths are user data and must never be parsed as HTML.
  node.textContent = text;
  return node;
}

function button(label: string, className: string, onClick: () => void): HTMLButtonElement {
  const node: HTMLButtonElement = el("button", className, label);
  node.type = "button";
  node.addEventListener("click", onClick);
  return node;
}

function showError(message: string): void {
  errorMessage = message;
  render();
}

function qualityRail(current: QualityChoice): HTMLDivElement {
  const rail: HTMLDivElement = el("div", "segmented");
  rail.setAttribute("role", "tablist");
  for (const option of QUALITY_OPTIONS) {
    const tab: HTMLButtonElement = button(option.label, "", () => {
      if (draft === null) {
        return;
      }
      draft = { ...draft, quality: option.value };
      savedNotice = false;
      render();
    });
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-selected", String(option.value === current));
    rail.append(tab);
  }
  return rail;
}

function folderRow(form: SettingsForm): HTMLDivElement {
  const row: HTMLDivElement = el("div", "settings-folder");
  row.append(el("p", "settings-path", form.outputDir === "" ? S.keepInSource : form.outputDir));
  const actions: HTMLDivElement = el("div", "settings-actions");
  actions.append(
    button(S.chooseFolder, "btn btn-outline", () => {
      void chooseFolder();
    }),
  );
  if (form.outputDir !== "") {
    actions.append(
      button(S.resetFolder, "btn btn-ghost", () => {
        if (draft === null) {
          return;
        }
        draft = { ...draft, outputDir: "" };
        savedNotice = false;
        render();
      }),
    );
  }
  row.append(actions);
  return row;
}

function percentField(form: SettingsForm): HTMLLabelElement {
  const field: HTMLLabelElement = el("label", "settings-field");
  field.append(el("span", "settings-label", S.resizeLabel));
  const line: HTMLDivElement = el("div", "settings-percent");
  const input: HTMLInputElement = el("input", "field-input");
  input.type = "number";
  input.min = "1";
  input.max = "100";
  input.step = "1";
  input.value = form.resizePercent;
  // WHY no render here: re-rendering on each keystroke would drop focus from the input.
  input.addEventListener("input", () => {
    if (draft === null) {
      return;
    }
    draft = { ...draft, resizePercent: input.value };
    savedNotice = false;
  });
  line.append(input, el("span", "settings-suffix", S.percentSuffix));
  field.append(line);
  return field;
}

async function chooseFolder(): Promise<void> {
  try {
    const picked: string | null = await open({ directory: true, multiple: false });
    if (picked === null || draft === null) {
      return;
    }
    draft = { ...draft, outputDir: picked };
    savedNotice = false;
    render();
  } catch (error: unknown) {
    console.error("không mở được hộp thoại chọn thư mục", error);
    showError(describeError(error));
  }
}

async function save(): Promise<void> {
  if (draft === null) {
    return;
  }
  const parsed = settingsFromForm(draft);
  if (!parsed.ok) {
    showError(parsed.message);
    return;
  }
  try {
    await invoke<void>("save_settings", { settings: parsed.settings });
  } catch (error: unknown) {
    console.error("không lưu được cài đặt", error);
    showError(describeError(error));
    return;
  }
  savedSettings = parsed.settings;
  draft = formFromSettings(parsed.settings);
  errorMessage = null;
  savedNotice = true;
  render();
}

function render(): void {
  const app: HTMLElement | null = document.getElementById("app");
  if (app === null) {
    throw new Error("thiếu phần tử #app trong settings.html");
  }
  app.replaceChildren();
  app.className = "settings-shell";
  app.append(el("h1", "settings-title", S.settingsTitle));

  if (draft === null || savedSettings === null) {
    app.append(el("p", "empty-state", S.settingsLoading));
  } else {
    const form: SettingsForm = draft;
    const quality: HTMLDivElement = el("div", "settings-field");
    quality.append(el("span", "settings-label", S.qualityLabel), qualityRail(form.quality));
    const folder: HTMLDivElement = el("div", "settings-field");
    folder.append(el("span", "settings-label", S.outputDirLabel), folderRow(form));
    app.append(quality, folder, percentField(form));
    const footer: HTMLDivElement = el("div", "settings-footer");
    footer.append(
      button(S.saveSettings, "btn btn-primary", () => {
        void save();
      }),
      el("p", "settings-saved", savedNotice ? S.settingsSaved : ""),
    );
    app.append(footer);
  }

  if (errorMessage !== null) {
    app.append(el("p", "banner-error", errorMessage));
  }
}

async function start(): Promise<void> {
  const settings: Settings = await invoke<Settings>("get_settings");
  savedSettings = settings;
  draft = formFromSettings(settings);
  render();
}

start().catch((error: unknown) => {
  console.error("không tải được cài đặt", error);
  showError(describeError(error));
});
