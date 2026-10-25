//! Văn bản, phụ đề (SRT, VTT) và bảng (CSV, TSV) làm bằng Rust thuần, không gọi chương trình ngoài.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    error::{MorphixError, Result},
    kind::TextFormat,
};

/// Một đoạn phụ đề: mốc bắt đầu, kết thúc (mili giây) và nội dung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// Dấu BOM UTF-8 đầu file; Excel và Windows Notepad hay tự thêm vào.
const UTF8_BOM_CHAR: char = '\u{feff}';
const UTF8_BOM_BYTES: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Lý do đọc mốc thời gian thất bại, hiện nguyên văn cho người dùng.
const MOC_SAI: &str = "mốc thời gian sai";
const MOC_QUA_LON: &str = "mốc thời gian quá lớn";

/// Đọc phụ đề SRT hoặc VTT. VTT bỏ header `WEBVTT` và các khối `NOTE`, `STYLE`, `REGION`.
pub fn read_cues(path: &Path) -> Result<Vec<Cue>> {
    let text: String = read_utf8(path)?;
    let mut cues: Vec<Cue> = Vec::new();
    for block in blocks(&text) {
        // WHY: header VTT và khối NOTE/STYLE/REGION không phải đoạn phụ đề nên bỏ qua.
        if is_vtt_metadata(&block) {
            continue;
        }
        let Some(timing_index) = block.iter().position(|line| line.contains("-->")) else {
            return Err(corrupt(
                path,
                format!("đoạn thứ {} không có mốc thời gian", cues.len() + 1),
            ));
        };
        let timing: &str = block[timing_index];
        let (start_ms, end_ms) =
            parse_timing(timing).map_err(|reason| corrupt(path, format!("{reason}: {timing}")))?;
        cues.push(Cue {
            start_ms,
            end_ms,
            text: block[timing_index + 1..].join("\n"),
        });
    }
    Ok(cues)
}

/// Ghi phụ đề SRT hoặc VTT. Định dạng khác trả `InvalidInput`.
pub fn write_cues(cues: &[Cue], format: TextFormat, out: &mut impl Write) -> Result<()> {
    // WHY: SRT dùng dấu phẩy trong mốc và có số thứ tự; VTT dùng dấu chấm và có header.
    let (header, separator, numbered) = match format {
        TextFormat::Srt => ("", ',', true),
        TextFormat::Vtt => ("WEBVTT\n\n", '.', false),
        other => {
            return Err(MorphixError::InvalidInput(format!(
                "không ghi phụ đề sang định dạng {}",
                other.extension()
            )))
        }
    };
    let mut body: String = header.to_string();
    for (index, cue) in cues.iter().enumerate() {
        // WHY: dòng trống trong nội dung sẽ bị đọc như hết đoạn, làm mất chữ phía sau.
        // WHY: đoạn rỗng không có dòng nào, nhưng "".split('\n') vẫn cho một phần tử rỗng nên phải bỏ qua.
        if !cue.text.is_empty() && cue.text.split('\n').any(|line| line.trim().is_empty()) {
            return Err(MorphixError::InvalidInput(format!(
                "đoạn {} có dòng trống, không ghi được vào phụ đề",
                index + 1
            )));
        }
        if numbered {
            body.push_str(&format!("{}\n", index + 1));
        }
        body.push_str(&format!(
            "{} --> {}\n{}\n\n",
            format_time(cue.start_ms, separator),
            format_time(cue.end_ms, separator),
            cue.text
        ));
    }
    out.write_all(body.as_bytes()).map_err(sink_error)
}

/// Chỉ giữ nội dung, mỗi đoạn một khối dòng, không có số thứ tự hay mốc.
pub fn strip_timestamps(cues: &[Cue]) -> String {
    cues.iter().map(|cue| format!("{}\n", cue.text)).collect()
}

/// Ghi nguyên văn văn bản (TXT, MD, hoặc nội dung đã bỏ mốc thời gian).
pub fn write_text(text: &str, out: &mut impl Write) -> Result<()> {
    out.write_all(text.as_bytes()).map_err(sink_error)
}

