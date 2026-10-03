//! Build-purpose and product-purpose dependency edges through the shipped
//! binary (wiki/spec/build/scoped_execution.md, dependency declarations):
//! one alias may name different packages in the two scopes, one package may
//! serve both purposes under two aliases, an import in either scope selects
//! only that scope's edges, and dropping an edge after publication rejects
//! only the imports that edge authorized. Fixture packages live in
//! `tests/omega/packages/build-purposes`.

use super::fixture::{Fixture, assert_status};
use artifacts::compile_timings::counters;

const ALPHA_BUILD: &str =
    include_str!("../../../../../../tests/omega/packages/build-purposes/alpha/build.omg");
const ALPHA_MAIN: &str =
    include_str!("../../../../../../tests/omega/packages/build-purposes/alpha/main.omg");
const BETA_BUILD: &str =
    include_str!("../../../../../../tests/omega/packages/build-purposes/beta/build.omg");
const BETA_MAIN: &str =
    include_str!("../../../../../../tests/omega/packages/build-purposes/beta/main.omg");

const PRODUCT_IMPORT_REJECTION: &str = "a product import may only select product dependencies";
const BUILD_IMPORT_REJECTION: &str = "a build import may only select build dependencies";

fn nested_build_fixture() -> Fixture {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.path("generator")).unwrap();
    fixture.write(
        "generator/build.omg",
        "machine build(builder: &mut Build) { Build::package(builder, \"generator\"); builder.log.write_line(\"generator activation\"); }\n",
    );
    fixture.write(
        "generator/main.omg",
        "pub machine generation_value() -> u64 { 7 }\n",
    );
    fixture.write(
        "dependency/build.omg",
        r#"use generator::main;
machine build(builder: &mut Build) {
    Build::package(builder, "arithmetic_kernels");
    Build::build_depend_as(builder, "generator", Source::Path { location: "../generator" });
    transition generation_value() == 7 { true -> generate(builder) _ -> wrong() }
    state generate(builder: &mut Build) {
        let generated: BuildPath = builder.output.resolve("generated.omg");
        let descriptor: i32 = builder.output.create(generated, 438);
        let written: i64 = builder.output.write(descriptor, "pub machine value() -> u64 { 7 }\n");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
        builder.log.write_line("helper activation");
    }
    state wrong() {}
}
"#,
    );
    fixture.write(
        "dependency/main.omg",
        "// The public value machine is supplied by this package's build.\n",
    );
    fixture.write(
        "root/build.omg",
        r#"use kit::main;
machine build(builder: &mut Build) {
    Build::package(builder, "cli_project");
    Build::build_depend_as(builder, "kit", Source::Path { location: "../dependency" });
    transition value() == 7 { true -> received(builder) _ -> wrong() }
    state received(builder: &mut Build) { builder.log.write_line("consumer received generated answer"); }
    state wrong() {}
}
"#,
    );
    fixture
}

fn nested_build_target() -> Option<target::TargetProfile> {
    let profile = target::TargetProfile::host_if_supported();
    if profile.is_none() {
        eprintln!("skipping nested build execution: no supported build execution profile");
    }
    profile
}

#[test]
fn nested_build_cross_profile_and_dual_purpose_inputs_publish_exact_occurrences() {
    let Some(profile) = nested_build_target() else {
        return;
    };
    let foreign = if profile == target::TargetProfile::LinuxX64 {
        target::TargetProfile::MacosArm64
    } else {
        target::TargetProfile::LinuxX64
    };
    for dual_purpose in [false, true] {
        let fixture = nested_build_fixture();
        if dual_purpose {
            let build = fixture.read("root/build.omg").replace(
                "    transition value()",
                "    Build::depend_as(builder, \"product_kit\", Source::Path { location: \"../dependency\" });\n    transition value()",
            );
            fixture.write("root/build.omg", &build);
            fixture.write(
                "root/main.omg",
                "use product_kit::main;\npub machine consume() -> u64 { value() }\n",
            );
        }
        let target = if dual_purpose { profile } else { foreign };
        let before = fixture.accepted_files();
        let output = publish_generated_update(&fixture, target);
        let text = combined(&output);
        assert!(text.contains("generator activation"), "{text}");
        assert!(text.contains("helper activation"), "{text}");
        assert!(
            text.contains("consumer received generated answer"),
            "{text}"
        );
        assert_lock_published(&fixture, &before);
        assert_helper_occurrences(&fixture, target, profile, dual_purpose);
        let accepted = fixture.accepted_files();
        assert_status(
            &fixture.omega(&[
                "--check",
                "--target",
                target.target_name(),
                "--offline",
                "main.omg",
            ]),
            0,
        );
        assert_eq!(fixture.accepted_files(), accepted);
        assert!(!fixture.path("dependency/generated.omg").exists());
    }
}

