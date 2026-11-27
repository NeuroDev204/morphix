import test from "node:test";
import assert from "node:assert/strict";
import { shouldShowAd, isAdConfigured, adScriptSrc } from "./ads.ts";

test("shouldShowAd", () => {
  assert.equal(shouldShowAd(false, 0), false);
  assert.equal(shouldShowAd(true, 0), true);
  assert.equal(shouldShowAd(false, 2), true);
  assert.equal(shouldShowAd(true, 1), true);
});
test("isAdConfigured", () => {
  assert.equal(isAdConfigured({ client: "", slot: "" }), false);
  assert.equal(isAdConfigured({ client: "ca-pub-1", slot: "" }), false);
  assert.equal(isAdConfigured({ client: "  ", slot: "123" }), false);
  assert.equal(isAdConfigured({ client: "ca-pub-1", slot: "123" }), true);
});
test("adScriptSrc", () => {
  assert.equal(
    adScriptSrc("ca-pub-1"),
    "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=ca-pub-1",
  );
});
