//! Build-purpose and product-purpose dependency edges through the shipped
//! binary (wiki/spec/build/scoped_execution.md, dependency declarations):
//! one alias may name different packages in the two scopes, one package may
//! serve both purposes under two aliases, an import in either scope selects
//! only that scope's edges, and dropping an edge after publication rejects
//! only the imports that edge authorized. Fixture packages live in
//! `tests/fixtures/packages/build-purposes`.

use super::fixture::{Fixture, assert_status};

const ALPHA_BUILD: &str =
    include_str!("../../../../tests/fixtures/packages/build-purposes/alpha/build.omg");
const ALPHA_MAIN: &str =
    include_str!("../../../../tests/fixtures/packages/build-purposes/alpha/main.omg");
const BETA_BUILD: &str =
    include_str!("../../../../tests/fixtures/packages/build-purposes/beta/build.omg");
const BETA_MAIN: &str =
    include_str!("../../../../tests/fixtures/packages/build-purposes/beta/main.omg");

const PRODUCT_IMPORT_REJECTION: &str = "a product import may only select product dependencies";
const BUILD_IMPORT_REJECTION: &str = "a build import may only select build dependencies";

/// A root beside `alpha` and `beta`; the build imports `kit` and the
/// product source imports `lib`, each resolved only within its own scope.
fn purposes_fixture(build_edges: &str, product_alias: &str) -> Fixture {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.path("alpha")).unwrap();
    std::fs::create_dir(fixture.path("beta")).unwrap();
    fixture.write("alpha/build.omg", ALPHA_BUILD);
    fixture.write("alpha/main.omg", ALPHA_MAIN);
    fixture.write("beta/build.omg", BETA_BUILD);
    fixture.write("beta/main.omg", BETA_MAIN);
    fixture.write(
        "root/build.omg",
        &format!(
            "use kit::main;\nmachine build(builder: &mut Build) {{\n    builder.package(\"purpose-root\");\n{build_edges}}}\n"
        ),
    );
    fixture.write(
        "root/main.omg",
        &format!("use {product_alias}::main;\nmachine main() -> u64 {{ beta_value() }}\n"),
    );
    fixture
}

const CROSS_SCOPE_EDGES: &str = "    builder.build_depend_as(\"kit\", Source::Path { location: \"../alpha\" });\n    builder.depend_as(\"kit\", Source::Path { location: \"../beta\" });\n";

/// `omega update` publishes the lock from the declarations as authored; it
/// never rewrites build.omg.
fn assert_lock_published(fixture: &Fixture, before: &(Option<Vec<u8>>, Option<Vec<u8>>)) {
    let after = fixture.accepted_files();
    assert_eq!(after.0, before.0, "update must not edit build.omg");
    assert!(after.1.is_some(), "update must publish omega.lock");
    fixture.lock();
}

fn combined(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn one_alias_selects_different_packages_in_the_two_scopes() {
    let fixture = purposes_fixture(CROSS_SCOPE_EDGES, "kit");
    let before = fixture.accepted_files();
    let output = fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]);
    assert_status(&output, 0);
    assert_lock_published(&fixture, &before);
    let audit = fixture.omega(&[
        "audit",
        "packages",
        "--target",
        "linux_x86_64",
        "--offline",
        "--details",
    ]);
    assert_status(&audit, 0);
    let report = combined(&audit);
    assert!(report.contains("fresh graph (3 packages)"), "{report}");
    assert!(
        report.contains("-- \"kit\" [build dependency 0] --> \"alpha\""),
        "{report}"
    );
    assert!(
        report.contains("-- \"kit\" [product dependency 0] --> \"beta\""),
        "{report}"
    );
}

#[test]
fn one_package_serves_both_purposes_under_two_aliases() {
    let fixture = purposes_fixture(
        "    builder.build_depend_as(\"kit\", Source::Path { location: \"../alpha\" });\n    builder.depend_as(\"lib\", Source::Path { location: \"../alpha\" });\n",
        "lib",
    );
    fixture.write(
        "root/main.omg",
        "use lib::main;\nmachine main() -> u64 { alpha_value() }\n",
    );
    let before = fixture.accepted_files();
    assert_status(
        &fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]),
        0,
    );
    assert_lock_published(&fixture, &before);
    let audit = fixture.omega(&[
        "audit",
        "packages",
        "--target",
        "linux_x86_64",
        "--offline",
        "--details",
    ]);
    assert_status(&audit, 0);
    let report = combined(&audit);
    assert!(report.contains("fresh graph (2 packages)"), "{report}");
    assert!(report.contains("edges 2"), "{report}");
    assert!(
        report.contains("-- \"kit\" [build dependency 0] --> \"alpha\"")
            && report.contains("-- \"lib\" [product dependency 0] --> \"alpha\""),
        "both purpose edges reach the one package: {report}"
    );
}

