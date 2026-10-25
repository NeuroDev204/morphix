//! Lớp dùng chung của CLI `morphix`: quản lý tích hợp hệ điều hành và định dạng báo cáo.
//! Không phụ thuộc morphix-gui, Tauri, GTK hay WebKit để lệnh CLI chạy trên máy không có giao diện.

pub mod integration;
pub mod output;
pub mod self_uninstall;
pub mod uri;