/// The Build carries no target (wiki/spec/build/configuration.md), so one
/// dependency serving a build helper and the product generates one API for
/// both occurrences, each reviewed under its own target and execution profile.
#[test]
fn generated_dependency_api_has_one_shape_across_build_and_product_profiles() {
    use package_manager::declarations::DependencyPurpose;
    let Some(execution) = nested_build_target() else {
        return;
    };
    let product = if execution == target::TargetProfile::WindowsX64 {
        target::TargetProfile::LinuxX64
    } else {
        target::TargetProfile::WindowsX64
    };
    let fixture = Fixture::new();
    fixture.write("dependency/main.omg", "// Generated API only.\n");
    fixture.write(
        "dependency/build.omg",
        r#"machine build(builder: &mut Build) {
    Build::package(builder, "arithmetic_kernels");
    let generated: BuildPath = builder.output.resolve("generated_api.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(descriptor, "pub machine generated_value() -> u64 { 17 }\n");
    let closed: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
    builder.log.write_line("dependency generated u64");
}
"#,
    );
    fixture.write("root/build.omg", r#"use helper::generated_api;
machine build(builder: &mut Build) {
    Build::package(builder, "cli_project");
    Build::build_depend_as(builder, "helper", Source::Path { location: "../dependency" });
    Build::depend_as(builder, "library", Source::Path { location: "../dependency" });
    let generated: BuildPath = builder.output.resolve("consumer_generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    transition generated_value() == 17 {
        true -> correct(builder, generated, descriptor)
        _ -> incorrect(builder, generated, descriptor)
    }
    state correct(builder: &mut Build, generated: BuildPath, descriptor: i32) {
        let count: i64 = builder.output.write(descriptor, "pub machine generated_by_helper() -> u64 { 29 }");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
        builder.log.write_line("consumer received helper u64");
    }
    state incorrect(builder: &mut Build, generated: BuildPath, descriptor: i32) {
        let count: i64 = builder.output.write(descriptor, "pub machine generated_by_helper() -> u64 { 0 }");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
    }
}
"#);
    fixture.write(
        "root/main.omg",
        "use library::generated_api;\npub machine consume() -> u64 { generated_value() }\n",
    );
    let before = fixture.accepted_files();
    let output = publish_generated_update(&fixture, product);
    let text = combined(&output);
    for message in ["dependency generated u64", "consumer received helper u64"] {
        assert!(text.contains(message), "{text}");
    }
    assert_lock_published(&fixture, &before);
    assert_helper_occurrences(&fixture, product, execution, true);
    let accepted = fixture.accepted_files();
    let checked = fixture.omega(&[
        "--check",
        "--target",
        product.target_name(),
        "--offline",
        "main.omg",
    ]);
    assert_status(&checked, 0);
    assert_eq!(fixture.accepted_files(), accepted);

    let reviews = fixture.fresh_reviews(product);
    let lock = fixture.lock();
    let helper = lock
        .target(product)
        .unwrap()
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "arithmetic_kernels")
        .unwrap()
        .key();
    assert!(reviews.review(helper).is_none());
    assert_eq!(reviews.reviews_for(helper).count(), 2);
    let build_review = reviews
        .review_occurrence(helper, DependencyPurpose::Build)
        .unwrap();
    let product_review = reviews
        .review_occurrence(helper, DependencyPurpose::Product)
        .unwrap();
    let build_bundle = build_review.generated_source_bundle();
    let product_bundle = product_review.generated_source_bundle();
    assert_eq!(build_bundle.sources().len(), 1);
    assert_eq!(product_bundle.sources().len(), 1);
    for bundle in [&build_bundle, &product_bundle] {
        assert_eq!(
            bundle.sources()[0].bytes(),
            b"pub machine generated_value() -> u64 { 17 }\n"
        );
    }
    assert_eq!(build_bundle.target(), execution);
    assert_eq!(product_bundle.target(), product);
    assert_eq!(build_bundle.build_execution_profile(), Some(execution));
    assert_eq!(product_bundle.build_execution_profile(), Some(execution));
    // The consumption commitment covers the packages, edges and consumed
    // source bytes, not the occurrence's purpose or target, so one generated
    // shape commits identically in both occurrences.
    assert_eq!(
        build_review.source_consumption_commitment(),
        product_review.source_consumption_commitment()
    );
    let root = lock
        .target(product)
        .unwrap()
        .source()
        .root()
        .selected()
        .key();
    let root_bundle = reviews
        .review_occurrence(root, DependencyPurpose::Product)
        .unwrap()
        .generated_source_bundle();
    assert_eq!(root_bundle.sources().len(), 1);
    assert_eq!(
        root_bundle.sources()[0].bytes(),
        b"pub machine generated_by_helper() -> u64 { 29 }"
    );
    for relative in [
        "dependency/generated_api.omg",
        "root/consumer_generated.omg",
    ] {
        assert!(
            !fixture.path(relative).exists(),
            "generated source must stay in owned staging custody"
        );
    }
}

