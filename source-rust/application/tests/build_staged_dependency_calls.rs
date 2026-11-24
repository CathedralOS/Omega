//! Exercise Build dependency waits through the CLI, including the values, loans
//! and host-issued registrations a Build can use after it wakes.

use std::process::Command;

/// Check code on both sides of a Product dependency wait. The local Build helper
/// supplies `value` independently of the package whose preparation we await.
fn check(name: &str, before: &str, after: &str, value: i32) -> (bool, String) {
    check_with_dependency_build(name, before, after, value, "", &["--check", "main.omg"])
}

fn check_with_dependency_build(
    name: &str,
    before: &str,
    after: &str,
    value: i32,
    dependency_body: &str,
    arguments: &[&str],
) -> (bool, String) {
    let target = target::TargetProfile::host_if_supported().expect("a hosted test target");
    let target = target.target_name();
    let build = format!(
        "use support/support.omg as amounts;

machine build(builder: @mutable Build)
suspends
{{
    builder.tests.disabled = true;
    let mutable kit_request: DependencyDescription = dependency\\from_source(Source::Path {{ location: \"dep\" }});
    dependency\\with_alias(@mutable kit_request, \"kit\");
    let kit_registered: DependencyRegistration = register(builder, kit_request);
{before}
    suspend Build::prepare_dependency(builder, kit_registered);
{after}
    machine carried(builder: @mutable Build) {{
        Build::bind(builder, {target}\\ProgramEntry, main);
    }}
    machine wrong(builder: @mutable Build) {{
        Build::application(builder, \"second_role\");
    }}
}}
machine register(builder: @mutable Build, request: DependencyDescription) -> DependencyRegistration {{
    Build::application(builder, \"staged\");
    Build::dependency(builder, request)
}}
machine needs_seven(value: i32)
requires value == 7
{{ }}
machine choose_amount(flag: bool) -> i32
ensures result >= 7 && result <= 9
{{ match flag {{ true -> 9, _ -> 7 }} }}
machine needs_range(value: i32) requires value >= 7 && value <= 9 {{ }}
machine initial_amount(builder: @mutable Build) -> i32 {{
    builder.tests.disabled = !builder.tests.disabled;
    3
}}
"
    );
    let helper = format!("public machine value() -> i32 {{ {value} }}");
    check_project_with_support(
        name,
        &build,
        value,
        dependency_body,
        Some(&helper),
        arguments,
    )
}

fn check_project(
    name: &str,
    build: &str,
    value: i32,
    dependency_body: &str,
    arguments: &[&str],
) -> (bool, String) {
    check_project_with_support(name, build, value, dependency_body, None, arguments)
}

fn check_project_with_support(
    name: &str,
    build: &str,
    value: i32,
    dependency_body: &str,
    support: Option<&str>,
    arguments: &[&str],
) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-staged-dependency-calls-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("dep")).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(root.join("build.omg"), build).expect("staged Build");
    std::fs::write(
        root.join("counts.omg"),
        format!(
            "public data Number {{ value: i32; }}\npublic machine value() -> i32 {{ {value} }}"
        ),
    )
    .expect("local Build helper");
    std::fs::write(
        root.join("dep/build.omg"),
        format!(
            "machine build(builder: @mutable Build) {{
    Build::package(builder, \"kit_package\");
{dependency_body}
}}
"
        ),
    )
    .expect("staged dependency Build");
    std::fs::write(
        root.join("dep/main.omg"),
        format!(
            "public data Number {{ value: i32; }}
public machine v() -> i32 {{ {value} }}
machine hidden() -> i32 {{ 7 }}
"
        ),
    )
    .expect("staged dependency source");
    if let Some(support) = support {
        std::fs::create_dir(root.join("support")).expect("Build helper directory");
        std::fs::write(root.join("support/support.omg"), support).expect("Build helper source");
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
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
fn an_anonymous_build_selects_its_own_constant() {
    let target = target::TargetProfile::host_if_supported().expect("a hosted test target");
    let build = format!(
        r#"const CHOSEN: bool = false;
machine build(builder: @mutable Build) {{
    Build::application(builder, "constant_custody");
    builder.tests.disabled = CHOSEN;
    Build::bind(builder, {}\ProgramEntry, main);
}}"#,
        target.target_name(),
    );
    let (compiled, report) = check_project(
        "anonymous-constant",
        &build,
        7,
        "",
        &["--check", "main.omg"],
    );
    assert!(compiled, "{report}");
}

#[test]
fn a_build_keeps_its_setting_while_it_waits_for_a_product_dependency() {
    let (compiled, stderr) = check("called", "", &after("|amounts|::value"), 7);
    assert!(compiled, "{stderr}");
}

#[test]
fn the_build_uses_the_value_its_local_helper_returns_after_the_wait() {
    let (compiled, stderr) = check("other-value", "", &after("|amounts|::value"), 8);
    assert!(!compiled);
    assert!(stderr.contains("project role"), "{stderr}");
    assert!(
        stderr.contains("while resuming the Build after dependency preparation"),
        "{stderr}"
    );
}

#[test]
fn preparation_keeps_the_registration_and_its_live_shared_loan() {
    // The initializer registers the project role. Repeating it fails even
    // when it supplies the same name. The second wait authenticates the token.
    let (compiled, stderr) = check(
        "captured-registration",
        "    let held: @DependencyRegistration = @kit_registered;",
        "    suspend Build::prepare_dependency(builder, $held);
    let seven: i32 = |amounts|::value();
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
        "    let mutable amount: i32 = 3;
    let loan: @mutable i32 = @mutable amount;",
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
fn a_build_uses_its_loan_and_local_helper_after_a_product_wait() {
    let (compiled, stderr) = check(
        "loan-and-import",
        "    let message: @[u8] = \"prepared\";
    let amount: i32 = 7;
    let held: @i32 = @amount;",
        &format!(
            "    builder.tests.disabled = $held == 7;\n    BuildLog::write_line(@mutable builder.log, message);\n{}",
            after("|amounts|::value")
        ),
        7,
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn a_wait_preserves_the_proof_for_an_already_resolved_call() {
    let (compiled, report) = check(
        "retained-proof",
        "    let amount: i32 = 7;",
        &format!("    needs_seven(amount);\n{}", after("|amounts|::value")),
        7,
    );
    assert!(compiled, "{report}");
}

#[test]
fn a_prepared_call_and_an_earlier_borrow_are_visible_together() {
    let (ran, report) = check_with_dependency_build(
        "live-loan-and-package",
        r#"    let mutable amount: i32 = initial_amount(builder);
    amount = 7;
    let held: @i32 = @amount;"#,
        r#"    let seven: i32 = |amounts|::value();
    needs_seven($held);
    transition seven == $held && !builder.tests.disabled {
        true -> carried(builder),
        _ -> wrong(builder)
    }"#,
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
}

#[test]
fn a_write_through_the_loan_invalidates_the_value_proved_before_preparation() {
    let (compiled, report) = check(
        "overwritten-frame-fact",
        "    let mutable amount: i32 = 7;\n    let held: @mutable i32 = @mutable amount;",
        r#"    let seven: i32 = |amounts|::value();
    $held = 8;
    needs_seven(amount);
    carried(builder);"#,
        7,
    );
    assert!(
        !compiled && report.contains("cannot prove requires contract for call needs_seven"),
        "{report}"
    );
}

#[test]
fn preparation_preserves_only_the_proved_range_of_a_local() {
    // At execution the flag selects 7. The checker has only the range from
    // the call, so only the first of these two contracts may use the value.
    for (callee, allowed) in [("needs_range", true), ("needs_seven", false)] {
        let (compiled, report) = check(
            callee,
            "    let amount: i32 = choose_amount(builder.optimizations.human_report != 0u8);",
            &format!(
                r#"    let seven: i32 = |amounts|::value();
    {callee}(amount);
    carried(builder);"#
            ),
            7,
        );
        assert_eq!(compiled, allowed, "{report}");
        if !allowed {
            assert!(
                report.contains("cannot prove requires contract for call needs_seven"),
                "{report}"
            );
        }
    }
}

#[test]
fn build_helpers_are_checked_at_the_call_site_in_both_file_layouts() {
    let target = target::TargetProfile::host_if_supported()
        .unwrap()
        .target_name();
    let helpers = r"machine seven() -> i32 { read_count() }
    machine read_count() -> i32 { |counts|::value() }";
    for (layout, import, call, nested, support, file) in [
        ("nested", "use counts.omg as counts;", "seven()", helpers, None, "build.omg:"),
        (
            "imported",
            "use support/support.omg as support;",
            r"|support|::seven()",
            "",
            Some(format!("use counts.omg as counts;\npublic {helpers}")),
            "support.omg:",
        ),
    ] {
        let build = format!(
            r#"{import}
machine build(builder: @mutable Build) suspends {{
    announce(builder);
    let mutable request: DependencyDescription = dependency\from_source(Source::Path {{ location: "dep" }});
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    let expected: i32 = 7;
    suspend Build::prepare_dependency(builder, registered);
    transition {call} == expected {{ true -> bind(builder), _ -> wrong(builder) }}
    machine announce(builder: @mutable Build) {{ set_role(builder); }}
    machine set_role(builder: @mutable Build) {{ Build::application(builder, "staged"); }}
    machine bind(builder: @mutable Build) {{ Build::bind(builder, {target}\ProgramEntry, main); }}
    machine wrong(builder: @mutable Build) {{ Build::application(builder, "wrong"); }}
    {nested}
}}
"#
        );
        let run = |case: &str, build: &str, support: Option<&str>| {
            check_project_with_support(
                &format!("helpers-{layout}-{case}"),
                build,
                7,
                "",
                support,
                &["run", "--both", "build.omg"],
            )
        };
        let (ran, output) = run("after", &build, support.as_deref());
        assert!(
            ran && output.contains("native exit 0") && output.contains("interp exit: 0"),
            "{layout}: {output}"
        );

        let early = build.replace(
            "set_role(builder);",
            &format!("let early: i32 = {call}; set_role(builder);"),
        );
        let (ran, output) = run("before", &early, support.as_deref());
        assert!(
            ran && output.contains("native exit 0") && output.contains("interp exit: 0"),
            "{layout}: {output}"
        );

        // The helper file supplies both the type and the constructor from the start.
        for (form, expression) in [
            (
                "annotation",
                r"let item: |counts|::Number = |counts|::Number { value: 7 }; item.value",
            ),
            (
                "constructor",
                r"(|counts|::Number { value: 7 }).value",
            ),
        ] {
            let type_helper = |text: &str| text.replace(r"|counts|::value()", expression);
            let type_support = support.as_deref().map(type_helper);
            let (ran, output) = run(
                &format!("{form}-after"),
                &type_helper(&build),
                type_support.as_deref(),
            );
            assert!(
                ran && output.contains("native exit 0") && output.contains("interp exit: 0"),
                "{layout} {form}: {output}"
            );
            let (ran, output) = run(
                &format!("{form}-before"),
                &type_helper(&early),
                type_support.as_deref(),
            );
            assert!(
                ran && output.contains("native exit 0") && output.contains("interp exit: 0"),
                "{layout} {form}: {output}"
            );
        }

        for (case, unused, diagnostic) in [
            (
                "unused-name",
                r"machine unused() { let bad: i32 = missing\v(); }",
                "no visible declaration in its source scope",
            ),
            (
                "unused-overflow",
                "machine unused(n: u64) -> u64 { n + 1u64 }",
                "may overflow `u64`",
            ),
        ] {
            let declarations = format!("{helpers}\n    {unused}");
            let invalid_build = build.replace(helpers, &declarations);
            let invalid_support = support
                .as_ref()
                .map(|body| body.replace(helpers, &declarations));
            let (ran, output) = run(case, &invalid_build, invalid_support.as_deref());
            assert!(
                !ran && output.contains(diagnostic)
                    && output.contains("unused`")
                    && output.contains(file)
                    && !output.contains("while resuming"),
                "{layout}: {output}"
            );
        }
    }
}

#[test]
fn resumed_calls_use_proved_relations_between_the_captured_locals() {
    let helpers = "machine next(n: i32) -> i32 requires n <= 9 ensures result > n { n + 1 }
machine require_related(a: i32, b: i32, larger: i32, flag: bool, second: bool, enabled: bool)
requires a == b && a < larger && flag == second && enabled { }";
    // Both flags happen to be false at run time. Only copying the original
    // result establishes equality for every possible input.
    for (name, other, accepted) in [
        ("copied-local", "amount", true),
        (
            "independent-local",
            "choose_amount(builder.optimizations.human_report != 0u8)",
            false,
        ),
    ] {
        let before = format!(
            "    let flag: bool = builder.optimizations.human_report != 0u8;
    let second: bool = flag;
    let enabled: bool = true;
    let amount: i32 = choose_amount(!builder.tests.disabled);
    let expected: i32 = {other};
    let larger: i32 = next(amount);"
        );
        let target = target::TargetProfile::host_if_supported()
            .unwrap()
            .target_name();
        let build = format!(
            r#"use counts.omg as counts;
machine build(builder: @mutable Build) suspends {{
    Build::application(builder, "staged");
    builder.tests.disabled = true;
    let mutable request: DependencyDescription = dependency\from_source(Source::Path {{ location: "dep" }});
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
{before}
    suspend Build::prepare_dependency(builder, registered);
    let seven: i32 = |counts|::value();
    require_related(amount, expected, larger, flag, second, enabled);
    suspend Build::prepare_dependency(builder, registered);
    Build::bind(builder, {target}\ProgramEntry, main);
}}
machine choose_amount(flag: bool) -> i32 ensures result >= 7 && result <= 9 {{
    match flag {{ true -> 9, _ -> 7 }}
}}
{helpers}
"#
        );
        let (ran, output) = check_project(name, &build, 7, "", &["run", "--both", "build.omg"]);
        assert_eq!(ran, accepted, "{output}");
        if accepted {
            assert!(
                output.contains("native exit 0") && output.contains("interp exit: 0"),
                "{output}"
            );
        } else {
            assert!(
                output.contains("cannot prove requires contract for call require_related"),
                "{output}"
            );
        }
    }
}

#[test]
fn a_write_after_the_wait_conflicts_with_a_shared_loan_of_its_destination() {
    let (compiled, report) = check(
        "live-loan-conflict-in-extended-scope",
        "    let mutable amount: i32 = 3;\n    let held: @i32 = @amount;",
        r#"    amount = |amounts|::value();
    transition $held == 3 { true -> carried(builder), _ -> wrong(builder) }"#,
        7,
    );
    assert!(
        !compiled
            && report.contains("storage is changed while a conflicting reference remains live"),
        "{report}"
    );
}

#[test]
fn a_live_loan_also_prevents_writes_before_preparation() {
    let (compiled, report) = check(
        "live-loan-conflict-in-prefix",
        "    let mutable amount: i32 = 3;\n    let held: @i32 = @amount;\n    amount = 7;",
        r#"    let seven: i32 = |amounts|::value();
    transition $held == seven { true -> carried(builder), _ -> wrong(builder) }"#,
        7,
    );
    assert!(
        !compiled
            && report.contains("storage is changed while a conflicting reference remains live"),
        "{report}"
    );
}

#[test]
fn a_resumed_mutable_loan_updates_its_own_home() {
    let (ran, report) = check_with_dependency_build(
        "two-live-homes",
        r#"    let other: i32 = 3;
    let mutable amount: i32 = 1;
    let held: @mutable i32 = @mutable amount;"#,
        r#"    let seven: i32 = |amounts|::value();
    $held = seven;
    transition amount == 7 && other == 3 { true -> carried(builder), _ -> wrong(builder) }"#,
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
}

#[test]
fn registration_and_a_mutable_local_survive_the_same_preparation() {
    let after_wait = format!(
        "    $held = 7;\n    builder.tests.disabled = amount == 7;\n    suspend Build::prepare_dependency(builder, kit_registered);\n{}",
        after("|amounts|::value")
    );
    let (ran, report) = check_with_dependency_build(
        "registration-and-mutable-local",
        "    let mutable amount: i32 = 3;\n    let held: @mutable i32 = @mutable amount;",
        &after_wait,
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
}

#[test]
fn a_local_helper_can_run_before_the_build_releases_its_live_borrow() {
    let (ran, report) = check_with_dependency_build(
        "call-with-live-borrow",
        "    let mutable amount: i32 = 3;\n    let held: @mutable i32 = @mutable amount;",
        "    let seven: i32 = |amounts|::value();\n    $held = seven;\n    suspend Build::prepare_dependency(builder, kit_registered);\n    transition amount == 7 {\n        true -> carried(builder),\n        _ -> wrong(builder)\n    }",
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
}

#[test]
fn a_build_keeps_the_byte_its_helper_read_before_preparation() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = r#"use counts.omg as counts;
machine middle(bytes: @[u8]) -> u8 requires bytes.length == 3 { bytes[1] }
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "staged_byte_read");
    let mutable request: DependencyDescription = dependency\from_source(Source::Path { location: "dep" });
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    let byte: u8 = middle("kit");
    suspend Build::prepare_dependency(builder, registered);
    let seven: i32 = |counts|::value();
    transition byte == 105u8 && seven == 7 {
        true -> done(builder),
        _ -> wrong(builder)
    }
    machine done(builder: @mutable Build) { Build::bind(builder, HOST\ProgramEntry, main); }
    machine wrong(builder: @mutable Build) { Build::application(builder, "wrong_byte"); }
}
"#.replace("HOST", target.target_name());
    let (ran, report) = check_project(
        "byte-before-wait",
        &build,
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
}

#[test]
fn a_field_borrow_survives_preparing_a_product_dependency() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let source = include_str!("fixtures/build/run/borrowed_field_after_mount/build.omg").replace(
        "linux_x86_64\\ProgramEntry",
        &format!("{}\\ProgramEntry", target.target_name()),
    );
    let (ran, report) = check_project(
        "borrowed-field",
        &source,
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );

    let conflict = source.replace("$held = seven;", "gauge.amount = seven;");
    let (ran, report) = check_project(
        "borrowed-field-conflict",
        &conflict,
        7,
        "",
        &["--check", "main.omg"],
    );
    assert!(
        !ran && report.contains("storage is changed while a conflicting reference remains live")
            && report.contains("build.omg:19:"),
        "{report}"
    );
}

#[test]
fn a_suspending_helper_returns_its_local_value_after_preparation() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let original = include_str!("fixtures/build/run/borrowed_field_after_mount/build.omg")
        .replace("\r\n", "\n")
        .replace(
            "linux_x86_64\\ProgramEntry",
            &format!("{}\\ProgramEntry", target.target_name()),
        );
    let wait = "suspend Build::prepare_dependency(builder, registration);";
    let mut build = original.replace(
        &format!("{wait}\n    let seven: i32 = helper_value();"),
        "let seven: i32 = suspend ready(builder, registration);",
    );
    assert_ne!(build, original);
    build.push_str(&format!(
        "\nmachine ready(builder: @mutable Build, registration: DependencyRegistration) -> i32 suspends {{\n    {wait}\n    helper_value()\n}}\n"
    ));
    let (ok, report) = check_project(
        "suspending-helper-result",
        &build,
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ok && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
    let wrong = build.replace(
        "machine helper_value() -> i32 { 7 }",
        "machine helper_value() -> i32 { 8 }",
    );
    assert_ne!(wrong, build);
    let (ok, report) = check_project(
        "wrong-suspending-helper-result",
        &wrong,
        7,
        "",
        &["--check", "main.omg"],
    );
    assert!(!ok && report.contains("project role"), "{report}");
}

#[test]
fn a_transition_can_use_the_result_of_a_suspending_build_helper() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = r#"use counts.omg as counts;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "ready_subject");
    let mutable request: DependencyDescription = dependency\from_source(Source::Path { location: "dep" });
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    SUBJECT {
        true -> finish(builder),
        _ -> wrong(builder)
    }
    machine finish(builder: @mutable Build) { Build::bind(builder, HOST\ProgramEntry, main); }
    machine wrong(builder: @mutable Build) { Build::application(builder, "wrong_dependency_value"); }
}
machine ready(builder: @mutable Build, item: DependencyRegistration) -> bool suspends {
    suspend Build::prepare_dependency(builder, item);
    |counts|::value() == 7
}
"#.replace("HOST", target.target_name());
    let call = "suspend ready(builder, registered)";
    let direct = build.replace("SUBJECT", &format!("transition {call}"));
    let bound = build.replace(
        "SUBJECT",
        &format!("let ready_now: bool = {call};\n    transition ready_now"),
    );
    // The second subject can only be evaluated after the first one returns.
    let pair = build
        .replace("SUBJECT", &format!(r"transition ({call}, |counts|::value())"))
        .replace("true ->", "(_, 7) ->")
        .replace("_ ->", "(_, _) ->");
    for (name, source) in [
        ("bound-subject", &bound),
        ("direct-subject", &direct),
        ("subsequent-subject", &pair),
    ] {
        let (ran, report) = check_project(name, source, 7, "", &["run", "--both", "build.omg"]);
        assert!(
            ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
            "{report}"
        );
    }
    let (ran, report) = check_project("false-subject", &direct, 8, "", &["--check", "main.omg"]);
    assert!(!ran && report.contains("project role"), "{report}");
}

#[test]
fn a_helper_prepares_a_package_with_the_callers_borrow_still_live() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let original = include_str!("fixtures/build/run/borrowed_field_after_mount/build.omg").replace(
        "linux_x86_64\\ProgramEntry",
        &format!("{}\\ProgramEntry", target.target_name()),
    );
    let wait = "suspend Build::prepare_dependency(builder, registration);";
    for (name, inside) in [("helper-wait", false), ("helper-calls-local", true)] {
        let mut source = original.replace(wait, "suspend ready(builder, registration);");
        let body = if inside {
            "builder.tests.disabled = helper_value() == 7;"
        } else {
            ""
        };
        source.push_str(&format!("\nmachine ready(builder: @mutable Build, registration: DependencyRegistration) suspends {{\n    {wait}\n    {body}\n}}\n"));
        let (ran, report) = check_project(name, &source, 7, "", &["run", "--both", "build.omg"]);
        assert!(
            ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
            "{report}"
        );
    }
}

#[test]
fn a_build_helper_can_use_its_imported_type_on_both_sides_of_a_wait() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let project = platform_custody::temporary_directory::TemporaryDirectory::create(
        "prepared-helper-type",
        false,
    )
    .unwrap();
    let root = project.path();
    std::fs::create_dir(root.join("vendor")).unwrap();
    std::fs::create_dir(root.join("support")).unwrap();
    std::fs::write(
        root.join("vendor/build.omg"),
        r#"machine build(builder: @mutable Build) {
        Build::package(builder, "scores");
    }"#,
    )
    .unwrap();
    std::fs::write(root.join("types.omg"), "public data Score = i32;").unwrap();
    std::fs::write(root.join("vendor/main.omg"), "public data Exported = i32;").unwrap();
    std::fs::write(root.join("main.omg"), "machine main() {}").unwrap();
    std::fs::write(
        root.join("support/helpers.omg"),
        r#"use types.omg as types;
public machine score() -> i32 {
    let seven: |types|::Score = 7;
    seven
}"#,
    )
    .unwrap();
    let build = r#"use support/helpers.omg as support;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "prepared_type");
    let mutable request: DependencyDescription = dependency\from_source(Source::Path { location: "vendor" });
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    suspend Build::prepare_dependency(builder, registered);
    let score: i32 = |support|::score();
    transition score == 7 { true -> finish(builder), _ -> wrong(builder) }
    machine finish(builder: @mutable Build) { Build::bind(builder, HOST\ProgramEntry, main); }
    machine wrong(builder: @mutable Build) { Build::application(builder, "duplicate"); }
}"#.replace("HOST", target.target_name());
    let run = |program: &str| {
        std::fs::write(root.join("build.omg"), program).unwrap();
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["run", "--both", "build.omg"])
            .current_dir(root)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    };
    let result = run(&build);
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.status.success()
            && report.contains("native exit 0")
            && report.contains("interp exit: 0"),
        "{report}"
    );
    let wait = "    suspend Build::prepare_dependency(builder, registered);\n";
    let call = "    let score: i32 = |support|::score();\n";
    let early = build.replace(&format!("{wait}{call}"), &format!("{call}{wait}"));
    assert_ne!(early, build);
    let result = run(&early);
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.status.success() && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
}

#[test]
fn each_transition_arm_can_prepare_its_own_dependency() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let project = platform_custody::temporary_directory::TemporaryDirectory::create(
        "alternative-preparations",
        false,
    )
    .unwrap();
    let root = project.path();
    std::fs::write(root.join("main.omg"), "machine main() {}").unwrap();
    for (directory, name, value) in [("left_vendor", "alpha", 7), ("right_vendor", "beta", 13)] {
        std::fs::write(root.join(format!("{name}.omg")),
            format!("public machine value() -> i32 {{ {value} }}")).unwrap();
        let package = root.join(directory);
        std::fs::create_dir(&package).unwrap();
        std::fs::write(
            package.join("build.omg"),
            format!(
                "machine build(builder: @mutable Build) {{ Build::package(builder, \"{name}\"); }}"
            ),
        )
        .unwrap();
        std::fs::write(
            package.join("main.omg"),
            format!("public machine value() -> i32 {{ {value} }}"),
        )
        .unwrap();
    }
    let build = r#"use alpha.omg as alpha;
use beta.omg as beta;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "alternatives");
    builder.tests.disabled = FLAG;
    transition builder.tests.disabled {
        true -> suspend left(builder),
        _ -> suspend right(builder)
    }
    machine left(builder: @mutable Build) suspends {
        let item: DependencyRegistration = Build::dependency(builder, dependency\from_source(Source::Path { location: "left_vendor" }));
        suspend Build::prepare_dependency(builder, item);
        let actual: i32 = |alpha|::value();
        transition actual == 7 { true -> done(builder), _ -> wrong(builder) }
    }
    machine right(builder: @mutable Build) suspends {
        let item: DependencyRegistration = Build::dependency(builder, dependency\from_source(Source::Path { location: "right_vendor" }));
        suspend Build::prepare_dependency(builder, item);
        let actual: i32 = |beta|::value();
        transition actual == 13 { true -> done(builder), _ -> wrong(builder) }
    }
    machine done(builder: @mutable Build) { Build::bind(builder, HOST\ProgramEntry, main); }
    machine wrong(builder: @mutable Build) { Build::application(builder, "duplicate"); }
}"#.replace("HOST", target.target_name());
    for (flag, unused_error) in [("true", false), ("false", false), ("true", true)] {
        let mut selected = build.replace("FLAG", flag);
        if unused_error {
            selected.push_str("\nmachine unused(n: u64) -> u64 { n + 1u64 }\n");
        }
        std::fs::write(root.join("build.omg"), selected).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["run", "--both", "build.omg"])
            .current_dir(root)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if unused_error {
            assert!(
                !output.status.success()
                    && report.contains("may overflow")
                    && report.contains("unused"),
                "{report}"
            );
        } else {
            assert!(
                output.status.success()
                    && report.contains("native exit 0")
                    && report.contains("interp exit: 0"),
                "{report}"
            );
        }
    }
}