#[test]
fn an_import_selects_only_its_own_scope_edges() {
    // A build-only edge cannot serve the product import.
    let fixture = purposes_fixture(
        "    builder.build_depend_as(\"kit\", Source::Path { location: \"../beta\" });\n",
        "kit",
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]);
    assert_status(&output, 1);
    let text = combined(&output);
    assert!(text.contains(PRODUCT_IMPORT_REJECTION), "{text}");
    assert!(
        text.contains("import `kit::main`") && text.contains("main.omg"),
        "{text}"
    );
    assert!(text.contains("names build dependency `kit`"), "{text}");
    assert_eq!(fixture.accepted_files(), before);

    // A product-only edge cannot serve the build import.
    let fixture = purposes_fixture(
        "    builder.depend_as(\"kit\", Source::Path { location: \"../beta\" });\n",
        "kit",
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]);
    assert_status(&output, 1);
    let text = combined(&output);
    assert!(text.contains(BUILD_IMPORT_REJECTION), "{text}");
    assert!(
        text.contains("import `kit::main`") && text.contains("build.omg"),
        "{text}"
    );
    assert!(text.contains("names product dependency `kit`"), "{text}");
    assert_eq!(fixture.accepted_files(), before);
}

/// A root-local module imported by the product source and the build source
/// checks once per scope (wiki/spec/build/scoped_execution.md — two checked
/// contexts): identical bytes, separate instances, no shared selection.
#[test]
fn a_root_local_module_checked_in_both_scopes_keeps_two_instances() {
    let fixture = Fixture::new();
    fixture.write(
        "root/build.omg",
        "use helper;\nmachine build(builder: &mut Build) {\n    builder.package(\"dual-root\");\n    helper::poke();\n}\n",
    );
    fixture.write(
        "root/main.omg",
        "use helper;\nmachine main() -> u64 { helper::answer() }\n",
    );
    fixture.write(
        "root/helper.omg",
        "module helper;\npub machine answer() -> u64 { 7 }\npub machine poke() {}\n",
    );
    let output = fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]);
    assert_status(&output, 0);
    assert!(
        fixture.accepted_files().1.is_some(),
        "update must publish omega.lock"
    );
}

#[test]
fn dropping_an_edge_after_publication_rejects_only_its_authorized_imports() {
    let fixture = purposes_fixture(CROSS_SCOPE_EDGES, "kit");
    assert_status(
        &fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]),
        0,
    );
    let published = fixture.accepted_files();
    let build = fixture.read("root/build.omg");

    // Without the product edge the product import of `kit` now names the
    // build-scope package and rejects; the build import stays authorized.
    fixture.write(
        "root/build.omg",
        &build.replace(
            "    builder.depend_as(\"kit\", Source::Path { location: \"../beta\" });\n",
            "",
        ),
    );
    let output = fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]);
    assert_status(&output, 1);
    let text = combined(&output);
    assert!(text.contains(PRODUCT_IMPORT_REJECTION), "{text}");
    assert!(
        text.contains("names build dependency `kit` (package `alpha`"),
        "{text}"
    );
    assert!(!text.contains(BUILD_IMPORT_REJECTION), "{text}");
    assert_eq!(
        fixture.accepted_files().1,
        published.1,
        "the lock is untouched"
    );

    // Without the build edge only the build import rejects.
    fixture.write(
        "root/build.omg",
        &build.replace(
            "    builder.build_depend_as(\"kit\", Source::Path { location: \"../alpha\" });\n",
            "",
        ),
    );
    let output = fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]);
    assert_status(&output, 1);
    let text = combined(&output);
    assert!(text.contains(BUILD_IMPORT_REJECTION), "{text}");
    assert!(
        text.contains("names product dependency `kit` (package `beta`"),
        "{text}"
    );
    assert!(!text.contains(PRODUCT_IMPORT_REJECTION), "{text}");
    assert_eq!(
        fixture.accepted_files().1,
        published.1,
        "the lock is untouched"
    );

    // Restoring both edges publishes again from the same declarations.
    fixture.write("root/build.omg", &build);
    assert_status(
        &fixture.omega(&["update", "--target", "linux_x86_64", "--offline"]),
        0,
    );
    assert_eq!(fixture.accepted_files().0, published.0);
    let lock = fixture.lock();
    assert_eq!(lock.targets().len(), 1);
}