#[test]
fn package_risk_acceptance_does_not_transfer_between_build_and_product_purposes() {
    let Some(profile) = nested_build_target() else {
        return;
    };
    let fixture = Fixture::with_assumption();
    let product_build = "machine build(builder: &mut Build) { Build::package(builder, \"cli_project\"); Build::depend_as(builder, \"kit\", Source::Path { location: \"../dependency\" }); }\n";
    let helper_build = product_build.replace("Build::depend_as", "Build::build_depend_as");
    fixture.write("root/build.omg", product_build);
    let update = || fixture.omega(&["update", "--target", profile.target_name(), "--offline"]);
    let initial = update();
    assert_status(&initial, 3);
    let initial_reviews = review_documents(&fixture, &initial);
    write_decisions(&initial_reviews, "accept");
    assert_status(&fixture.omega(&["update", "--resume", "--offline"]), 0);
    assert_status(&update(), 0);

    fixture.write("root/build.omg", &helper_build);
    let before = fixture.accepted_files();
    let changed = update();
    assert_status(&changed, 3);
    assert_eq!(fixture.accepted_files(), before);
    let reviews = review_documents(&fixture, &changed);
    assert!(
        reviews
            .iter()
            .any(|(_, text)| { text.contains("- purposes product\n+ purposes build\n") })
    );
    assert_status(&fixture.omega(&["update", "--resume", "--offline"]), 3);
    assert_eq!(fixture.accepted_files(), before);
    write_decisions(&reviews, "reject");
    assert_status(&fixture.omega(&["update", "--resume", "--offline"]), 3);
    assert_eq!(fixture.accepted_files(), before);
    write_decisions(&reviews, "accept");
    assert_status(&fixture.omega(&["update", "--resume", "--offline"]), 0);
    let accepted = fixture.accepted_files();
    assert_ne!(accepted.1, before.1);
    assert_status(
        &fixture.omega(&[
            "--check",
            "--target",
            profile.target_name(),
            "--offline",
            "main.omg",
        ]),
        0,
    );
    assert_eq!(fixture.accepted_files(), accepted);
    assert_status(&update(), 0);

    fixture.write("root/build.omg", product_build);
    let before_reverse = fixture.accepted_files();
    let reverse = update();
    assert_status(&reverse, 3);
    assert_eq!(fixture.accepted_files(), before_reverse);
    let reverse_reviews = review_documents(&fixture, &reverse);
    assert!(
        reverse_reviews
            .iter()
            .any(|(_, text)| { text.contains("- purposes build\n+ purposes product\n") })
    );
    write_decisions(&reverse_reviews, "accept");
    // An answer for the proposed product occurrence cannot be resumed after
    // changing the authored purpose again, even back to the accepted build one.
    fixture.write("root/build.omg", &helper_build);
    let changed_again = fixture.accepted_files();
    assert_status(&fixture.omega(&["update", "--resume", "--offline"]), 1);
    assert_eq!(fixture.accepted_files(), changed_again);
}

/// Captured-input snapshots and private output staging are benign build work
/// that needs no restricted-action decision (wiki/spec/packages/acceptance.md),
/// so a generator-only update publishes without pausing for review.
fn publish_generated_update(
    fixture: &Fixture,
    target: target::TargetProfile,
) -> std::process::Output {
    let output = fixture.omega(&["update", "--target", target.target_name(), "--offline"]);
    assert_status(&output, 0);
    output
}

