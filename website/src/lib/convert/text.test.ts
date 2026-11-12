import test from "node:test";
import assert from "node:assert/strict";
import { convertPureText } from "./text.ts";

const SRT = "\uFEFF1\r\n00:00:01,000 --> 00:00:02,500\r\nXin chào\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nDòng một\r\nDòng hai\r\n";
const VTT = "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nXin chào\n\n00:00:03.000 --> 00:00:04.000\nDòng một\nDòng hai\n";

test("srt sang vtt", () => assert.equal(convertPureText(SRT, "srt", "vtt"), VTT));
test("vtt sang srt", () => assert.equal(
  convertPureText(VTT, "vtt", "srt"),
  "1\n00:00:01,000 --> 00:00:02,500\nXin chào\n\n2\n00:00:03,000 --> 00:00:04,000\nDòng một\nDòng hai\n"
));
test("phụ đề sang txt", () => {
  assert.equal(convertPureText(SRT, "srt", "txt"), "Xin chào\n\nDòng một\nDòng hai\n");
  assert.equal(convertPureText(VTT, "vtt", "txt"), "Xin chào\n\nDòng một\nDòng hai\n");
});
test("csv sang tsv giữ ô có ngoặc", () => assert.equal(
  convertPureText('ten,ghi chu\r\n"An, Binh","noi ""a"""\r\n', "csv", "tsv"),
  'ten\tghi chu\nAn, Binh\t"noi ""a"""\n'
));
test("tsv sang csv", () => assert.equal(
  convertPureText("a\tb\nx,y\tz\n", "tsv", "csv"), 'a,b\n"x,y",z\n'
));
test("csv sang bảng markdown", () => assert.equal(
  convertPureText('a,b\n"1|2","dong 1\ndong 2"\n', "csv", "md"),
  "| a | b |\n| --- | --- |\n| 1\\|2 | dong 1<br>dong 2 |\n"
));
test("txt sang md giữ nguyên", () => assert.equal(convertPureText("abc\n", "txt", "md"), "abc\n"));
test("cặp cần thư viện trả null", () => {
  assert.equal(convertPureText("# a", "md", "html"), null);
  assert.equal(convertPureText("<p>a</p>", "html", "md"), null);
});
