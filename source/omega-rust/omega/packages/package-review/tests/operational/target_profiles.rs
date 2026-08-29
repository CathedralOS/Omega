use crate::support::*;

#[test]
fn review_rejects_target_free_and_standalone_checked_programs() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");

    let target_free = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        None,
        package_inputs(&package.0),
    )
    .expect("target-free package fixture should check");
    let diagnostics = project_checked_package_review(&target_free)
        .expect_err("review must require an explicit target");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires one explicit target selection")
    }));

    let standalone = omega_compiler::compile_to_checked(&package.0.join("main.omg"), None)
        .expect("standalone fixture should check");
    let diagnostics = project_checked_package_review(&standalone)
        .expect_err("review must require package-aware compilation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires package-aware checked compilation")
    }));
}

#[test]
fn review_distinguishes_profiles_that_share_a_native_target() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target uefi_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let windows = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("Windows review fixture should check");
    let uefi = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("uefi_x64"),
        package_inputs(&package.0),
    )
    .expect("UEFI review fixture should check");

    assert_eq!(
        windows.selected_native_target(),
        uefi.selected_native_target()
    );
    let windows = project_checked_package_review(&windows).expect("Windows review projection");
    let uefi = project_checked_package_review(&uefi).expect("UEFI review projection");
    assert_eq!(windows.target(), omega_target::TargetProfile::WindowsX64);
    assert_eq!(uefi.target(), omega_target::TargetProfile::UefiX64);
    assert_ne!(windows.target(), uefi.target());
    assert_ne!(
        windows.canonical_review_bytes().expect("Windows encoding"),
        uefi.canonical_review_bytes().expect("UEFI encoding"),
    );
}

#[test]
fn review_encoding_ignores_unreviewed_arena_insertion_order() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write("main.omg", "boundary machine host_ping();\n");
    second.write(
        "main.omg",
        "machine unrelated() { }\nboundary machine host_ping();\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("arena-order fixture should check")
    };
    let first = project_checked_package_review(&compile(&first))
        .expect("first arena-order review")
        .canonical_review_bytes()
        .expect("first arena-order encoding");
    let second = project_checked_package_review(&compile(&second))
        .expect("second arena-order review")
        .canonical_review_bytes()
        .expect("second arena-order encoding");

    assert_eq!(first, second);
}