fn assert_helper_occurrences(
    fixture: &Fixture,
    target: target::TargetProfile,
    execution: target::TargetProfile,
    dual: bool,
) {
    use package_manager::declarations::DependencyPurpose;
    let lock = fixture.lock();
    let accepted = lock.target(target).unwrap();
    let helper = accepted
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "arithmetic_kernels")
        .unwrap();
    let occurrences = accepted
        .occurrences()
        .iter()
        .filter(|occurrence| occurrence.acceptance().package() == helper.key().identity())
        .collect::<Vec<_>>();
    assert_eq!(occurrences.len(), if dual { 2 } else { 1 });
    for occurrence in occurrences {
        let context = occurrence.context();
        assert_eq!(context.build_execution_profile(), Some(execution));
        assert_eq!(
            context.target(),
            match context.purpose() {
                DependencyPurpose::Product => {
                    assert!(dual);
                    target
                }
                DependencyPurpose::Build => execution,
            }
        );
    }
}

fn review_documents(
    fixture: &Fixture,
    output: &std::process::Output,
) -> Vec<(std::path::PathBuf, String)> {
    fixture
        .review_paths(output)
        .into_iter()
        .map(|path| {
            let document = std::fs::read_to_string(&path).unwrap();
            (path, document)
        })
        .collect()
}

