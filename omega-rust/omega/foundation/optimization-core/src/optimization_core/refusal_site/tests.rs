// Unit tests verifying that `CompilerCheckSite` withholds compiler source paths
// from `Debug`/`Display` unless `OMEGA_COMPILER_CHECK_SITES` is set, while
// retaining `site.location()` as inspectable data.

use super::CompilerCheckSite;

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

#[test]
fn the_location_remains_readable_as_data() {
    let site = CompilerCheckSite::caller();
    assert!(site.location().file().ends_with("tests.rs"));
}
