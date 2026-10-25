use std::path::PathBuf;

use crate::integration::{
    ComponentStatus, IntegrationComponentReport, IntegrationReport, OverallStatus,
};
use morphix_core::{ItemReport, MorphixError};
use serde::Serialize;

#[derive(Serialize)]
struct JsonItem {
    inputs: Vec<String>,
    output: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct JsonReport {
    results: Vec<JsonItem>,
    ok: usize,
    failed: usize,
}

#[derive(Serialize)]
struct JsonPlanItem {
    inputs: Vec<String>,
    output: String,
}

#[derive(Serialize)]
struct JsonPlan {
    plan: Vec<JsonPlanItem>,
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|p| p.display().to_string()).collect()
}

/// Các lỗi đã mang đường dẫn trong thông báo thì in nguyên; lỗi còn lại thêm tên file để biết của file nào.
fn error_line(inputs: &[PathBuf], error: &MorphixError) -> String {
    match error {
        MorphixError::Io { .. }
        | MorphixError::Unsupported { .. }
        | MorphixError::Corrupt { .. }
        | MorphixError::NotSmaller { .. } => format!("lỗi  {error}"),
        _ => format!("lỗi  {}: {error}", names(inputs).join(", ")),
    }
}

/// Kết quả ra `stdout` (dòng `ok`), lỗi ra `stderr`; `--json` thì **một** object duy nhất trên `stdout`.
pub fn print_results(items: &[ItemReport], as_json: bool) {
    if as_json {
        let results: Vec<JsonItem> = items
            .iter()
            .map(|item| JsonItem {
                inputs: names(&item.inputs),
                output: item.result.as_ref().ok().map(|p| p.display().to_string()),
                error: item.result.as_ref().err().map(ToString::to_string),
            })
            .collect();
        let ok = results.iter().filter(|r| r.error.is_none()).count();
        let report = JsonReport {
            failed: results.len() - ok,
            ok,
            results,
        };
        println!(
            "{}",
            serde_json::to_string(&report).expect("JSON của kết quả luôn mã hoá được")
        );
        return;
    }
    for item in items {
        match &item.result {
            Ok(output) => println!(
                "ok  {} -> {}",
                names(&item.inputs).join(", "),
                output.display()
            ),
            Err(error) => eprintln!("{}", error_line(&item.inputs, error)),
        }
    }
}

/// Kế hoạch của `--dry-run`: tên file sẽ tạo, không tạo gì.
pub fn print_plan(plan: &[(Vec<PathBuf>, PathBuf)], as_json: bool) {
    if as_json {
        let plan = JsonPlan {
            plan: plan
                .iter()
                .map(|(inputs, output)| JsonPlanItem {
                    inputs: names(inputs),
                    output: output.display().to_string(),
                })
                .collect(),
        };
        println!(
            "{}",
            serde_json::to_string(&plan).expect("JSON của kế hoạch luôn mã hoá được")
        );
        return;
    }
    for (inputs, output) in plan {
        println!("{} -> {}", names(inputs).join(", "), output.display());
    }
}

/// Báo cáo `doctor`: ffmpeg và thư mục dữ liệu là kiểm tra cũ; `integration` là trạng thái tích hợp chỉ đọc.
#[derive(Serialize)]
pub struct DoctorReport {
    pub ffmpeg: Option<String>,
    pub data_dir: String,
    pub data_dir_writable: bool,
    pub data_dir_error: Option<String>,
    pub log: String,
    pub integration: Option<IntegrationReport>,
    pub integration_error: Option<String>,
}

fn overall_label(overall: OverallStatus) -> &'static str {
    match overall {
        OverallStatus::Success => "thành công",
        OverallStatus::Partial => "thành công một phần",
        OverallStatus::Failure => "thất bại",
        OverallStatus::DryRun => "chỉ thử, không ghi gì",
        OverallStatus::Inconclusive => "không kết luận được",
    }
}

/// Mỗi mục một dòng; mục lỗi ra `stderr` theo quy ước của các lệnh khác.
fn print_components(components: &[IntegrationComponentReport]) {
    for component in components {
        let line = format!(
            "tích hợp {}: {} - {}",
            component.id.as_str(),
            component.status.as_str(),
            component.message
        );
        match component.status {
            ComponentStatus::Failed | ComponentStatus::Conflict | ComponentStatus::Stale => {
                eprintln!("{line}")
            }
            _ => println!("{line}"),
        }
    }
}

/// Kết quả install/uninstall: `--json` là một object duy nhất trên `stdout`.
/// Trả lỗi mã hoá thay vì panic: lệnh vẫn thoát có mã rõ ràng để script không đọc nhầm.
pub fn print_integration(report: &IntegrationReport, as_json: bool) -> serde_json::Result<()> {
    if as_json {
        println!("{}", serde_json::to_string(report)?);
        return Ok(());
    }
    print_components(&report.components);
    println!("kết quả: {}", overall_label(report.overall));
    Ok(())
}

/// Báo cáo doctor: giữ nguyên các dòng ffmpeg/thư mục dữ liệu/log, thêm trạng thái tích hợp.
pub fn print_doctor(report: &DoctorReport, as_json: bool) -> serde_json::Result<()> {
    if as_json {
        println!("{}", serde_json::to_string(report)?);
        return Ok(());
    }
    match &report.ffmpeg {
        Some(path) => println!("ffmpeg: {path}"),
        None => println!("ffmpeg: không tìm thấy (cần cho video và âm thanh từ đợt 2)"),
    }
    let state = if report.data_dir_writable {
        "ghi được"
    } else {
        "KHÔNG ghi được"
    };
    println!("thư mục dữ liệu: {} ({state})", report.data_dir);
    println!("log: {}", report.log);
    match (&report.integration, &report.integration_error) {
        (Some(integration), _) => print_components(&integration.components),
        (None, Some(error)) => eprintln!("tích hợp: không kiểm được ({error})"),
        (None, None) => eprintln!("tích hợp: không kiểm được"),
    }
    Ok(())
}
