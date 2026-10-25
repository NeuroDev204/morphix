import { hasMixedExtensions } from "./format";
import type { WheelItem, WheelPayload } from "./menu-types";
import { S } from "./strings";

export interface KhaySelection { paths: string[]; error: string | null; revision: number }
export type SelectionView =
  | { kind: "empty" }
  | { kind: "error"; message: string }
  | { kind: "files"; summary: string; names: string[]; extra: number; items: WheelItem[]; unsupported: string | null };
export const MAX_NAMES_SHOWN: number = 3;

export function composeSelectionView(selection: KhaySelection, payload: WheelPayload | null, payloadError: string | null): SelectionView {
  const error: string | null = selection.error ?? payloadError;
  if (error !== null) return { kind: "error", message: error };
  if (selection.paths.length === 0) return { kind: "empty" };
  const names: string[] = selection.paths.map((path: string): string => path.split(/[\\/]/).pop() ?? path);
  const items: WheelItem[] = [...(payload?.items ?? [])].sort((a: WheelItem, b: WheelItem): number => Number(a.is_tool) - Number(b.is_tool));
  return { kind: "files", summary: S.selectionTitle(names.length), names: names.slice(0, MAX_NAMES_SHOWN),
    extra: Math.max(0, names.length - MAX_NAMES_SHOWN), items,
    unsupported: payload !== null && items.length === 0 ? (hasMixedExtensions(names) ? S.emptyMixed : S.emptyUnsupported) : null };
}