fn write_decisions(documents: &[(std::path::PathBuf, String)], answer: &str) {
    for (path, document) in documents {
        let answered = document
            .lines()
            .map(|line| {
                if line.starts_with("decision ") {
                    format!("{} {answer}\n", line.strip_suffix(" pending").unwrap())
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        assert_ne!(
            answered, *document,
            "expected explicit risk-bearing decisions"
        );
        std::fs::write(path, answered).unwrap();
    }
}

#[test]
fn nested_build_cycle_rejects_before_any_activation() {
    let Some(profile) = nested_build_target() else {
        return;
    };
    let fixture = nested_build_fixture();
    fixture.write(
        "generator/build.omg",
        "machine build(builder: &mut Build) {\n    Build::package(builder, \"generator\");\n    Build::depend_as(builder, \"kit\", Source::Path { location: \"../dependency\" });\n    builder.log.write_line(\"generator activation\");\n}\n",
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["update", "--target", profile.target_name(), "--offline"]);
    assert_status(&output, 1);
    let text = combined(&output);
    assert!(text.contains("cycle"), "{text}");
    assert!(!text.contains("generator activation"), "{text}");
    assert!(!text.contains("helper activation"), "{text}");
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn nested_build_inspection_runs_prerequisites_and_uses_generated_helpers() {
    let Some(profile) = nested_build_target() else {
        return;
    };
    let fixture = nested_build_fixture();
    // Keep the requested entry distinct from the package's default main, and
    // retain a boundary declaration: inspection must not publish acceptance.
    fixture.write(
        "root/entry.omg",
        "boundary machine assumed_zero() -> u64 ensures result == 0;\nmachine inspected() {}\n",
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&[
        "inspect-terminal",
        "--machine",
        "inspected",
        "--target",
        profile.target_name(),
        "entry.omg",
    ]);
    assert_status(&output, 0);
    let text = combined(&output);
    let generator = text
        .find("generator activation")
        .expect("generator build ran");
    let helper = text.find("helper activation").expect("helper build ran");
    let consumer = text
        .find("consumer received generated answer")
        .expect("inspection retained generated helper source");
    assert!(generator < helper && helper < consumer, "{text}");
    assert!(text.contains("selected_machine=inspected"), "{text}");
    assert!(!fixture.path("dependency/generated.omg").exists());
    assert!(!fixture.path("root/omega.admissions").exists());
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn nested_build_inspection_rejects_a_failed_prerequisite_before_the_consumer() {
    let Some(profile) = nested_build_target() else {
        return;
    };
    let fixture = nested_build_fixture();
    // Physical helper source cannot excuse skipping its broken prerequisite.
    fixture.write("dependency/main.omg", "pub machine value() -> u64 { 7 }\n");
    fixture.write(
        "generator/main.omg",
        "pub machine generation_value() -> u64 { false }\n",
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&[
        "inspect-terminal",
        "--machine",
        "main",
        "--target",
        profile.target_name(),
        "main.omg",
    ]);
    assert_status(&output, 1);
    let text = combined(&output);
    assert!(text.contains("generator"), "{text}");
    assert!(
        !text.contains("consumer received generated answer"),
        "{text}"
    );
    assert!(!text.contains("selected_machine="), "{text}");
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn nested_build_sample_refresh_produces_a_runnable_native_product() {
    let Some(profile) = nested_build_target() else {
        return;
    };
    let fixture = nested_build_fixture();
    let build = fixture.read("root/build.omg").replace(
        "Build::package(builder, \"cli_project\");",
        &format!(
            "Build::application(builder, \"cli_project\");\n    builder.roots.bind({}::ProgramEntry, Main::main);",
            profile.target_name(),
        ),
    );
    fixture.write("root/build.omg", &build);
    fixture.write(
        "root/main.omg",
        "data Main {}\nmachine Main::main(&mut self) {}\n",
    );
    let before = fixture.accepted_files();
    let unreviewed = fixture.omega(&["refresh-samples", "."]);
    assert_status(&unreviewed, 1);
    assert!(combined(&unreviewed).contains("package acceptance is missing"));
    assert_eq!(fixture.accepted_files(), before);
    publish_generated_update(&fixture, profile);
    let before = fixture.accepted_files();
    let output =
        fixture.omega_with_env(&["refresh-samples", "."], &[("RUST_MIN_STACK", "2097152")]);
    assert_status(&output, 0);
    let text = combined(&output);
    assert!(
        text.contains("consumer received generated answer"),
        "{text}"
    );
    assert!(text.contains("1 of 1 samples built"), "{text}");
    let executable = fixture.path(if profile == target::TargetProfile::WindowsX64 {
        "root/build/omega-program.exe"
    } else {
        "root/build/omega-program"
    });
    let run = std::process::Command::new(&executable)
        .output()
        .expect("run the refreshed host executable");
    assert_status(&run, 0);
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/omega.admissions").exists());
    assert!(!fixture.path("dependency/generated.omg").exists());
}

#[test]
fn sample_refresh_requires_a_package_declaration() {
    let Some(_) = nested_build_target() else {
        return;
    };
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.path("root/build.omg")).unwrap();
    let before = fixture.accepted_files();
    let output = fixture.omega(&["refresh-samples", "."]);
    assert_status(&output, 1);
    let text = combined(&output);
    assert!(
        text.contains("requires a package project with a sibling build.omg"),
        "{text}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/build/omega-program").exists());
    assert!(!fixture.path("root/build/omega-program.exe").exists());
}

fn refresh_application_build(profile: target::TargetProfile) -> String {
    format!(
        "machine build(builder: &mut Build) {{\n    Build::application(builder, \"refresh_app\");\n    builder.roots.bind({}::ProgramEntry, Main::main);\n}}\n",
        profile.target_name(),
    )
}

#[test]
fn sample_refresh_reports_a_failed_sample_and_builds_its_sibling() {
    let Some(profile) = nested_build_target() else {
        return;
    };
    let fixture = Fixture::new();
    for sample in ["good", "bad"] {
        std::fs::create_dir_all(fixture.path(&format!("root/samples/{sample}"))).unwrap();
        fixture.write(
            &format!("root/samples/{sample}/build.omg"),
            &refresh_application_build(profile),
        );
    }
    fixture.write(
        "root/samples/good/main.omg",
        "data Main {}\nmachine Main::main(&mut self) {}\n",
    );
    fixture.write(
        "root/samples/bad/main.omg",
        "data Main {}\nmachine Main::main(&mut self) { let value: u64 = false; }\n",
    );
    assert_status(
        &fixture.omega(&[
            "update",
            "--project",
            "samples/good",
            "--target",
            profile.target_name(),
            "--offline",
        ]),
        0,
    );
    let output = fixture.omega(&["refresh-samples", "samples"]);
    assert_status(&output, 1);
    let text = combined(&output);
    assert!(text.contains("1 of 2 samples built"), "{text}");
    assert!(text.contains("FAILED") && text.contains("bad"), "{text}");
    let executable = if profile == target::TargetProfile::WindowsX64 {
        "omega-program.exe"
    } else {
        "omega-program"
    };
    let run =
        std::process::Command::new(fixture.path(&format!("root/samples/good/build/{executable}")))
            .output()
            .expect("run the successful sibling sample");
    assert_status(&run, 0);
    assert!(
        !fixture
            .path(&format!("root/samples/bad/build/{executable}"))
            .exists()
    );
}

#[test]
fn sample_refresh_rejects_unsettled_trust_without_rewriting_acceptance() {
    let Some(profile) = nested_build_target() else {
        return;
    };
    let fixture = Fixture::new();
    fixture.write("root/build.omg", &refresh_application_build(profile));
    fixture.write(
        "root/main.omg",
        "data Main {}\nmachine Main::main(&mut self) {}\n",
    );
    assert_status(
        &fixture.omega(&["update", "--target", profile.target_name(), "--offline"]),
        0,
    );
    let admissions = format!("{}  accepted fact: stale\n", "1".repeat(64));
    fixture.write("root/omega.admissions", &admissions);
    let before = fixture.accepted_files();
    let output = fixture.omega(&["refresh-samples", "."]);
    assert_status(&output, 1);
    assert!(combined(&output).contains("project trust admissions are not settled"));
    assert_eq!(fixture.read("root/omega.admissions"), admissions);
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/build/omega-program").exists());
    assert!(!fixture.path("root/build/omega-program.exe").exists());
}

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
            "use kit::main;\nmachine build(builder: &mut Build) {{\n    Build::package(builder, \"purpose_root\");\n{build_edges}}}\n"
        ),
    );
    fixture.write(
        "root/main.omg",
        &format!("use {product_alias}::main;\nmachine main() -> u64 {{ beta_value() }}\n"),
    );
    fixture
}

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

/// SOURCE-SET-UNION acceptance: one unioned two-target check shares a single
/// prepared source closure — each package's parse frontier is built once for
/// the request — while every target still reports its own checked outcome.
/// Nested (generator -> dependency -> root) and dual-purpose (the dependency
/// also serves the product import) generated-source custody ride the union.
#[test]
fn two_target_check_assembles_the_source_union_once() {
    use compiler::CompileOptions;
    use omega::compilation::{CompileProjectRequest, ProjectProduct, compile_project_for_targets};
    let Some(execution) = nested_build_target() else {
        return;
    };
    let foreign = if execution == target::TargetProfile::LinuxX64 {
        target::TargetProfile::MacosArm64
    } else {
        target::TargetProfile::LinuxX64
    };
    let fixture = nested_build_fixture();
    let build = fixture.read("root/build.omg").replace(
        "    transition value()",
        "    Build::depend_as(builder, \"product_kit\", Source::Path { location: \"../dependency\" });\n    transition value()",
    );
    fixture.write("root/build.omg", &build);
    fixture.write(
        "root/main.omg",
        "use product_kit::main;\npub machine consume() -> u64 { value() }\n",
    );
    let target_names = [
        execution.target_name().to_owned(),
        foreign.target_name().to_owned(),
    ];
    let check_request = || {
        let mut request = CompileProjectRequest::new(CompileOptions {
            root_path: fixture.path("root/main.omg"),
            build_dir: Some(fixture.path("union-check")),
            target_name: None,
        });
        request.product = ProjectProduct::Check;
        request.offline = true;
        request.timings = true;
        request
    };

    let assemblies_before = counters::source_assembly_invocation_count();
    let outcomes = compile_project_for_targets(check_request(), &target_names)
        .expect("a two-target request returns one outcome per target");
    let union_assemblies = counters::source_assembly_invocation_count() - assemblies_before;
    assert_eq!(outcomes.len(), 2);
    for outcome in &outcomes {
        let checked = outcome.result.as_ref().unwrap_or_else(|error| {
            panic!("target {} failed: {error}", outcome.target.target_name())
        });
        assert!(checked.report.source_file_count > 0);
        // Shared work is measured once and copied into every target report.
        assert!(
            !checked.timings.phases().is_empty(),
            "target {} report lost the shared timing ladder",
            outcome.target.target_name()
        );
    }
    // Four source contexts: generator Build, dependency Build and Product,
    // and root Product. A later single-target request can carry dependency
    // reviews from disk, so its assembly count is not a cold baseline. The
    // second target may carry the Build contexts too; neither carry nor copied
    // timing rows changes how many source assemblies this request executes.
    assert_eq!(
        union_assemblies, 4,
        "each purpose-scoped source set assembles once"
    );
    assert!(
        !fixture.path("dependency/generated.omg").exists(),
        "generated source stays in staged custody"
    );
}

/// A fixture-private source identity keeps source-count assertions independent
/// of dependency reviews that another test may have persisted.
fn add_private_standard_library(fixture: &Fixture) {
    fn copy_omega_sources(source: &std::path::Path, destination: &std::path::Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let destination = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_omega_sources(&path, &destination);
            } else if path.extension().is_some_and(|extension| extension == "omg") {
                std::fs::copy(path, destination).unwrap();
            }
        }
    }
    let standard_library = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|ancestor| ancestor.join("source/library/std"))
        .find(|path| path.join("build.omg").is_file())
        .expect("repository standard library");
    copy_omega_sources(&standard_library, &fixture.path("standard"));
    let build = fixture.read("standard/build.omg");
    fixture.write(
        "standard/build.omg",
        &format!(
            "{build}\n// unique source-union fixture: {:?}\n",
            fixture.path("standard")
        ),
    );
}