#[test]
fn resumption_preserves_arithmetic_and_float_contracts() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = r#"use counts.omg as counts;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "staged_proofs");
    let mutable request: DependencyDescription = dependency\from_source(Source::Path { location: "dep" });
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    let start: i32 = pick(builder.tests.disabled);
    let after: i32 = increment(start);
    let measured: f64 = reading(builder.optimizations.human_report != 0u8);
    suspend Build::prepare_dependency(builder, registered);
    let value: i32 = |counts|::value();
    require_step(start, after);
    require_small(measured);
    Build::bind(builder, HOST\ProgramEntry, main);
}
machine pick(flag: bool) -> i32 ensures result >= 7 && result <= 9 { match flag { true -> 7, _ -> 9 } }
machine increment(value: i32) -> i32 requires value <= 9 ensures result == value + 1 { value + 1 }
machine require_step(start: i32, after: i32) requires start <= 9 && after == start + 1 {}
machine reading(flag: bool) -> f64 ensures result >= 0.0 && result <= 0.5 { match flag { true -> 0.25, _ -> 0.5 } }
machine require_small(x: f64) requires x >= 0.0 && x <= 0.5 {}
"#.replace("HOST", target.target_name());
    let run =
        |name, source: &str| check_project(name, source, 7, "", &["run", "--both", "build.omg"]);
    let (ran, report) = run("staged-proof", &build);
    assert!(
        ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
    for (name, before, after, contract) in [
        (
            "staged-proof-arithmetic",
            "increment(start)",
            "increment(pick(builder.optimizations.human_report != 0u8))",
            "require_step",
        ),
        (
            "staged-proof-float",
            "reading(builder.optimizations.human_report != 0u8)",
            "0.75",
            "require_small",
        ),
    ] {
        let (ran, report) = run(name, &build.replace(before, after));
        assert!(
            !ran && report.contains(&format!(
                "cannot prove requires contract for call {contract}"
            )),
            "{report}"
        );
    }
}

