import test from "node:test";
import assert from "node:assert/strict";
import { extOf, groupOf, targetsFor, sharedTargets, outputName, ffmpegArgs, mimeOf } from "./catalog.ts";

test("extOf chuẩn hoá", () => {
  assert.equal(extOf("ANH.JPG"), "jpg");
  assert.equal(extOf("a.jpeg"), "jpg");
  assert.equal(extOf("b.tif"), "tiff");
  assert.equal(extOf("c.htm"), "html");
  assert.equal(extOf("README"), "");
  assert.equal(extOf("x.tar.gz"), "gz");
});
test("groupOf", () => {
  assert.equal(groupOf("png"), "image");
  assert.equal(groupOf("wma"), "audio");
  assert.equal(groupOf("wmv"), "video");
  assert.equal(groupOf("docx"), "doc");
  assert.equal(groupOf("pdf"), null);
  assert.equal(groupOf(""), null);
});
test("targetsFor bỏ chính nó", () => {
  assert.deepEqual(targetsFor("png"), ["jpg", "webp", "avif", "gif", "tiff", "bmp"]);
  assert.deepEqual(targetsFor("mp4"), ["mkv", "webm", "mov", "avi", "gif", "mp3"]);
  assert.deepEqual(targetsFor("wma"), ["mp3", "m4a", "wav", "flac", "ogg", "opus"]);
  assert.deepEqual(targetsFor("md"), ["txt", "html"]);
  assert.deepEqual(targetsFor("srt"), ["vtt", "txt"]);
  assert.deepEqual(targetsFor("7z"), []);
});
test("sharedTargets", () => {
  assert.deepEqual(sharedTargets(["png", "jpg"]), ["jpg", "png", "webp", "avif", "gif", "tiff", "bmp"]);
  assert.deepEqual(sharedTargets(["txt", "md"]), ["txt", "md", "html"]);
  assert.equal(sharedTargets(["png", "mp4"]), null);
  assert.equal(sharedTargets(["png", "pdf"]), null);
  assert.equal(sharedTargets(["csv", "srt"]), null);
  assert.equal(sharedTargets([]), null);
});
test("outputName không trùng", () => {
  const taken = new Set<string>();
  assert.equal(outputName("a.png", "webp", taken), "a.webp");
  assert.equal(outputName("a.jpg", "webp", taken), "a (2).webp");
  assert.equal(outputName("a.gif", "webp", taken), "a (3).webp");
  assert.equal(outputName("ghi chu.final.md", "html", taken), "ghi chu.final.html");
});
test("ffmpegArgs", () => {
  assert.deepEqual(ffmpegArgs("mp3"), ["-vn", "-c:a", "libmp3lame", "-b:a", "192k"]);
  assert.deepEqual(ffmpegArgs("gif"), ["-vf", "fps=10,scale=480:-1:flags=lanczos"]);
  assert.ok(ffmpegArgs("mp4")!.includes("ultrafast"));
  assert.deepEqual(ffmpegArgs("bmp"), []);
  assert.equal(ffmpegArgs("wmv"), null);
});
test("mimeOf", () => {
  assert.equal(mimeOf("webp"), "image/webp");
  assert.equal(mimeOf("mp3"), "audio/mpeg");
  assert.equal(mimeOf("md"), "text/markdown");
  assert.equal(mimeOf("zzz"), "application/octet-stream");
});