/// Every entry contract is already supplied by one package. Nominating Linux
/// and macOS separately must not rebuild the application's identical source
/// forest. A fixture-private source copy makes the dependency's review cold
/// without mutating process-wide cache settings or relying on test order.
#[test]
fn two_target_application_shares_entry_contract_source_assembly() {
    use compiler::CompileOptions;
    use omega::compilation::{CompileProjectRequest, ProjectProduct, compile_project_for_targets};
    if nested_build_target().is_none() {
        return;
    }
    let fixture = Fixture::new();
    add_private_standard_library(&fixture);
    fixture.write(
        "root/build.omg",
        r#"machine build(builder: &mut Build) {
    Build::application(builder, "entry_source_union");
    Build::depend(builder, Source::Path { location: "../standard" });
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
"#,
    );
    fixture.write(
        "root/main.omg",
        "data Main {}\nmachine Main::main(&mut self) {}\n",
    );
    let mut request = CompileProjectRequest::new(CompileOptions {
        root_path: fixture.path("root/main.omg"),
        build_dir: Some(fixture.path("union-check")),
        target_name: None,
    });
    request.product = ProjectProduct::Check;
    request.offline = true;
    let assemblies_before = counters::source_assembly_invocation_count();
    let outcomes = compile_project_for_targets(
        request,
        &["linux_x86_64".to_owned(), "macos_arm64".to_owned()],
    )
    .expect("one application outcome per requested target");
    let assemblies = counters::source_assembly_invocation_count() - assemblies_before;
    assert_eq!(outcomes.len(), 2);
    for outcome in outcomes {
        let checked = outcome.result.unwrap_or_else(|error| {
            panic!("target {} failed: {error}", outcome.target.target_name())
        });
        assert!(checked.report.source_file_count > 0);
    }
    assert_eq!(assemblies, 2, "one library and one application source set");
}