#[test]
fn only_the_selected_transition_arm_waits_for_its_dependency() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = r#"use counts.omg as counts;
const CHOSEN: bool = FLAG;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "conditional");
    let mutable request: DependencyDescription = dependency\from_source(Source::Path { location: "dep" });
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    builder.tests.disabled = CHOSEN;
    transition builder.tests.disabled {
        true -> suspend ready(builder, registered),
        _ -> finish(builder)
    }
    machine ready(builder: @mutable Build, registered: DependencyRegistration) suspends {
        suspend Build::prepare_dependency(builder, registered);
        let value: i32 = |counts|::value();
        transition value == 7 { true -> finish(builder), _ -> wrong(builder) }
    }
    machine finish(builder: @mutable Build) { Build::bind(builder, HOST\ProgramEntry, main); }
    machine wrong(builder: @mutable Build) { Build::application(builder, "duplicate_role"); }
}"#.replace("HOST", target.target_name());
    for (flag, value) in [("true", 7), ("false", 8)] {
        let (ran, report) = check_project(
            &format!("conditional-wait-{flag}"),
            &build.replace("FLAG", flag),
            value,
            "",
            &["run", "--both", "build.omg"],
        );
        assert!(
            ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
            "{report}"
        );
    }
    let wrong_local = build.replace("FLAG", "false").replace(
        r"let value: i32 = |counts|::value();",
        r"let value: i32 = |counts|::value(); let wrong_type: i32 = true;",
    );
    let (ran, report) = check_project(
        "conditional-skip-known-type",
        &wrong_local,
        8,
        "",
        &["--check", "main.omg"],
    );
    assert!(
        !ran && report.contains("wrong_type") && report.contains("requires `i32`"),
        "{report}"
    );
    let unchecked = format!(
        "{}\nmachine unused(n: u64) -> u64 {{ n + 1u64 }}",
        build.replace("FLAG", "false")
    );
    let (ran, report) = check_project(
        "conditional-skip-unused",
        &unchecked,
        8,
        "",
        &["--check", "main.omg"],
    );
    assert!(
        !ran && report.contains("may overflow") && report.contains("unused"),
        "{report}"
    );
    let (ran, report) = check_project(
        "conditional-wait-bad",
        &build.replace("FLAG", "true"),
        8,
        "",
        &["--check", "main.omg"],
    );
    assert!(!ran && report.contains("project role"), "{report}");
}

