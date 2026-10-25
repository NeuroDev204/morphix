#[test]
fn version_matches_workspace() {
    assert_eq!(morphix_gui::app_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn init_logging_is_safe_and_idempotent() {
    morphix_gui::init_logging();
    morphix_gui::init_logging();
}