/// Đọc bảng CSV (dấu phẩy) hoặc TSV (tab). Mọi dòng là một hàng, không coi dòng đầu là tiêu đề.
pub fn read_table(path: &Path, format: TextFormat) -> Result<Vec<Vec<String>>> {
    let delimiter: u8 = table_delimiter(format)?;
    let text: String = read_utf8(path)?;
    if format == TextFormat::Tsv {
        // WHY: TSV không có chuẩn trích dẫn nên tách tay theo tab; csv sẽ bỏ dấu " và bỏ dòng trống.
        let rows: Vec<Vec<String>> = text
            .lines()
            .map(|line: &str| -> Vec<String> { line.split('\t').map(str::to_owned).collect() })
            .collect();
        return Ok(rows);
    }
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        // WHY: không ép số cột đều nhau để một dòng lệch vẫn đọc được, dữ liệu không bị bỏ.
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    for record in reader.records() {
        let record =
            record.map_err(|e| corrupt(path, format!("không phân tích được bảng: {e}")))?;
        rows.push(record.iter().map(str::to_owned).collect());
    }
    // WHY: file kiểu Excel tiếng Việt dùng ";" làm ngăn cách; đọc như CSV sẽ thành một cột
    // và hỏng cả bảng âm thầm, nên báo lỗi để người dùng lưu lại đúng dấu. Chỉ báo khi mọi dòng
    // đều là một ô có ";" và file không có dấu phẩy hay ngoặc kép, vì CSV hợp lệ vẫn có thể có ";".
    if format == TextFormat::Csv
        && !rows.is_empty()
        && rows
            .iter()
            .all(|row| row.len() == 1 && row[0].contains(';'))
        && !text.contains([',', '"'])
    {
        return Err(corrupt(
            path,
            "file có dấu chấm phẩy (;) nhưng không có dấu phẩy (,); hãy lưu lại CSV với dấu phẩy làm ngăn cách",
        ));
    }
    Ok(rows)
}

/// Ghi bảng ra CSV (có BOM đầu file) hoặc TSV. TSV từ chối ô chứa tab hoặc xuống dòng.
pub fn write_table(rows: &[Vec<String>], format: TextFormat, out: &mut impl Write) -> Result<()> {
    match format {
        TextFormat::Csv => {
            // WHY: BOM giúp Excel trên Windows nhận đúng UTF-8 cho tiếng Việt.
            out.write_all(UTF8_BOM_BYTES).map_err(sink_error)?;
            let mut writer = csv::WriterBuilder::new()
                .delimiter(b',')
                // WHY: ghi đúng số cột của từng dòng như đã đọc, không thêm ô trống cho dòng lệch.
                .flexible(true)
                .terminator(csv::Terminator::Any(b'\n'))
                .from_writer(&mut *out);
            for row in rows {
                writer
                    .write_record(row)
                    .map_err(|e| sink_error(std::io::Error::other(e)))?;
            }
            writer.flush().map_err(sink_error)
        }
        TextFormat::Tsv => {
            let mut body: String = String::new();
            for (index, row) in rows.iter().enumerate() {
                if row.iter().any(|cell| cell.contains(['\t', '\n', '\r'])) {
                    return Err(MorphixError::InvalidInput(format!(
                        "dòng {} có ô chứa tab hoặc xuống dòng, TSV không biểu diễn được",
                        index + 1
                    )));
                }
                body.push_str(&row.join("\t"));
                body.push('\n');
            }
            out.write_all(body.as_bytes()).map_err(sink_error)
        }
        other => Err(MorphixError::InvalidInput(format!(
            "{} không phải định dạng bảng",
            other.extension()
        ))),
    }
}

fn table_delimiter(format: TextFormat) -> Result<u8> {
    match format {
        TextFormat::Csv => Ok(b','),
        TextFormat::Tsv => Ok(b'\t'),
        other => Err(MorphixError::InvalidInput(format!(
            "{} không phải định dạng bảng",
            other.extension()
        ))),
    }
}