#[test]
fn an_open_reader_and_writer_keep_their_cursor_and_staged_bytes_across_preparation() {
    let before = r#"    let input: BuildPath = BuildSource::resolve(@builder.source, "main.omg");
    let reader: i32 = BuildSource::open(@builder.source, input, 0);
    let mutable first: [u8; 1] = [0u8];
    let first_count: i64 = BuildSource::read(@builder.source, reader, @mutable first[..], 1u64);
    let saw_first: bool = first_count == 1i64 && first[0] == 109u8;
    let promised: RequiredOutput = BuildOutput::require(@mutable builder.output, "joined.txt");
    let output: BuildPath = BuildOutput::resolve(@builder.output, "joined.txt");
    let writer: i32 = BuildOutput::create(@mutable builder.output, output, 0);
    _ = BuildOutput::write(@mutable builder.output, writer, "machine joined() -> i32 { ");"#;
    let after = r#"    let selected: i32 = |amounts|::value();
    let mutable second: [u8; 1] = [0u8];
    let second_count: i64 = BuildSource::read(@builder.source, reader, @mutable second[..], 1u64);
    _ = BuildSource::close(@builder.source, reader);
    let denied: i32 = BuildSource::open(@builder.source, input, 1);
    _ = BuildOutput::write(@mutable builder.output, writer, "7i32 }");
    _ = BuildOutput::close(@mutable builder.output, writer);
    Build::include_source(builder, "joined.omg", "machine joined() -> i32 { 7i32 }");
    let ready: bool = selected == 7 && saw_first && second_count == 1i64 && second[0] == 97u8 && denied == -1;
    let completion: OutputCompletion = BuildOutput::complete(@mutable builder.output, promised, output);
    transition completion {
        OutputCompletion::Sealed { receipt } -> checked(builder, ready),
        OutputCompletion::Retry { obligation, file } -> failed(builder, obligation)
    }
    machine failed(builder: @mutable Build, obligation: RequiredOutput) {
        BuildOutput::fail(@mutable builder.output, obligation, "staged bytes did not settle");
    }
    machine checked(builder: @mutable Build, ready: bool) {
        transition ready {
            true -> carried(builder), _ -> wrong(builder)
        }
    }"#;
    let (ran, report) = check_with_dependency_build(
        "open-files-across-mount",
        before,
        after,
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ran && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
    let output = platform_custody::temporary_directory::TemporaryDirectory::create(
        "open-writer-completed-output",
        false,
    )
    .unwrap();
    let (built, report) = check_with_dependency_build(
        "open-files-export",
        before,
        after,
        7,
        "",
        &["--build-dir", output.path().to_str().unwrap(), "build.omg"],
    );
    assert!(built, "{report}");
    let completed = report
        .lines()
        .find_map(|line| line.strip_prefix("published completed build outputs to "))
        .expect("the Build must export the file it completed");
    assert_eq!(
        std::fs::read(std::path::Path::new(completed).join("files/joined.txt")).unwrap(),
        b"machine joined() -> i32 { 7i32 }",
    );

    let forged = after.replace(
        "promised, output);",
        "promised, BuildPath {});",
    );
    let (ran, report) = check_with_dependency_build(
        "unissued-output-path-after-mount",
        before,
        &forged,
        7,
        "",
        &["--check", "build.omg"],
    );
    assert!(
        !ran && report.contains("filesystem value was not issued by this activation"),
        "{report}"
    );
}

#[test]
fn an_issued_output_keeps_its_borrowed_path_and_obligation_across_preparation() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = include_str!("fixtures/build/run/required_output_after_mount/build.omg").replace(
        "linux_x86_64\\ProgramEntry",
        &format!("{}\\ProgramEntry", target.target_name()),
    );
    let (ran, report) = check_project(
        "issued-output-after-wait",
        &build,
        7,
        "",
        &["run", "--both", "build.omg"],
    );
    assert!(
        ran && report.contains("interp exit: 0") && report.contains("native exit 0"),
        "{report}"
    );

    // Settling the same obligation with a fresh, guest-constructed path must fail.
    let forged = build.replace("obligation, checked);", "obligation, BuildPath {});");
    assert_ne!(forged, build);
    let (ran, report) = check_project(
        "unissued-output-after-wait",
        &forged,
        7,
        "",
        &["--check", "build.omg"],
    );
    assert!(
        !ran && report.contains("Build filesystem value was not issued by this activation")
            && report.contains("build.omg:17:")
            && !report.contains("ExecutionSite")
            && !report.contains("EffectRejected"),
        "{report}"
    );
}

