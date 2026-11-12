interface Cue {
  start: string;
  end: string;
  text: string;
}

function parseSubtitles(input: string): Cue[] {
  let text = input;
  if (text.startsWith("WEBVTT")) {
    text = text.replace(/^WEBVTT[^\n]*\n*/, "");
  }

  const blocks = text.split(/\n\n+/);
  const cues: Cue[] = [];

  for (const block of blocks) {
    const trimmed = block.trim();
    if (!trimmed) continue;

    const lines = trimmed.split("\n");
    let arrowLineIdx = -1;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].includes("-->")) {
        arrowLineIdx = i;
        break;
      }
    }

    if (arrowLineIdx === -1) continue;

    const [startPart, endPart] = lines[arrowLineIdx].split("-->");
    const start = startPart.trim();
    const end = endPart.trim().split(/\s+/)[0];
    const textLines = lines.slice(arrowLineIdx + 1).join("\n");

    cues.push({ start, end, text: textLines });
  }

  return cues;
}

function toSrtTime(t: string): string {
  let s = t.replace(".", ",");
  const parts = s.split(":");
  if (parts.length === 2) {
    s = `00:${s}`;
  }
  return s;
}

function toVttTime(t: string): string {
  return t.replace(",", ".");
}

function parseDelimited(text: string, sep: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let inQuotes = false;
  let i = 0;

  while (i < text.length) {
    const char = text[i];
    if (inQuotes) {
      if (char === '"') {
        if (i + 1 < text.length && text[i + 1] === '"') {
          cell += '"';
          i += 2;
          continue;
        } else {
          inQuotes = false;
          i++;
          continue;
        }
      } else {
        cell += char;
        i++;
        continue;
      }
    } else {
      if (char === '"') {
        inQuotes = true;
        i++;
        continue;
      } else if (char === sep) {
        row.push(cell);
        cell = "";
        i++;
        continue;
      } else if (char === "\n") {
        row.push(cell);
        rows.push(row);
        row = [];
        cell = "";
        i++;
        continue;
      } else {
        cell += char;
        i++;
        continue;
      }
    }
  }

  if (cell.length > 0 || row.length > 0) {
    row.push(cell);
    rows.push(row);
  }

  return rows;
}

function formatDelimited(rows: string[][], sep: string): string {
  return (
    rows
      .map((row) =>
        row
          .map((cell) => {
            if (cell.includes(sep) || cell.includes('"') || cell.includes("\n")) {
              return `"${cell.replace(/"/g, '""')}"`;
            }
            return cell;
          })
          .join(sep)
      )
      .join("\n") + (rows.length > 0 ? "\n" : "")
  );
}

function escapeMdCell(cell: string): string {
  return cell.replace(/\|/g, "\\|").replace(/\n/g, "<br>");
}

function formatMarkdownTable(rows: string[][]): string {
  if (rows.length === 0) return "";
  const header = rows[0];
  const colCount = header.length;

  const lines: string[] = [];
  lines.push(`| ${header.map(escapeMdCell).join(" | ")} |`);
  lines.push(`| ${header.map(() => "---").join(" | ")} |`);

  for (let r = 1; r < rows.length; r++) {
    const row = rows[r];
    const cells: string[] = [];
    for (let c = 0; c < colCount; c++) {
      cells.push(escapeMdCell(row[c] || ""));
    }
    lines.push(`| ${cells.join(" | ")} |`);
  }

  return lines.join("\n") + "\n";
}

export function convertPureText(input: string, from: string, to: string): string | null {
  if (input.charCodeAt(0) === 0xfeff) {
    input = input.slice(1);
  }
  input = input.replace(/\r\n/g, "\n").replace(/\r/g, "\n");

  const f = from.toLowerCase();
  const t = to.toLowerCase();

  if (f === "txt" && t === "md") {
    return input;
  }

  if (f === "srt" && t === "vtt") {
    const cues = parseSubtitles(input);
    return `WEBVTT\n\n${cues.map((c) => `${toVttTime(c.start)} --> ${toVttTime(c.end)}\n${c.text}`).join("\n\n")}\n`;
  }

  if (f === "vtt" && t === "srt") {
    const cues = parseSubtitles(input);
    return `${cues.map((c, i) => `${i + 1}\n${toSrtTime(c.start)} --> ${toSrtTime(c.end)}\n${c.text}`).join("\n\n")}\n`;
  }

  if ((f === "srt" || f === "vtt") && t === "txt") {
    const cues = parseSubtitles(input);
    return `${cues.map((c) => c.text).join("\n\n")}\n`;
  }

  if (f === "csv" && t === "tsv") {
    const rows = parseDelimited(input, ",");
    return formatDelimited(rows, "\t");
  }

  if (f === "tsv" && t === "csv") {
    const rows = parseDelimited(input, "\t");
    return formatDelimited(rows, ",");
  }

  if ((f === "csv" || f === "tsv") && t === "md") {
    const sep = f === "csv" ? "," : "\t";
    const rows = parseDelimited(input, sep);
    return formatMarkdownTable(rows);
  }

  return null;
}
