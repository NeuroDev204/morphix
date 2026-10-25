use std::path::PathBuf;

use morphix_gui::{startup_from_args, StartupRequest};

fn args(items: &[&str]) -> Vec<String> {
    std::iter::once("morphix-gui")
        .chain(items.iter().copied())
        .map(String::from)
        .collect()
}

#[test]
fn show_khay_flag_is_read_and_never_becomes_a_file_path() {
    let got = startup_from_args(args(&["--show-khay"])).unwrap();
    assert_eq!(
        got,
        StartupRequest {
            show_khay: true,
            capture_selection: false,
            unsupported_selection: false,
            paths: Vec::<PathBuf>::new(),
        }
    );
}

#[test]
fn show_khay_before_menu_keeps_the_menu_paths_untouched() {
    let got = startup_from_args(args(&["--show-khay", "menu", "/tmp/Ảnh 2.jpg"])).unwrap();
    assert!(got.show_khay);
    assert_eq!(got.paths, vec![PathBuf::from("/tmp/Ảnh 2.jpg")]);
}

#[test]
fn menu_without_show_khay_keeps_the_existing_route() {
    let got = startup_from_args(args(&["menu", "/tmp/a.jpg"])).unwrap();
    assert!(!got.show_khay);
    assert_eq!(got.paths, vec![PathBuf::from("/tmp/a.jpg")]);
}

#[test]
fn no_arguments_opens_the_app_without_forcing_the_khay() {
    let got = startup_from_args(args(&[])).unwrap();
    assert_eq!(
        got,
        StartupRequest {
            show_khay: false,
            capture_selection: false,
            unsupported_selection: false,
            paths: Vec::new()
        }
    );
}

#[test]
fn unknown_flag_is_a_usage_error_not_a_path() {
    assert!(startup_from_args(args(&["--shows-khay"])).is_err());
}

#[test]
fn startup_parses_capture_selection_flag() {
    let startup = startup_from_args(args(&["--capture-selection"])).unwrap();
    assert!(startup.capture_selection);
    assert!(!startup.show_khay);
    assert!(startup.paths.is_empty());
}

#[test]
fn startup_parses_unsupported_selection_without_treating_it_as_a_path() {
    let startup = startup_from_args(args(&["--show-khay", "--unsupported-selection"])).unwrap();
    assert!(startup.show_khay);
    assert!(startup.unsupported_selection);
    assert!(startup.paths.is_empty());
}

#[test]
fn unsupported_selection_can_be_used_without_show_khay_flag() {
    let startup = startup_from_args(args(&["--unsupported-selection"])).unwrap();
    assert!(startup.unsupported_selection);
    assert!(startup.paths.is_empty());
}