#[test]
fn replacing_the_output_facet_before_a_wait_does_not_reissue_it() {
    let (ran, report) = check(
        "replaced-output",
        "    builder.output = BuildOutput {};",
        "    builder.tests.disabled = |amounts|::value() == 7;\n    let path: BuildPath = BuildOutput::resolve(@builder.output, \"answer.dat\");",
        7,
    );
    assert!(
        !ran && report.contains("original compiler-issued output"),
        "{report}"
    );
}

#[test]
fn copies_of_an_issued_registration_still_name_the_same_request_after_a_wait() {
    let (ran, report) = check(
        "copied-registration-after-mount",
        "    let copy: DependencyRegistration = kit_registered;\n    let held: @DependencyRegistration = @copy;",
        &format!(
            "    builder.tests.disabled = |amounts|::value() == 7;\n    suspend Build::prepare_dependency(builder, $held);\n    suspend Build::prepare_dependency(builder, kit_registered);\n{}",
            after("|amounts|::value"),
        ),
        7,
    );
    assert!(ran, "{report}");
}

#[test]
fn a_local_call_after_a_wait_does_not_end_a_live_shared_borrow() {
    let (ran, report) = check(
        "conflict-after-mounted-call",
        "    let mutable amount: i32 = 3;\n    let held: @i32 = @amount;",
        "    let seven: i32 = |amounts|::value();\n    amount = seven;\n    builder.tests.disabled = $held == 7;",
        7,
    );
    assert!(
        !ran && report.contains("storage is changed while a conflicting reference remains live")
            && report.contains("build.omg:14:14")
            && !report.contains("while resuming the Build after dependency preparation"),
        "{report}"
    );
}

