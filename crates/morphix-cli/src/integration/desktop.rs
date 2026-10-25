//! Read-only inspection of Morphix package-owned Ubuntu desktop integration.

use std::{
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::{
    tools::{ToolOutcome, ToolRunner},
    ComponentId, ComponentStatus, IntegrationComponentReport,
};

pub const SYSTEM_DING_EXTENSION_PATH: &str =
    "/usr/share/gnome-shell/extensions/ding@rastersoft.com";
const SOURCE_MANIFEST: &str = include_str!("../../../../integration/ding/source.json");
const DING_PACKAGE_NAME: &str = "gnome-shell-extension-desktop-icons-ng";
const DIVERSION_PACKAGE: &str = "morphix";
const DIVERSION_SUFFIX: &str = ".morphix-original";
const EXPECTED_PATCHED_FILES: [&str; 2] = ["app/desktopManager.js", "app/fileItemMenu.js"];

#[derive(Debug, Deserialize)]
struct DingManifest {
    package: String,
    version: String,
    source_files: std::collections::BTreeMap<String, String>,
    patched_files: std::collections::BTreeMap<String, String>,
}

/// Kiểm trạng thái DING mà không thay đổi package database hoặc file hệ thống.
pub fn inspect_desktop(extension_root: &Path, tools: &ToolRunner) -> IntegrationComponentReport {
    let id: ComponentId = ComponentId::DesktopContextMenu;
    let manifest: DingManifest = match serde_json::from_str(SOURCE_MANIFEST) {
        Ok(manifest) => manifest,
        Err(error) => {
            return report(
                id,
                ComponentStatus::Failed,
                Some(extension_root),
                &format!("manifest DING tích hợp không hợp lệ: {error}"),
            )
        }
    };
    let patched_paths: Vec<&str> = manifest.patched_files.keys().map(String::as_str).collect();
    if patched_paths != EXPECTED_PATCHED_FILES
        || manifest.package != DING_PACKAGE_NAME
        || manifest
            .patched_files
            .keys()
            .any(|path| !manifest.source_files.contains_key(path))
    {
        return report(
            id,
            ComponentStatus::Failed,
            Some(extension_root),
            "manifest DING phải ghim đúng hai file menu đã hỗ trợ",
        );
    }
    if extension_root.is_symlink() || extension_root.join("app").is_symlink() {
        return report(
            id,
            ComponentStatus::Conflict,
            Some(extension_root),
            "thư mục DING đi qua symlink; giữ nguyên và kiểm tra lại package",
        );
    }

    let package_version: String =
        match installed_package_version(&manifest.package, id, extension_root, tools) {
            Ok(version) => version,
            Err(report) => return report,
        };
    if package_version != manifest.version {
        return report(
            id,
            ComponentStatus::Stale,
            Some(extension_root),
            &format!(
                "DING có phiên bản {package_version}; Morphix hỗ trợ {}",
                manifest.version
            ),
        );
    }

    let mut missing_diversion: Option<PathBuf> = None;
    for (relative_path, expected_patch_hash) in &manifest.patched_files {
        let patched_path: PathBuf = extension_root.join(relative_path);
        let mut backup_name = patched_path.as_os_str().to_os_string();
        backup_name.push(DIVERSION_SUFFIX);
        let backup_path: PathBuf = PathBuf::from(backup_name);
        let Some(expected_source_hash) = manifest.source_files.get(relative_path) else {
            return report(
                id,
                ComponentStatus::Failed,
                Some(&backup_path),
                "manifest thiếu hash DING gốc",
            );
        };

        match inspect_diversion(&patched_path, &backup_path, id, tools) {
            Ok(true) => {}
            Ok(false) => {
                if let Err(report) =
                    verify_file_hash(&patched_path, expected_source_hash, id, tools)
                {
                    return report;
                }
                missing_diversion.get_or_insert(patched_path);
                continue;
            }
            Err(report) => return report,
        }

        if let Err(report) = verify_file_hash(&backup_path, expected_source_hash, id, tools) {
            return report;
        }
        if let Err(report) = verify_file_hash(&patched_path, expected_patch_hash, id, tools) {
            return report;
        }
    }

    if let Some(path) = missing_diversion {
        return report(
            id,
            ComponentStatus::Missing,
            Some(&path),
            "DING chưa được divert bởi package Morphix",
        );
    }

    report(
        id,
        ComponentStatus::Installed,
        Some(extension_root),
        "menu DING được Morphix quản lý và các file khớp manifest",
    )
}

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

fn installed_package_version(
    package: &str,
    id: ComponentId,
    path: &Path,
    tools: &ToolRunner,
) -> Result<String, IntegrationComponentReport> {
    let args: [&OsStr; 3] = [
        OsStr::new("-W"),
        OsStr::new("-f=${Version}"),
        OsStr::new(package),
    ];
    match tools("dpkg-query", &args) {
        ToolOutcome::NotFound => Err(report(
            id,
            ComponentStatus::NotApplicable,
            Some(path),
            "dpkg-query không có trên Linux này",
        )),
        ToolOutcome::Failed(message) => {
            Err(report(id, ComponentStatus::Failed, Some(path), &message))
        }
        ToolOutcome::Exited {
            code: Some(0),
            stdout,
            ..
        } if !stdout.trim().is_empty() => Ok(stdout.trim().to_string()),
        ToolOutcome::Exited {
            code: Some(1),
            stdout,
            ..
        } if stdout.trim().is_empty() => Err(report(
            id,
            ComponentStatus::Missing,
            Some(path),
            "gói DING chưa được cài",
        )),
        ToolOutcome::Exited {
            code,
            stdout,
            stderr,
        } => Err(report(
            id,
            ComponentStatus::Failed,
            Some(path),
            &format!(
                "dpkg-query không đọc được phiên bản {package} (mã {}): {}",
                exit_code_text(code),
                output_detail(&stdout, &stderr)
            ),
        )),
    }
}

fn inspect_diversion(
    path: &Path,
    expected_target: &Path,
    id: ComponentId,
    tools: &ToolRunner,
) -> Result<bool, IntegrationComponentReport> {
    let owner_args: [&OsStr; 2] = [OsStr::new("--listpackage"), path.as_os_str()];
    let owner: String = match tools("dpkg-divert", &owner_args) {
        ToolOutcome::NotFound => {
            return Err(report(
                id,
                ComponentStatus::Failed,
                Some(path),
                "không tìm thấy dpkg-divert sau khi đọc được package DING",
            ))
        }
        ToolOutcome::Failed(message) => {
            return Err(report(id, ComponentStatus::Failed, Some(path), &message))
        }
        ToolOutcome::Exited {
            code: Some(0),
            stdout,
            ..
        } => stdout.trim().to_string(),
        ToolOutcome::Exited {
            code,
            stdout,
            stderr,
        } => {
            return Err(report(
                id,
                ComponentStatus::Failed,
                Some(path),
                &format!(
                    "dpkg-divert --listpackage kết thúc với mã {}: {}",
                    exit_code_text(code),
                    output_detail(&stdout, &stderr)
                ),
            ))
        }
    };
    if owner.is_empty() {
        return Ok(false);
    }
    if owner != DIVERSION_PACKAGE {
        return Err(report(
            id,
            ComponentStatus::Conflict,
            Some(path),
            &format!("file đang được divert bởi {owner}, không phải Morphix"),
        ));
    }

    let target_args: [&OsStr; 2] = [OsStr::new("--truename"), path.as_os_str()];
    match tools("dpkg-divert", &target_args) {
        ToolOutcome::Exited {
            code: Some(0),
            stdout,
            ..
        } if stdout.trim() == expected_target.to_string_lossy() => Ok(true),
        ToolOutcome::Exited {
            code: Some(0),
            stdout,
            ..
        } => Err(report(
            id,
            ComponentStatus::Conflict,
            Some(path),
            &format!(
                "dpkg-divert trỏ tới đường dẫn ngoài dự kiến: {}",
                stdout.trim()
            ),
        )),
        ToolOutcome::NotFound => Err(report(
            id,
            ComponentStatus::Failed,
            Some(path),
            "không chạy được dpkg-divert",
        )),
        ToolOutcome::Failed(message) => {
            Err(report(id, ComponentStatus::Failed, Some(path), &message))
        }
        ToolOutcome::Exited {
            code,
            stdout,
            stderr,
        } => Err(report(
            id,
            ComponentStatus::Failed,
            Some(path),
            &format!(
                "dpkg-divert --truename kết thúc với mã {}: {}",
                exit_code_text(code),
                output_detail(&stdout, &stderr)
            ),
        )),
    }
}

fn verify_file_hash(
    path: &Path,
    expected_hash: &str,
    id: ComponentId,
    tools: &ToolRunner,
) -> Result<(), IntegrationComponentReport> {
    match regular_file_state(path) {
        Ok(()) => {}
        Err(FileState::Missing | FileState::NotRegular) => {
            return Err(report(
                id,
                ComponentStatus::Conflict,
                Some(path),
                "thiếu file thường do package Morphix quản lý",
            ))
        }
        Err(FileState::Failed(message)) => {
            return Err(report(id, ComponentStatus::Failed, Some(path), &message))
        }
    }

    let args: [&OsStr; 1] = [path.as_os_str()];
    match tools("sha256sum", &args) {
        ToolOutcome::NotFound => Err(report(
            id,
            ComponentStatus::Failed,
            Some(path),
            "coreutils sha256sum không có trên hệ thống",
        )),
        ToolOutcome::Failed(message) => {
            Err(report(id, ComponentStatus::Failed, Some(path), &message))
        }
        ToolOutcome::Exited {
            code: Some(0),
            stdout,
            ..
        } => {
            let actual_hash: &str = stdout.split_whitespace().next().unwrap_or_default();
            if actual_hash.len() != 64
                || !actual_hash.bytes().all(|value| value.is_ascii_hexdigit())
            {
                return Err(report(
                    id,
                    ComponentStatus::Failed,
                    Some(path),
                    "sha256sum trả về output không đúng định dạng",
                ));
            }
            if actual_hash == expected_hash {
                Ok(())
            } else {
                Err(report(
                    id,
                    ComponentStatus::Conflict,
                    Some(path),
                    "SHA-256 của file DING không khớp manifest đã duyệt",
                ))
            }
        }
        ToolOutcome::Exited {
            code,
            stdout,
            stderr,
        } => Err(report(
            id,
            ComponentStatus::Failed,
            Some(path),
            &format!(
                "sha256sum kết thúc với mã {}: {}",
                exit_code_text(code),
                output_detail(&stdout, &stderr)
            ),
        )),
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
