//! Read-only inspection of files owned by an installed package (legacy `.deb` installs).

use std::{
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::Path,
};

use super::{
    tools::{ToolOutcome, ToolRunner},
    ComponentId, ComponentStatus, IntegrationComponentReport,
};

pub const SYSTEM_DING_EXTENSION_PATH: &str =
    "/usr/share/gnome-shell/extensions/ding@rastersoft.com";
const DIVERSION_PACKAGE: &str = "morphix";

/// Kiểm tra file hệ thống có thuộc package Morphix hay không.
pub fn inspect_package_file(
    id: ComponentId,
    path: &Path,
    tools: &ToolRunner,
) -> IntegrationComponentReport {
    let args: [&OsStr; 2] = [OsStr::new("-S"), path.as_os_str()];
    match tools("dpkg-query", &args) {
        ToolOutcome::NotFound => report(
            id,
            ComponentStatus::NotApplicable,
            Some(path),
            "dpkg-query không có trên hệ thống này",
        ),
        ToolOutcome::Failed(message) => report(id, ComponentStatus::Failed, Some(path), &message),
        ToolOutcome::Exited {
            code: Some(0),
            stdout,
            ..
        } => {
            let owners: Vec<&str> = stdout
                .lines()
                .filter_map(|line| line.split_once(": ").map(|(owner, _)| owner))
                .collect();
            if owners.is_empty() {
                return report(
                    id,
                    ComponentStatus::Failed,
                    Some(path),
                    "dpkg-query không trả về package sở hữu file",
                );
            }
            if owners.iter().any(|owner| !is_morphix_owner(owner)) {
                return report(
                    id,
                    ComponentStatus::Conflict,
                    Some(path),
                    "đường dẫn thuộc package khác",
                );
            }
            match regular_file_state(path) {
                Ok(()) => report(
                    id,
                    ComponentStatus::Installed,
                    Some(path),
                    "file được package Morphix cài đặt",
                ),
                Err(FileState::Missing) => report(
                    id,
                    ComponentStatus::Stale,
                    Some(path),
                    "dpkg khai báo file nhưng file không tồn tại",
                ),
                Err(FileState::NotRegular) => report(
                    id,
                    ComponentStatus::Conflict,
                    Some(path),
                    "đường dẫn package không phải file thường",
                ),
                Err(FileState::Failed(message)) => {
                    report(id, ComponentStatus::Failed, Some(path), &message)
                }
            }
        }
        ToolOutcome::Exited {
            code: Some(1),
            stdout,
            ..
        } if stdout.trim().is_empty() => match regular_file_state(path) {
            Ok(()) | Err(FileState::NotRegular) => report(
                id,
                ComponentStatus::Conflict,
                Some(path),
                "có file không thuộc package dpkg nào đã biết",
            ),
            Err(FileState::Missing) => report(
                id,
                ComponentStatus::Missing,
                Some(path),
                "package chưa được cài",
            ),
            Err(FileState::Failed(message)) => {
                report(id, ComponentStatus::Failed, Some(path), &message)
            }
        },
        ToolOutcome::Exited {
            code,
            stdout,
            stderr,
        } => report(
            id,
            ComponentStatus::Failed,
            Some(path),
            &format!(
                "dpkg-query -S kết thúc với mã {}: {}",
                exit_code_text(code),
                output_detail(&stdout, &stderr)
            ),
        ),
    }
}

enum FileState {
    Missing,
    NotRegular,
    Failed(String),
}

fn regular_file_state(path: &Path) -> Result<(), FileState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(FileState::NotRegular),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(FileState::Missing),
        Err(error) => Err(FileState::Failed(format!(
            "không kiểm tra được {}: {error}",
            path.display()
        ))),
    }
}

fn is_morphix_owner(owner: &str) -> bool {
    owner == DIVERSION_PACKAGE
        || owner
            .strip_prefix("morphix:")
            .is_some_and(|arch| !arch.is_empty())
}

fn exit_code_text(code: Option<i32>) -> String {
    code.map_or_else(|| "không xác định".to_string(), |value| value.to_string())
}

fn output_detail(stdout: &str, stderr: &str) -> String {
    let detail: String = [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    if detail.is_empty() {
        "không có chi tiết từ lệnh".to_string()
    } else {
        detail
    }
}

fn report(
    id: ComponentId,
    status: ComponentStatus,
    path: Option<&Path>,
    message: &str,
) -> IntegrationComponentReport {
    IntegrationComponentReport {
        id,
        status,
        changed: false,
        path: path.map(Path::to_path_buf),
        message: message.to_string(),
    }
}