#[test]
fn a_user_record_named_dependency_registration_keeps_its_own_fields() {
    let target = target::TargetProfile::host_if_supported().expect("a hosted test target");
    let target = target.target_name();
    let build = format!(
        r#"use counts.omg as counts;
data DependencyRegistration [copy] {{ mark: u32; }}
machine build(builder: @mutable Build) suspends {{
    Build::application(builder, "staged");
    let mutable request: DependencyDescription = dependency\from_source(Source::Path {{ location: "dep" }});
    dependency\with_alias(@mutable request, "kit");
    let shadow: DependencyRegistration = DependencyRegistration {{ mark: 7u32 }};
    let real = Build::dependency(builder, request);
    suspend Build::prepare_dependency(builder, real);
    builder.tests.disabled = shadow.mark == 7u32;
    let seven: i32 = |counts|::value();
    transition seven == 7 && builder.tests.disabled {{
        true -> carried(builder), _ -> wrong(builder)
    }}
    machine carried(builder: @mutable Build) {{ Build::bind(builder, {target}\ProgramEntry, main); }}
    machine wrong(builder: @mutable Build) {{ Build::application(builder, "second"); }}
}}
"#
    );
    let (ran, report) = check_project(
        "same-spelled-user-record",
        &build,
        7,
        "",
        &["--check", "main.omg"],
    );
    assert!(ran, "{report}");
}