/// The emitted route consumes the same source union as Check, then realizes
/// each target independently. Review the fixture in a child-only cache so the
/// measured request is cold without changing this test process's environment.
#[test]
fn two_target_native_products_share_nested_source_union() {
    use compiler::CompileOptions;
    use omega::compilation::{CompileProjectRequest, ProjectProduct, compile_project_for_targets};
    let Some(execution) = nested_build_target() else {
        return;
    };
    let foreign = if execution == target::TargetProfile::LinuxX64 {
        target::TargetProfile::MacosArm64
    } else {
        target::TargetProfile::LinuxX64
    };
    let fixture = nested_build_fixture();
    add_private_standard_library(&fixture);
    let dependency_build = fixture.read("dependency/build.omg").replace(
        "pub machine value() -> u64 { 7 }",
        "pub machine value() -> i32 { 7 }",
    );
    fixture.write("dependency/build.omg", &dependency_build);
    let build = fixture
        .read("root/build.omg")
        .replace(
            "Build::package(builder, \"cli_project\");",
            &format!(
                "Build::application(builder, \"source_union_native\");\n    \
                 Build::depend(builder, Source::Path {{ location: \"../standard\" }});\n    \
                 builder.roots.bind({}::ProgramEntry, Main::main);\n    \
                 builder.roots.bind({}::ProgramEntry, Main::main);",
                execution.target_name(),
                foreign.target_name(),
            ),
        )
        .replace(
            "    transition value()",
            "    Build::depend_as(builder, \"product_kit\", Source::Path { location: \"../dependency\" });\n    transition value()",
        );
    fixture.write("root/build.omg", &build);
    fixture.write(
        "root/main.omg",
        r#"use product_kit::main;
use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega::language::core::binding;
data Main { exit: Binding<ProcessExit>; }
machine Main::main(&mut self)
reaches
    ProcessExit
{
    self.exit.exit_process(value());
}
"#,
    );
    let acceptance_cache = fixture.path("acceptance-cache");
    std::fs::create_dir(&acceptance_cache).unwrap();
    let acceptance_cache = acceptance_cache.to_str().unwrap();
    // These are the resolver's supported host locations. They affect only the
    // offline fixture subprocess: Linux uses XDG, Windows LOCALAPPDATA, macOS
    // HOME/Library/Caches. The parent retains its normal immutable cache root.
    let acceptance_environment = [
        ("XDG_CACHE_HOME", acceptance_cache),
        ("LOCALAPPDATA", acceptance_cache),
        ("HOME", acceptance_cache),
    ];
    let review = fixture.omega_with_env(
        &[
            "update",
            "--target",
            execution.target_name(),
            "--target",
            foreign.target_name(),
            "--offline",
        ],
        &acceptance_environment,
    );
    assert_status(&review, 3);
    write_decisions(&review_documents(&fixture, &review), "accept");
    assert_status(
        &fixture.omega_with_env(
            &["update", "--resume", "--offline"],
            &acceptance_environment,
        ),
        0,
    );
    let accepted = fixture.accepted_files();
    let measured_cache = fixture.path("pure-build-measurement-cache");
    std::fs::create_dir(&measured_cache).unwrap();
    let measured_directory = fixture.path("pure-build-measurement-native");
    let measured_cache = measured_cache.to_str().unwrap();
    let measured = fixture.omega_with_env(
        &[
            "--target",
            execution.target_name(),
            "--target",
            foreign.target_name(),
            "--build-dir",
            measured_directory.to_str().unwrap(),
            "--offline",
            "main.omg",
        ],
        &[
            ("XDG_CACHE_HOME", measured_cache),
            ("LOCALAPPDATA", measured_cache),
            ("HOME", measured_cache),
        ],
    );
    assert_status(&measured, 0);
    let measured_output = String::from_utf8_lossy(&measured.stdout);
    assert_eq!(
        measured_output
            .lines()
            .filter(|line| *line == "consumer received generated answer")
            .count(),
        1,
        "the cold root Pure activation executes once, not once per target/pass: {measured_output}"
    );
    let measured_products = measured_output
        .lines()
        .filter_map(|line| line.strip_prefix("published native output to "))
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    assert_eq!(measured_products.len(), 2, "{measured_output}");
    for path in &measured_products {
        assert!(path.metadata().unwrap().len() > 0);
    }
    let measured_host = measured_products
        .iter()
        .find(|path| path.starts_with(measured_directory.join(execution.target_name())))
        .expect("measured host artifact");
    assert_status(
        &std::process::Command::new(measured_host).output().unwrap(),
        7,
    );

    let build_directory = fixture.path("union-native");
    let mut request = CompileProjectRequest::new(CompileOptions {
        root_path: fixture.path("root/main.omg"),
        build_dir: Some(build_directory.clone()),
        target_name: None,
    });
    request.product = ProjectProduct::NativeArtifact;
    request.offline = true;
    let assemblies_before = counters::source_assembly_invocation_count();
    let outcomes = compile_project_for_targets(
        request,
        &[
            execution.target_name().to_owned(),
            foreign.target_name().to_owned(),
        ],
    )
    .expect("one native outcome per requested target");
    let assemblies = counters::source_assembly_invocation_count() - assemblies_before;
    assert_eq!(outcomes.len(), 2);
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| outcome.target == execution)
            .count(),
        1,
        "the requested host target occurs exactly once"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| outcome.target == foreign)
            .count(),
        1,
        "the requested foreign target occurs exactly once"
    );
    let mut host_executable = None;
    for outcome in outcomes {
        let product = outcome.result.unwrap_or_else(|error| {
            panic!("target {} failed: {error}", outcome.target.target_name())
        });
        let executable = product
            .executable_path
            .expect("native output was published");
        assert!(executable.starts_with(build_directory.join(outcome.target.target_name())));
        assert!(executable.metadata().unwrap().len() > 0);
        if outcome.target == execution {
            host_executable = Some(executable);
        }
    }
    // Generator Build, dependency Build and Product, standard-library Product,
    // and root Product each own one source context. Build/Product custody is
    // distinct even where bytes match; target entry nominations do not add one.
    assert_eq!(
        assemblies, 5,
        "each cold purpose-scoped source set assembles once"
    );
    let run = std::process::Command::new(host_executable.expect("host native product"))
        .output()
        .expect("run the host product consuming the generated dependency");
    assert_status(&run, 7);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.is_empty());
    assert_eq!(fixture.accepted_files(), accepted);
    assert!(!fixture.path("dependency/generated.omg").exists());
    assert!(!fixture.path("root/generated.omg").exists());
}

#[test]
fn an_import_selects_only_its_own_scope_edges() {
    // A build-only edge cannot serve the product import.
    let fixture = purposes_fixture(
        "    Build::build_depend_as(builder, \"kit\", Source::Path { location: \"../beta\" });\n",
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
        "    Build::depend_as(builder, \"kit\", Source::Path { location: \"../beta\" });\n",
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
        "use helper;\nmachine build(builder: &mut Build) {\n    Build::package(builder, \"dual_root\");\n    helper::poke();\n}\n",
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
