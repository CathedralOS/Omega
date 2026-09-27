use super::{PackageCommandKind, rejects};

#[test]
fn package_selection_rejects_duplicates_missing_values_and_wrong_commands() {
    rejects(
        PackageCommandKind::Install,
        &["source", "--package", "one", "--package", "two"],
        "duplicate --package",
    );
    for suffix in [vec![], vec![""], vec!["--as"]] {
        let mut arguments = vec!["source", "--package"];
        arguments.extend(suffix);
        rejects(PackageCommandKind::Install, &arguments, "requires");
    }
    rejects(
        PackageCommandKind::Update,
        &["--package", "one"],
        "only valid for install",
    );
}