#[test]
fn resumption_cannot_turn_a_guest_value_into_an_issued_registration() {
    let fake = "    let fake: DependencyRegistration = DependencyRegistration {};\n    let held: @DependencyRegistration = @fake;";
    let replaced = "    let mutable slot: DependencyRegistration = kit_registered;\n    slot = DependencyRegistration {};\n    let held: @DependencyRegistration = @slot;";
    let mounted = "    builder.tests.disabled = |amounts|::value() == 7;\n";
    for (name, before, use_dependency) in [
        ("forged-registration", fake, ""),
        ("forged-after-mount", fake, mounted),
        ("overwritten-before-mount", replaced, mounted),
    ] {
        let (compiled, stderr) = check(
            name,
            before,
            &format!("{use_dependency}    suspend Build::prepare_dependency(builder, $held);"),
            7,
        );
        assert!(!compiled);
        assert!(
            stderr.contains("requires a registration this activation issued"),
            "{stderr}"
        );
    }
}

#[test]
fn a_write_conflicting_with_a_loan_that_crosses_a_wait_is_refused() {
    let (compiled, stderr) = check(
        "conflicting-write",
        "    let mutable amount: i32 = 3;
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
        &after("|amounts|::value"),
        7,
    );
    assert!(!compiled, "{stderr}");
    assert!(
        stderr.contains("an earlier local with this name is still visible") && stderr.contains("build.omg:13:9"),
        "{stderr}"
    );
}

#[test]
fn a_local_resolution_error_after_a_wait_is_reported_before_the_dependency_runs() {
    let (compiled, report) = check_with_dependency_build(
        "duplicate-after-wait",
        "    let amount: i32 = 7;",
        "    let seven: i32 = 7;
    let seven: i32 = 9;
    needs_seven(amount);",
        7,
        "    unknown_in_dependency();",
        &["--check", "main.omg"],
    );
    assert!(!compiled, "{report}");
    assert!(report.contains("duplicate local data `seven`"), "{report}");
    assert!(!report.contains("while resuming"), "{report}");
    assert!(!report.contains("while preparing the dependency"), "{report}");
}

#[test]
fn a_build_can_call_its_local_helper_before_and_after_a_product_wait() {
    let (compiled, report) = check(
        "helper-before-and-after",
        "    let early: i32 = |amounts|::value();",
        "    let late: i32 = |amounts|::value();
    transition early == 7 && early == late { true -> carried(builder), _ -> wrong(builder) }",
        7,
    );
    assert!(compiled, "{report}");
}

