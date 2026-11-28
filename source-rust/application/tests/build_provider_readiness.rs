//! The invoker supplies a provider library before entering the Build. The Build
//! waits for the implementation it named and calls the installed requirement.

mod support;

use build_declarations::{BuildDependencySource, ProjectName};
use diagnostics::Diagnostic;
use package_manager::project::{BuildLibraryRequest, ProjectInput, open_products};
use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::PathBuf;
use target::TargetProfile;

fn fixture() -> PathBuf {
    support::repository_root().join("tests/fixtures/build/pass/explicit_provider_readiness_exit")
}

fn build_for(backing: &str) -> String {
    let source = std::fs::read_to_string(fixture().join("build.omg")).unwrap();
    assert!(source.contains("|backing|::TestImage"));
    source.replace("|backing|::TestImage", &format!("|backing|::{backing}"))
}

fn check(source: &str) -> Result<(), Vec<Diagnostic>> {
    package_compilation::toolchain_library::configure(
        &support::repository_root().join("source/library"),
    )
    .unwrap();
    let temp = TemporaryDirectory::create("supplied-provider-readiness", false).unwrap();
    let root = temp.path().join("app");
    let vendor = root.join("vendor");
    std::fs::create_dir_all(&vendor).unwrap();
    for file in ["build.omg", "main.omg"] {
        std::fs::copy(fixture().join("backing").join(file), vendor.join(file)).unwrap();
    }
    std::fs::copy(fixture().join("main.omg"), root.join("main.omg")).unwrap();
    let entry = root.join("build.omg");
    std::fs::write(&entry, source).unwrap();

    // An unregistered directory under the root is ordinary product source,
    // so the `product` package is staged only for a Build that registers it.
    if source.contains(r#"location: "product""#) {
        let product = root.join("product");
        std::fs::create_dir(&product).unwrap();
        std::fs::write(
            product.join("build.omg"),
            r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "product" }; }"#,
        )
        .unwrap();
        std::fs::write(product.join("main.omg"), "public data Item {}").unwrap();
    }

    let libraries = [BuildLibraryRequest {
        max_reach: Default::default(),
        alias: ProjectName::parse("images").unwrap(),
        source: BuildDependencySource::Path("vendor".into()),
    }];
    let target = TargetProfile::LinuxX64;
    let products = open_products(
        ProjectInput::WithBuildLibraries {
            file: &entry,
            libraries: &libraries,
        },
        &[target],
        true,
        false,
        &temp.path().join("out"),
        Default::default(),
    )?;
    products
        .select(target)?
        .product_stage(&[target])?
        .check(target, &mut Default::default())?;
    Ok(())
}

/// The refusal of the `wrong_backing` arm, which binds only a UEFI entry.
const WRONG_BACKING: &str =
    "selected target `linux_x86_64` has no bound required root slot `linux_x86_64::ProgramEntry`";

fn assert_refused(source: &str, reason: &str) {
    let errors = check(source).expect_err("the Build must be refused");
    assert!(
        errors.iter().any(|error| error.message.contains(reason)),
        "{errors:?}"
    );
}

#[test]
fn a_build_waits_for_its_named_provider_and_calls_through_it() {
    check(&build_for("TestImage")).unwrap();
}

#[test]
fn the_call_after_the_wait_goes_through_the_provider_the_build_named() {
    // `OtherImage` reports 128, so the Build takes `wrong_backing`, which
    // binds no Linux entry.
    assert_refused(&build_for("OtherImage"), WRONG_BACKING);
}

#[test]
fn a_backing_that_is_private_or_absent_is_refused() {
    for backing in ["PrivateImage", "MissingImage"] {
        assert_refused(
            &build_for(backing),
            "no visible public implementation in its supplied library",
        );
    }
}

#[test]
fn the_installed_provider_survives_a_product_dependency_wait() {
    let read = "    let observed: address = |image|::ProgramImage::retained_suffix_start();";
    let wait = r#"    let request: DependencyDescription = |dependency|::from_source(Source::Path { location: "product" });
    let registration: DependencyRegistration = Build::dependency(builder, request);
    suspend Build::prepare_dependency(builder, registration);"#;
    for read_first in [false, true] {
        let (prior, guard) = if read_first {
            (
                "    let prior: address = |image|::ProgramImage::retained_suffix_start();\n",
                "prior == observed && observed == expected_offset()",
            )
        } else {
            ("", "observed == expected_offset()")
        };
        let source = |backing| {
            let build = build_for(backing);
            assert!(build.contains(read) && build.contains("observed == 64"));
            let build = build
                .replace(read, &format!("{prior}{wait}\n{read}"))
                .replace("observed == 64", guard);
            format!("use {{omega}}/build/dependency.omg;\n{build}\nmachine expected_offset() -> address {{ 64 }}\n")
        };
        check(&source("TestImage")).unwrap();
        assert_refused(&source("OtherImage"), WRONG_BACKING);
    }
}

#[test]
fn an_untaken_arm_still_requires_its_supplied_backing() {
    let source = r#"use main.omg as app;
use {omega}/language/core/program_image.omg as image;
use {images}/main.omg as backing;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "untaken_readiness" };
    transition false {
        true -> suspend unused(builder),
        false -> finish(builder)
    }
    machine unused(builder: @mutable BuildPlan) suspends {
        suspend Build::await_provider::<|image|::ProgramImage, |backing|::TestImage>(builder);
    }
    machine finish(builder: @mutable BuildPlan) { Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main); }
}
"#;
    check(source).unwrap();
    let missing = source.replace("|backing|::TestImage", "|backing|::MissingImage");
    assert_refused(
        &missing,
        "no visible public implementation in its supplied library",
    );
}
