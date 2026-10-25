import { S } from "./strings";

export type IntegrationStatus =
  | "installed"
  | "already_installed"
  | "removed"
  | "already_absent"
  | "missing"
  | "stale"
  | "conflict"
  | "unsupported"
  | "unknown"
  | "not_applicable"
  | "failed";

export interface IntegrationRow {
  readonly id: string;
  readonly label: string;
  readonly status: IntegrationStatus;
  readonly statusLabel: string;
  readonly message: string;
}

export interface IntegrationView {
  readonly rows: readonly IntegrationRow[];
  readonly guidance: string | null;
}

const desktopComponents = [
  ["nautilus_extension", "Files"],
  ["desktop_context_menu", "Desktop"],
  ["application_launcher", "Ứng dụng Morphix"],
] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStatus(value: unknown): value is IntegrationStatus {
  return typeof value === "string" && Object.prototype.hasOwnProperty.call(S.integrationStatusLabels, value);
}

export function integrationViewFromDoctor(report: unknown): IntegrationView {
  const integration: unknown = isRecord(report) ? report.integration : null;
  const components: unknown = isRecord(integration) ? integration.components : null;
  if (!Array.isArray(components)) {
    throw new Error(S.integrationReportMissing);
  }

  const rows: IntegrationRow[] = desktopComponents.map(([id, label]) => {
    const component: unknown = components.find(
      (candidate: unknown): boolean => isRecord(candidate) && candidate.id === id,
    );
    const status: IntegrationStatus = isRecord(component) && isStatus(component.status)
      ? component.status
      : "unknown";
    const message: string = isRecord(component) && typeof component.message === "string"
      ? component.message
      : S.integrationComponentMissing;
    return { id, label, status, statusLabel: S.integrationStatusLabels[status], message };
  });
  const needsRepair: boolean = rows.some((row) =>
    ["missing", "stale", "conflict", "unknown", "failed"].includes(row.status),
  );
  return {
    rows,
    guidance: needsRepair ? S.integrationRepair : null,
  };
}