#[test]
fn a_product_dependency_is_not_selectable_from_the_build() {
    let (compiled, stderr) = check(
        "product-dependency",
        "    let mutable lib_request: DependencyDescription = dependency\\from_source(Source::Path { location: \"dep\" });
    dependency\\with_alias(@mutable lib_request, \"lib\");
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
    assert!(
        stderr.contains(
            "while reading the signature of `build`"
        ),
        "{stderr}"
    );
    let location = stderr.lines().map(str::trim).find_map(|line| {
        line.strip_prefix("at ")
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
fn a_dependency_build_error_reports_the_childs_source() {
    let (compiled, stderr) = check_with_dependency_build(
        "invalid-dependency-build",
        "",
        &after("|amounts|::value"),
        7,
        "    unknown();",
        &["--check", "main.omg"],
    );
    assert!(!compiled);
    assert!(
        stderr.contains("this call has no visible declaration in its source scope"),
        "{stderr}"
    );
    assert!(
        stderr.contains("during lowered-form checking while preparing the dependency"),
        "{stderr}"
    );
    let prefix =
        "machine build(builder: @mutable Build) {\n    Build::package(builder, \"kit_package\");\n    ";
    assert!(
        stderr.contains(&format!(
            "source bytes {}..{}",
            prefix.len(),
            prefix.len() + "unknown".len()
        )),
        "{stderr}"
    );
    let expected = std::path::Path::new("dep").join("build.omg");
    assert!(
        stderr.contains(&format!("{}:3:5", expected.display())),
        "{stderr}"
    );
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
        r#"machine build(builder: @mutable Build) { Build::package(builder, "kit"); }"#,
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
    let build = r#"machine build(builder: @mutable Build) suspends {
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
            && report.contains("Build import `kit` has no supplied source")
            && report.contains("build.omg:1:5"),
        "{report}"
    );
    let unknown = leaked
        .replace("use kit;", "use kitchen;")
        .replace("kit\\value()", "kitchen\\value()");
    let output = invoke(&unknown);
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success()
            && report.contains("Build import `kitchen` has no supplied source")
            && report.contains("build.omg:1:5"),
        "{report}"
    );
}

#[test]
fn an_unused_unbound_import_is_refused_at_the_authored_name() {
    let build = "use {absent}/helper.omg as missing;\nmachine build(builder: @mutable Build) { Build::application(builder, \"never_used\"); Build::bind(builder, linux_x86_64\\ProgramEntry, main); }\n";
    let (compiled, report) = check_project(
        "unused-deferred",
        build,
        7,
        "",
        &["--check", "main.omg", "--target", "linux_x86_64"],
    );
    assert!(
        !compiled
            && report.contains("Build import `{absent}/helper.omg as missing` has no supplied source")
            && report.contains("build.omg:1:5"),
        "{report}"
    );
}

#[test]
fn a_wait_cannot_add_a_library_to_the_checked_build() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = r#"use {kit}/main.omg as chosen;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "fixed_imports");
    let mutable request: DependencyDescription = dependency\from_source(Source::Path { location: "dep" });
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    suspend Build::prepare_dependency(builder, registered);
    let value: i32 = |chosen|::v();
    Build::bind(builder, HOST\ProgramEntry, main);
}"#.replace("HOST", host.target_name());
    let call = |name, source: &str| {
        check_project_with_support(
            name,
            source,
            7,
            "",
            Some("public machine v() -> i32 { 7 }"),
            &["--check", "main.omg"],
        )
    };
    let (ok, report) = call(
        "local-helper",
        &build.replace("{kit}/main.omg", "support/support.omg"),
    );
    assert!(ok, "{report}");
    for (name, source) in [
        ("product-only", build.clone()),
        (
            "self-registration",
            build.replace("Build::dependency(", "Build::build_dependency("),
        ),
    ] {
        let (ok, report) = call(name, &source);
        assert!(
            !ok && report
                .contains("Build import `{kit}/main.omg as chosen` has no supplied source")
                && report.contains("build.omg:1:5"),
            "{report}"
        );
    }
}

#[test]
fn a_build_binds_its_products_file_without_being_able_to_call_it() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let project = platform_custody::temporary_directory::TemporaryDirectory::create(
        "product-file-operand",
        false,
    )
    .unwrap();
    let root = project.path();
    let kit = root.join("vendor");
    std::fs::create_dir(&kit).unwrap();
    std::fs::write(
        kit.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "kit"); }"#,
    )
    .unwrap();
    std::fs::write(kit.join("entry.omg"), "public machine start() {}").unwrap();
    std::fs::write(
        root.join("main.omg"),
        "machine unreachable() crashes Trap { crash Trap; }",
    )
    .unwrap();
    let build = r#"use {kit}/entry.omg as selected;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "product_file");
    let registered: DependencyRegistration = Build::dependency(builder, dependency\from_source(Source::Path { location: "vendor" }));
    suspend Build::prepare_dependency(builder, registered);
    Build::bind(builder, HOST\ProgramEntry, |selected|::start);
}"#.replace("HOST", host.target_name());
    let invoke = |source: &str, args: &[&str]| {
        std::fs::write(root.join("build.omg"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(args)
            .current_dir(root)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.success(), report)
    };
    let (ok, report) = invoke(&build, &["run", "--both", "build.omg"]);
    assert!(
        ok && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
    let with_unused = format!("use {{absent}}/other.omg as unused;\n{build}");
    let (ok, report) = invoke(&with_unused, &["--check", "build.omg"]);
    assert!(
        !ok && report.contains("Build import `{absent}/other.omg as unused` has no supplied source")
            && !report.contains("Build import `{kit}/entry.omg as selected` has no supplied source"),
        "{report}"
    );
    let called = build.replace(
        "    Build::bind",
        "    |selected|::start();\n    Build::bind",
    );
    let (ok, report) = invoke(&called, &["--check", "build.omg"]);
    assert!(
        !ok && report.contains("Build import `{kit}/entry.omg as selected` has no supplied source")
            && report.contains("build.omg:1:5"),
        "{report}"
    );
}

#[test]
fn a_dependency_publishes_source_bytes_under_the_name_the_product_imports() {
    let target = target::TargetProfile::host_if_supported()
        .unwrap()
        .target_name();
    let build = r#"use {kit}/generated/start.omg as entry;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "generated_entry");
    let mutable source: DependencyDescription = dependency\from_source(Source::Path { location: "dep" });
    dependency\with_alias(@mutable source, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, source);
    suspend Build::prepare_dependency(builder, registered);
    Build::bind(builder, HOST\ProgramEntry, |entry|::start);
}"#.replace("HOST", target);
    let publish = r#"Build::include_source(builder, "generated/start.omg", "use main.omg as seed; public machine start() crashes Trap { transition |seed|::v() == 7 { true -> done(), _ -> bad() } machine done() {} machine bad() { crash Trap; } }");"#;
    let run =
        |name, body: &str| check_project(name, &build, 7, body, &["run", "--both", "build.omg"]);
    let (ok, report) = run("published-source-bytes", publish);
    assert!(
        ok && report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
    let outside = publish.replace("generated/start.omg", "../start.omg");
    let (ok, report) = run("published-source-outside", &outside);
    assert!(
        !ok && report.contains("invalid generated source filename `../start.omg`"),
        "{report}"
    );
}

#[test]
fn a_build_cannot_publish_the_same_filename_again_after_a_dependency_wait() {
    let publish = r#"Build::include_source(builder, "generated/value.omg", "const SEVEN: i32 = 7;");"#;
    let resumed = format!("{publish}\n    carried(builder);");
    let (ok, report) = check("duplicate-source-after-wait", publish, &resumed, 7);
    assert!(
        !ok && report.contains("generated/value.omg") && report.contains("already"),
        "{report}"
    );
}
