import { describe, it, expect } from "vitest";
import { composeSelectionView, type KhaySelection } from "../src/selection";
import { S } from "../src/strings";

const empty: KhaySelection = { paths: [], error: null, revision: 0 };
describe("Khay selection", () => {
  it("empty_when_no_paths_and_no_error", () => {
    expect(composeSelectionView(empty, null, null)).toEqual({ kind: "empty" });
  });
  it("error_wins_and_keeps_empty_drop_zone", () => {
    expect(composeSelectionView({ ...empty, paths: ["a.jpg"], error: "lỗi" }, null, null)).toEqual({ kind: "error", message: "lỗi" });
  });
  it("shows_three_names_and_extra_count", () => {
    const view = composeSelectionView({ ...empty, paths: Array.from({ length: 53 }, (_, i) => `/tmp/${i}.jpg`) }, null, null);
    expect(view).toMatchObject({ kind: "files", names: ["0.jpg", "1.jpg", "2.jpg"], extra: 50 });
  });
  it("no_items_reports_mixed_or_unsupported_reason", () => {
    const payload = { files: [], items: [], total_bytes: 0 };
    expect(composeSelectionView({ ...empty, paths: ["a.jpg", "b.docx"] }, payload, null)).toMatchObject({ unsupported: S.emptyMixed });
    expect(composeSelectionView({ ...empty, paths: ["a.bin"] }, payload, null)).toMatchObject({ unsupported: S.emptyUnsupported });
  });
  it("payload_error_becomes_error_view", () => {
    expect(composeSelectionView({ ...empty, paths: ["a.jpg"] }, null, "missing")).toEqual({ kind: "error", message: "missing" });
  });
});