/// Đọc file thành UTF-8 và bỏ BOM đầu file. Không đoán mã hoá khác: lỗi thì nói rõ.
pub fn read_utf8(path: &Path) -> Result<String> {
    let bytes: Vec<u8> = std::fs::read(path).map_err(|source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let text: String = String::from_utf8(bytes)
        .map_err(|_| corrupt(path, "không phải UTF-8 (chỉ hỗ trợ UTF-8 và UTF-8 có BOM)"))?;
    Ok(match text.strip_prefix(UTF8_BOM_CHAR) {
        Some(rest) => rest.to_string(),
        None => text,
    })
}

/// Tách file thành khối, ngăn bởi dòng trống. Khối cuối không cần dòng trống theo sau.
fn blocks(text: &str) -> Vec<Vec<&str>> {
    let mut out: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Khối VTT không phải đoạn phụ đề: bắt đầu bằng `WEBVTT`, `NOTE`, `STYLE` hoặc `REGION` (đúng cả từ).
/// WHY: so khớp cả từ đầu tiên, nếu không `NOTE-1` (id của cue) sẽ bị bỏ mất âm thầm. Id cue cũng có
/// thể là `STYLE`, nên STYLE/REGION chỉ bị bỏ khi khối không có mốc thời gian.
fn is_vtt_metadata(block: &[&str]) -> bool {
    match block[0].split_whitespace().next() {
        Some("WEBVTT" | "NOTE") => true,
        Some("STYLE" | "REGION") => !block.iter().any(|line| line.contains("-->")),
        _ => false,
    }
}

/// Đọc dòng `start --> end [cài đặt VTT]`. Lỗi là lý do để báo cho người dùng.
fn parse_timing(line: &str) -> std::result::Result<(u64, u64), &'static str> {
    let mut parts = line.split_whitespace();
    let (Some(start), Some("-->"), Some(end)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(MOC_SAI);
    };
    Ok((parse_time(start)?, parse_time(end)?))
}

/// Nhận `HH:MM:SS,mmm` (SRT) và `HH:MM:SS.mmm` hoặc `MM:SS.mmm` (VTT).
/// WHY: hai định dạng chỉ khác dấu thập phân và việc có giờ hay không, nên một hàm đọc cả hai.
fn parse_time(text: &str) -> std::result::Result<u64, &'static str> {
    let (clock, fraction) = text.rsplit_once([',', '.']).ok_or(MOC_SAI)?;
    if fraction.len() != 3 {
        return Err(MOC_SAI);
    }
    let millis: u64 = fraction.parse().map_err(|_| MOC_SAI)?;
    let fields: Vec<u64> = clock
        .split(':')
        .map(|field| field.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()
        .ok_or(MOC_SAI)?;
    // WHY: giờ do người dùng nhập có thể rất lớn; phép nhân thường sẽ panic (debug) hoặc tràn số
    // thành mốc sai (release), nên kiểm tra tràn và báo lỗi thay vì đoán.
    let seconds: u64 = match fields[..] {
        [minutes, seconds] => minutes.checked_mul(60).and_then(|m| m.checked_add(seconds)),
        [hours, minutes, seconds] => hours
            .checked_mul(60)
            .and_then(|h| h.checked_add(minutes))
            .and_then(|m| m.checked_mul(60))
            .and_then(|m| m.checked_add(seconds)),
        _ => return Err(MOC_SAI),
    }
    .ok_or(MOC_QUA_LON)?;
    seconds
        .checked_mul(1000)
        .and_then(|s| s.checked_add(millis))
        .ok_or(MOC_QUA_LON)
}

fn format_time(ms: u64, separator: char) -> String {
    let hours: u64 = ms / 3_600_000;
    let minutes: u64 = ms / 60_000 % 60;
    let seconds: u64 = ms / 1000 % 60;
    let millis: u64 = ms % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02}{separator}{millis:03}")
}

fn corrupt(path: &Path, reason: impl Into<String>) -> MorphixError {
    MorphixError::Corrupt {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

/// WHY: ghi vào `out` không gắn với đường dẫn nào nên đường dẫn để trống; người gọi có đường dẫn
/// đích thì tự gắn vào thông báo, không nuốt lỗi.
fn sink_error(source: std::io::Error) -> MorphixError {
    MorphixError::Io {
        path: PathBuf::new(),
        source,
    }
}
