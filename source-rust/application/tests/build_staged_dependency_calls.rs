//! Exercise Build preparation through the CLI. A Build can use a local or a
//! loan after a wait, while a dependency name becomes selectable only after
//! its registration has been prepared.

use std::process::Command;

/// `omega --check` on a project whose Build registers `dep/` as the build
/// dependency `kit`. `before` runs before the Build waits for `kit`, `after`
/// runs after it, and `value` is what the dependency's `v()` returns.
fn check(name: &str, before: &str, after: &str, value: i32) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-staged-dependency-calls-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("dep")).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "use kit;

machine build(builder: @mut Build)
suspends
{{
    builder.tests.disabled = true;
    let mut kit_request: DependencyDescription = dependency\\from_source(Source::Path {{ location: \"dep\" }});
    dependency\\with_alias(@mut kit_request, \"kit\");
    let kit_registered: DependencyRegistration = register(builder, kit_request);
{before}
    suspend Build::prepare_dependency(builder, kit_registered);
{after}
    machine carried(builder: @mut Build) {{
        Build::bind(builder, linux_x86_64\\ProgramEntry, main);
    }}
    machine wrong(builder: @mut Build) {{
        Build::application(builder, \"second_role\");
    }}
}}
machine register(builder: @mut Build, request: DependencyDescription) -> DependencyRegistration {{
    Build::application(builder, \"staged\");
    Build::build_dependency(builder, request)
}}
"
        ),
    )
    .expect("staged Build");
    std::fs::write(
        root.join("dep/build.omg"),
        "machine build(builder: @mut Build) {
    Build::package(builder, \"kit_package\");
}
",
    )
    .expect("staged dependency Build");
    std::fs::write(
        root.join("dep/main.omg"),
        format!(
            "public machine v() -> i32 {{ {value} }}
machine hidden() -> i32 {{ 7 }}
"
        ),
    )
    .expect("staged dependency source");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The statements after the wait: call `callee`, and bind the entry only if
/// it returned 7 and the setting made before the wait is still set.
fn after(callee: &str) -> String {
    format!(
        "    let seven: i32 = {callee}();
    transition seven == 7 && builder.tests.disabled {{
        true -> carried(builder),
        _ -> wrong(builder)
    }}"
    )
}

#[test]
fn a_build_calls_the_dependency_it_prepared_and_keeps_what_it_did_before() {
    let (compiled, stderr) = check("called", "", &after("kit\\v"), 7);
    assert!(compiled, "{stderr}");
}

#[test]
fn the_value_after_the_wait_is_the_one_the_dependency_returns() {
    let (compiled, stderr) = check("other-value", "", &after("kit\\v"), 8);
    assert!(!compiled);
    assert!(stderr.contains("project role"), "{stderr}");
}

#[test]
fn preparation_keeps_the_registration_and_its_live_shared_loan() {
    // The initializer registers the project role. Repeating it fails even
    // when it supplies the same name. The second wait authenticates the token.
    let (compiled, stderr) = check(
        "captured-registration",
        "    let held: @DependencyRegistration = @kit_registered;",
        "    suspend Build::prepare_dependency(builder, $held);
    let seven: i32 = kit\\v();
    transition seven == 7 {
        true -> carried(builder),
        _ -> wrong(builder)
    }",
        7,
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn a_mutable_loan_still_names_the_original_local_after_a_wait() {
    let (compiled, stderr) = check(
        "mutable-local",
        "    let mut amount: i32 = 3;
    let loan: @mut i32 = @mut amount;",
        "    $loan = 7;
    transition amount == 7 {
        true -> carried(builder),
        _ -> wrong(builder)
    }",
        7,
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn the_build_uses_a_loan_then_calls_the_newly_visible_dependency_after_one_wait() {
    let (compiled, stderr) = check(
        "loan-and-import",
        "    let message: @[u8] = \"prepared\";
    let amount: i32 = 7;
    let held: @i32 = @amount;",
        &format!(
            "    builder.tests.disabled = $held == 7;\n    BuildLog::write_line(@mut builder.log, message);\n{}",
            after("kit\\v")
        ),
        7,
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn resumption_cannot_turn_a_guest_value_into_an_issued_registration() {
    let (compiled, stderr) = check(
        "forged-registration",
        "    let fake: DependencyRegistration = DependencyRegistration {};
    let held: @DependencyRegistration = @fake;",
        "    suspend Build::prepare_dependency(builder, $held);",
        7,
    );
    assert!(!compiled);
    assert!(
        stderr.contains("requires a registration this activation issued"),
        "{stderr}"
    );
}

#[test]
fn a_write_conflicting_with_a_loan_that_crosses_a_wait_is_refused() {
    let (compiled, stderr) = check(
        "conflicting-write",
        "    let mut amount: i32 = 3;
    let loan: @i32 = @amount;",
        "    amount = 7;
    transition $loan == 3 {
        true -> carried(builder),
        _ -> wrong(builder)
    }",
        7,
    );
    assert!(!compiled, "{stderr}");
    assert!(
        stderr.contains("storage is changed while a conflicting reference remains live"),
        "{stderr}"
    );
}

#[test]
fn a_preparation_refuses_shadowing_before_the_first_build_effects() {
    let (compiled, stderr) = check(
        "duplicate-local",
        "    Build::application(builder, \"second_role\");
    let seven: i32 = 1;",
        &after("kit\\v"),
        7,
    );
    assert!(!compiled, "{stderr}");
    assert!(
        stderr.contains("binding `seven` conflicts with a local still visible"),
        "{stderr}"
    );
}

#[test]
fn a_call_written_before_the_wait_names_nothing() {
    let (compiled, stderr) = check(
        "used-before",
        "    let early: i32 = kit\\v();",
        &after("kit\\v"),
        7,
    );
    assert!(!compiled);
    assert!(
        stderr.contains(concat!(
            "this call has no visible declaration in its source scope: ",
            "an unknown machine name is an error wherever it appears, ",
            "in a statement that never executes as in any other"
        )),
        "{stderr}"
    );
    let location = stderr.lines().map(str::trim).find_map(|line| {
        line.strip_prefix("at ")
            .and_then(|position| position.strip_suffix(":10:22"))
    });
    let expected_path = std::path::PathBuf::from(format!(
        "omega-build-staged-dependency-calls-{}-used-before",
        std::process::id()
    ))
    .join("build.omg");
    assert!(
        location.is_some_and(|path| std::path::Path::new(path).ends_with(&expected_path)),
        "the refusal identifies the call before dependency preparation: {stderr}"
    );
}

#[test]
fn a_product_dependency_is_not_selectable_from_the_build() {
    let (compiled, stderr) = check(
        "product-dependency",
        "    let mut lib_request: DependencyDescription = dependency\\from_source(Source::Path { location: \"dep\" });
    dependency\\with_alias(@mut lib_request, \"lib\");
    let lib_registered: DependencyRegistration = Build::dependency(builder, lib_request);",
        &after("lib\\v"),
        7,
    );
    assert!(!compiled);
    assert!(
        stderr.contains(concat!(
            "this call has no visible declaration in its source scope: ",
            "an unknown machine name is an error wherever it appears, ",
            "in a statement that never executes as in any other"
        )),
        "{stderr}"
    );
    let location = stderr.lines().map(str::trim).find_map(|line| {
        line.strip_prefix("in package `staged`, ")
            .and_then(|position| position.strip_suffix(":14:22"))
    });
    let expected_path = std::path::PathBuf::from(format!(
        "omega-build-staged-dependency-calls-{}-product-dependency",
        std::process::id()
    ))
    .join("build.omg");
    assert!(
        location.is_some_and(|path| std::path::Path::new(path).ends_with(&expected_path)),
        "the refusal identifies the call through a Product-only dependency: {stderr}"
    );
}

#[test]
fn a_private_machine_of_the_dependency_is_not_selectable() {
    let (compiled, _) = check("private", "", &after("kit\\hidden"), 7);
    assert!(!compiled);
}

#[test]
fn preparation_makes_a_product_dependency_ready_only_for_its_product() {
    let project = platform_custody::temporary_directory::TemporaryDirectory::create(
        "prepared-product-scope",
        false,
    )
    .unwrap();
    let root = project.path();
    std::fs::create_dir(root.join("vendor")).unwrap();
    std::fs::write(
        root.join("vendor/build.omg"),
        r#"machine build(builder: @mut Build) { Build::package(builder, "kit"); }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("vendor/main.omg"),
        "public machine value() -> i32 { 7 }",
    )
    .unwrap();
    std::fs::write(
        root.join("main.omg"),
        r#"use kit;
machine main() crashes Trap {
    transition kit\value() == 7 {
        true -> pass(),
        _ -> wrong()
    }
}
machine pass() {}
machine wrong() crashes Trap { crash Trap; }
"#,
    )
    .unwrap();
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = r#"machine build(builder: @mut Build) suspends {
    Build::application(builder, "prepared_product");
    let registered: DependencyRegistration = Build::dependency(builder, dependency\from_source(Source::Path { location: "vendor" }));
    suspend Build::prepare_dependency(builder, registered);
    Build::bind(builder, HOST_TARGET\ProgramEntry, main);
}
"#.replace("HOST_TARGET", host.target_name());
    let invoke = |contents: &str| {
        std::fs::write(root.join("build.omg"), contents).unwrap();
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["run", "--both", "build.omg"])
            .current_dir(root)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    };
    let output = invoke(&build);
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success()
            && report.contains("native exit 0")
            && report.contains("interp exit: 0"),
        "{report}"
    );

    let leaked = format!(
        "use kit;\n{}",
        build.replace(
            "    Build::bind",
            "    let leaked: i32 = kit\\value();\n    Build::bind"
        )
    );
    let output = invoke(&leaked);
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success()
            && (report.contains("no visible declaration")
                || report.contains("Product dependency")
                || report.contains("product dependency")),
        "{report}"
    );
}
