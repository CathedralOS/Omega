//! A rendered refusal site must never hand an author a compiler path.

use super::CompilerCheckSite;

/// With the developer channel closed — the state every author compiles in —
/// neither rendering may contain this crate's own source path.
#[test]
fn closed_channel_withholds_the_compiler_path() {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
        return;
    }
    let site = CompilerCheckSite::caller();
    for rendered in [format!("{site:?}"), format!("{site}")] {
        assert!(
            !rendered.contains("omega-rust/") && !rendered.contains(".rs"),
            "a withheld site rendered a compiler source path: {rendered}"
        );
    }
}

/// The location stays available to a developer holding the value, so
/// withholding it from rendering costs no debugging information.
#[test]
fn the_location_remains_readable_as_data() {
    let site = CompilerCheckSite::caller();
    assert!(site.location().file().ends_with("tests.rs"));
}
