//! Exercise Build dependency waits through the CLI, including the values, loans
//! and host-issued registrations a Build can use after it wakes.

#[path = "support/project.rs"]
mod project;

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
        "use main.omg as app;
use support/support.omg as amounts;

use {{omega}}/build/dependency.omg;
machine build(builder: @mutable BuildPlan)
suspends
{{
    builder.tests.disabled = true;
    let mutable kit_request: DependencyDescription = |dependency|::from_source(Source::Path {{ location: \"dep\" }});
    |dependency|::with_alias(@mutable kit_request, \"kit\");
    let kit_registered: DependencyRegistration = register(builder, kit_request);
{before}
    suspend Build::prepare_dependency(builder, kit_registered);
{after}
    machine carried(builder: @mutable BuildPlan) {{
        Build::bind(builder, {target}::ProgramEntry, |app|::main);
    }}
    machine wrong(builder: @mutable BuildPlan) {{
        builder.product = BuildProduct::Unselected;
    }}
}}
machine register(builder: @mutable BuildPlan, request: DependencyDescription) -> DependencyRegistration {{
    builder.product = BuildProduct::Application {{ name: \"staged\" }};
    Build::dependency(builder, request)
}}
machine needs_seven(value: i32)
requires value == 7
{{ }}
machine choose_amount(flag: bool) -> i32
ensures result >= 7 && result <= 9
{{ match flag {{ true -> 9, _ -> 7 }} }}
machine needs_range(value: i32) requires value >= 7 && value <= 9 {{ }}
machine initial_amount(builder: @mutable BuildPlan) -> i32 {{
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
    let counts = format!(
        "public data Number {{ value: i32; }}\npublic machine value() -> i32 {{ {value} }}"
    );
    let dependency_build = format!(
        "machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Package {{ name: \"kit_package\" }};
{dependency_body}
}}
"
    );
    let dependency_main = format!(
        "public data Number {{ value: i32; }}
public machine v() -> i32 {{ {value} }}
machine hidden() -> i32 {{ 7 }}
"
    );
    let scratch = project::stage(
        name,
        &[
            ("main.omg", "machine main() { }\n"),
            ("build.omg", build),
            ("counts.omg", &counts),
            ("dep/build.omg", &dependency_build),
            ("dep/main.omg", &dependency_main),
        ],
    );
    if let Some(support) = support {
        project::write(scratch.path(), &[("support/support.omg", support)]);
    }
    let output = project::observe(scratch.path(), arguments);
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
        r#"use main.omg as app;
const CHOSEN: bool = false;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "constant_custody" }};
    builder.tests.disabled = CHOSEN;
    Build::bind(builder, {}::ProgramEntry, |app|::main);
}}"#,
        target.target_name(),
    );
    let scratch = project::stage(
        "anonymous-constant",
        &[("build.omg", &build), ("main.omg", "machine main() {}")],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(scratch.path())
        .args(["--check", "main.omg"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    for command in ["run", "build"] {
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
            &[command, "build.omg"],
        );
        assert!(ran, "{report}");
    }
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
        (
            "nested",
            "use counts.omg as counts;",
            "seven()",
            helpers,
            None,
            "build.omg:",
        ),
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
            r#"use main.omg as app;
{import}
use {{omega}}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {{
    announce(builder);
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path {{ location: "dep" }});
    |dependency|::with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    let expected: i32 = 7;
    suspend Build::prepare_dependency(builder, registered);
    transition {call} == expected {{ true -> bind(builder), _ -> wrong(builder) }}
    machine announce(builder: @mutable BuildPlan) {{ set_role(builder); }}
    machine set_role(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Application {{ name: "staged" }}; }}
    machine bind(builder: @mutable BuildPlan) {{ Build::bind(builder, {target}::ProgramEntry, |app|::main); }}
    machine wrong(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Unselected; }}
    {nested}
}}
"#
        );
        for command in ["run", "build"] {
            let run = |case: &str, build: &str, support: Option<&str>| {
                check_project_with_support(
                    &format!("helpers-{layout}-{case}"),
                    build,
                    7,
                    "",
                    support,
                    &[command, "build.omg"],
                )
            };
            let (ran, output) = run("after", &build, support.as_deref());
            assert!(ran, "{layout}: {output}");

            let early = build.replace(
                "set_role(builder);",
                &format!("let early: i32 = {call}; set_role(builder);"),
            );
            let (ran, output) = run("before", &early, support.as_deref());
            assert!(ran, "{layout}: {output}");

            // The helper file supplies both the type and the constructor from the start.
            for (form, expression) in [
                (
                    "annotation",
                    r"let item: |counts|::Number = |counts|::Number { value: 7 }; item.value",
                ),
                ("constructor", r"(|counts|::Number { value: 7 }).value"),
            ] {
                let type_helper = |text: &str| text.replace(r"|counts|::value()", expression);
                let type_support = support.as_deref().map(type_helper);
                let (ran, output) = run(
                    &format!("{form}-after"),
                    &type_helper(&build),
                    type_support.as_deref(),
                );
                assert!(ran, "{layout} {form}: {output}");
                let (ran, output) = run(
                    &format!("{form}-before"),
                    &type_helper(&early),
                    type_support.as_deref(),
                );
                assert!(ran, "{layout} {form}: {output}");
            }

            for (case, unused, diagnostic) in [
                (
                    "unused-name",
                    r"machine unused() { let bad: i32 = |missing|::v(); }",
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
            r#"use main.omg as app;
use counts.omg as counts;
use {{omega}}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {{
    builder.product = BuildProduct::Application {{ name: "staged" }};
    builder.tests.disabled = true;
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path {{ location: "dep" }});
    |dependency|::with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
{before}
    suspend Build::prepare_dependency(builder, registered);
    let seven: i32 = |counts|::value();
    require_related(amount, expected, larger, flag, second, enabled);
    suspend Build::prepare_dependency(builder, registered);
    Build::bind(builder, {target}::ProgramEntry, |app|::main);
}}
machine choose_amount(flag: bool) -> i32 ensures result >= 7 && result <= 9 {{
    match flag {{ true -> 9, _ -> 7 }}
}}
{helpers}
"#
        );
        for command in ["run", "build"] {
            let (ran, output) = check_project(name, &build, 7, "", &[command, "build.omg"]);
            assert_eq!(ran, accepted, "{output}");
            if !accepted {
                assert!(
                    output.contains("cannot prove requires contract for call require_related"),
                    "{output}"
                );
            }
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
    for command in ["run", "build"] {
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
            &[command, "build.omg"],
        );
        assert!(ran, "{report}");
    }
}

#[test]
fn registration_and_a_mutable_local_survive_the_same_preparation() {
    let after_wait = format!(
        "    $held = 7;\n    builder.tests.disabled = amount == 7;\n    suspend Build::prepare_dependency(builder, kit_registered);\n{}",
        after("|amounts|::value")
    );
    for command in ["run", "build"] {
        let (ran, report) = check_with_dependency_build(
            "registration-and-mutable-local",
            "    let mutable amount: i32 = 3;\n    let held: @mutable i32 = @mutable amount;",
            &after_wait,
            7,
            "",
            &[command, "build.omg"],
        );
        assert!(ran, "{report}");
    }
}

#[test]
fn a_local_helper_can_run_before_the_build_releases_its_live_borrow() {
    for command in ["run", "build"] {
        let (ran, report) = check_with_dependency_build(
            "call-with-live-borrow",
            "    let mutable amount: i32 = 3;\n    let held: @mutable i32 = @mutable amount;",
            "    let seven: i32 = |amounts|::value();\n    $held = seven;\n    suspend Build::prepare_dependency(builder, kit_registered);\n    transition amount == 7 {\n        true -> carried(builder),\n        _ -> wrong(builder)\n    }",
            7,
            "",
            &[command, "build.omg"],
        );
        assert!(ran, "{report}");
    }
}

#[test]
fn a_build_keeps_the_byte_its_helper_read_before_preparation() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = r#"use main.omg as app;
use counts.omg as counts;
machine middle(bytes: @[u8]) -> u8 requires bytes.length == 3 { bytes[1] }
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "staged_byte_read" };
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    let byte: u8 = middle("kit");
    suspend Build::prepare_dependency(builder, registered);
    let seven: i32 = |counts|::value();
    transition byte == 105u8 && seven == 7 {
        true -> done(builder),
        _ -> wrong(builder)
    }
    machine done(builder: @mutable BuildPlan) { Build::bind(builder, HOST::ProgramEntry, |app|::main); }
    machine wrong(builder: @mutable BuildPlan) { builder.product = BuildProduct::Unselected; }
}
"#.replace("HOST", target.target_name());
    for command in ["run", "build"] {
        let (ran, report) =
            check_project("byte-before-wait", &build, 7, "", &[command, "build.omg"]);
        assert!(ran, "{report}");
    }
}

#[test]
fn a_field_borrow_survives_preparing_a_product_dependency() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let source = include_str!("fixtures/build/run/borrowed_field_after_mount/build.omg").replace(
        "linux_x86_64::ProgramEntry",
        &format!("{}::ProgramEntry", target.target_name()),
    );
    for command in ["run", "build"] {
        let (ran, report) =
            check_project("borrowed-field", &source, 7, "", &[command, "build.omg"]);
        assert!(ran, "{report}");

        let conflict = source.replace("$held = seven;", "gauge.amount = seven;");
        let (ran, report) = check_project(
            "borrowed-field-conflict",
            &conflict,
            7,
            "",
            &["--check", "main.omg"],
        );
        assert!(
            !ran && report
                .contains("storage is changed while a conflicting reference remains live")
                && report.contains("build.omg:21:11"),
            "{report}"
        );
    }
}

#[test]
fn a_suspending_helper_returns_its_local_value_after_preparation() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let original = include_str!("fixtures/build/run/borrowed_field_after_mount/build.omg")
        .replace("\r\n", "\n")
        .replace(
            "linux_x86_64::ProgramEntry",
            &format!("{}::ProgramEntry", target.target_name()),
        );
    let wait = "suspend Build::prepare_dependency(builder, registration);";
    let mut build = original.replace(
        &format!("{wait}\n    let seven: i32 = helper_value();"),
        "let seven: i32 = suspend ready(builder, registration);",
    );
    assert_ne!(build, original);
    build.push_str(&format!(
        "\nmachine ready(builder: @mutable BuildPlan, registration: DependencyRegistration) -> i32 suspends {{\n    {wait}\n    helper_value()\n}}\n"
    ));
    for command in ["run", "build"] {
        let (ok, report) = check_project(
            "suspending-helper-result",
            &build,
            7,
            "",
            &[command, "build.omg"],
        );
        assert!(ok, "{report}");
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
}

#[test]
fn a_transition_can_use_the_result_of_a_suspending_build_helper() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = r#"use main.omg as app;
use counts.omg as counts;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "ready_subject" };
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    SUBJECT {
        true -> finish(builder),
        _ -> wrong(builder)
    }
    machine finish(builder: @mutable BuildPlan) { Build::bind(builder, HOST::ProgramEntry, |app|::main); }
    machine wrong(builder: @mutable BuildPlan) { builder.product = BuildProduct::Unselected; }
}
machine ready(builder: @mutable BuildPlan, item: DependencyRegistration) -> bool suspends {
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
        .replace(
            "SUBJECT",
            &format!(r"transition ({call}, |counts|::value())"),
        )
        .replace("true ->", "(_, 7) ->")
        .replace("_ ->", "(_, _) ->");
    for (name, source) in [
        ("bound-subject", &bound),
        ("direct-subject", &direct),
        ("subsequent-subject", &pair),
    ] {
        for command in ["run", "build"] {
            let (ran, report) = check_project(name, source, 7, "", &[command, "build.omg"]);
            assert!(ran, "{report}");
        }
    }
    let (ran, report) = check_project("false-subject", &direct, 8, "", &["--check", "main.omg"]);
    assert!(!ran && report.contains("project role"), "{report}");
}

#[test]
fn a_helper_prepares_a_package_with_the_callers_borrow_still_live() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let original = include_str!("fixtures/build/run/borrowed_field_after_mount/build.omg").replace(
        "linux_x86_64::ProgramEntry",
        &format!("{}::ProgramEntry", target.target_name()),
    );
    let wait = "suspend Build::prepare_dependency(builder, registration);";
    for (name, inside) in [("helper-wait", false), ("helper-calls-local", true)] {
        let mut source = original.replace(wait, "suspend ready(builder, registration);");
        let body = if inside {
            "builder.tests.disabled = helper_value() == 7;"
        } else {
            ""
        };
        source.push_str(&format!("\nmachine ready(builder: @mutable BuildPlan, registration: DependencyRegistration) suspends {{\n    {wait}\n    {body}\n}}\n"));
        for command in ["run", "build"] {
            let (ran, report) = check_project(name, &source, 7, "", &[command, "build.omg"]);
            assert!(ran, "{report}");
        }
    }
}

#[test]
fn a_build_helper_can_use_its_imported_type_on_both_sides_of_a_wait() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let scratch = project::stage(
        "prepared-helper-type",
        &[
            (
                "vendor/build.omg",
                r#"machine build(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::Package { name: "scores" };
    }"#,
            ),
            ("types.omg", "public data Score = i32;"),
            ("vendor/main.omg", "public data Exported = i32;"),
            ("main.omg", "machine main() {}"),
            (
                "support/helpers.omg",
                r#"use types.omg as types;
public machine score() -> i32 {
    let seven: |types|::Score = 7;
    seven
}"#,
            ),
        ],
    );
    let root = scratch.path();
    let build = r#"use main.omg as app;
use support/helpers.omg as support;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "prepared_type" };
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "vendor" });
    |dependency|::with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    suspend Build::prepare_dependency(builder, registered);
    let score: i32 = |support|::score();
    transition score == 7 { true -> finish(builder), _ -> wrong(builder) }
    machine finish(builder: @mutable BuildPlan) { Build::bind(builder, HOST::ProgramEntry, |app|::main); }
    machine wrong(builder: @mutable BuildPlan) { builder.product = BuildProduct::Unselected; }
}"#.replace("HOST", target.target_name());
    for command in ["run", "build"] {
        let run = |program: &str| {
            std::fs::write(root.join("build.omg"), program).unwrap();
            project::observe(root, &[command, "build.omg"])
        };
        let result = run(&build);
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.status.success(), "{report}");
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
        assert!(result.status.success(), "{report}");
    }
}

#[test]
fn each_transition_arm_can_prepare_its_own_dependency() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let scratch = project::stage(
        "alternative-preparations",
        &[("app/main.omg", "machine main() {}")],
    );
    let root = scratch.path().join("app");
    for (directory, name, value) in [("left_vendor", "alpha", 7), ("right_vendor", "beta", 13)] {
        let source = format!("public machine value() -> i32 {{ {value} }}");
        let package = format!(
            "machine build(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Package {{ name: \"{name}\" }}; }}"
        );
        project::write(
            scratch.path(),
            &[
                (&format!("app/{name}.omg"), &source),
                (&format!("{directory}/build.omg"), &package),
                (&format!("{directory}/main.omg"), &source),
            ],
        );
    }
    let build = r#"use main.omg as app;
use alpha.omg as alpha;
use beta.omg as beta;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "alternatives" };
    builder.tests.disabled = FLAG;
    transition builder.tests.disabled {
        true -> suspend left(builder),
        _ -> suspend right(builder)
    }
    machine left(builder: @mutable BuildPlan) suspends {
        let item: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../left_vendor" }));
        suspend Build::prepare_dependency(builder, item);
        let actual: i32 = |alpha|::value();
        transition actual == 7 { true -> done(builder), _ -> wrong(builder) }
    }
    machine right(builder: @mutable BuildPlan) suspends {
        let item: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../right_vendor" }));
        suspend Build::prepare_dependency(builder, item);
        let actual: i32 = |beta|::value();
        transition actual == 13 { true -> done(builder), _ -> wrong(builder) }
    }
    machine done(builder: @mutable BuildPlan) { Build::bind(builder, HOST::ProgramEntry, |app|::main); }
    machine wrong(builder: @mutable BuildPlan) { builder.product = BuildProduct::Unselected; }
}"#.replace("HOST", target.target_name());
    for (flag, unused_error) in [("true", false), ("false", false), ("true", true)] {
        let mut selected = build.replace("FLAG", flag);
        if unused_error {
            selected.push_str("\nmachine unused(n: u64) -> u64 { n + 1u64 }\n");
        }
        std::fs::write(root.join("build.omg"), selected).unwrap();
        for command in ["run", "build"] {
            let output = project::observe(&root, &[command, "build.omg"]);
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
                assert!(output.status.success(), "{report}");
            }
        }
    }
}

#[test]
fn resumption_preserves_arithmetic_and_float_contracts() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = r#"use main.omg as app;
use counts.omg as counts;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "staged_proofs" };
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    let start: i32 = pick(builder.tests.disabled);
    let after: i32 = increment(start);
    let measured: f64 = reading(builder.optimizations.human_report != 0u8);
    suspend Build::prepare_dependency(builder, registered);
    let value: i32 = |counts|::value();
    require_step(start, after);
    require_small(measured);
    Build::bind(builder, HOST::ProgramEntry, |app|::main);
}
machine pick(flag: bool) -> i32 ensures result >= 7 && result <= 9 { match flag { true -> 7, _ -> 9 } }
machine increment(value: i32) -> i32 requires value <= 9 ensures result == value + 1 { value + 1 }
machine require_step(start: i32, after: i32) requires start <= 9 && after == start + 1 {}
machine reading(flag: bool) -> f64 ensures result >= 0.0 && result <= 0.5 { match flag { true -> 0.25, _ -> 0.5 } }
machine require_small(x: f64) requires x >= 0.0 && x <= 0.5 {}
"#.replace("HOST", target.target_name());
    for command in ["run", "build"] {
        let run = |name, source: &str| check_project(name, source, 7, "", &[command, "build.omg"]);
        let (ran, report) = run("staged-proof", &build);
        assert!(ran, "{report}");
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
}

#[test]
fn only_the_selected_transition_arm_waits_for_its_dependency() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = r#"use main.omg as app;
use counts.omg as counts;
const CHOSEN: bool = FLAG;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "conditional" };
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    builder.tests.disabled = CHOSEN;
    transition builder.tests.disabled {
        true -> suspend ready(builder, registered),
        _ -> finish(builder)
    }
    machine ready(builder: @mutable BuildPlan, registered: DependencyRegistration) suspends {
        suspend Build::prepare_dependency(builder, registered);
        let value: i32 = |counts|::value();
        transition value == 7 { true -> finish(builder), _ -> wrong(builder) }
    }
    machine finish(builder: @mutable BuildPlan) { Build::bind(builder, HOST::ProgramEntry, |app|::main); }
    machine wrong(builder: @mutable BuildPlan) { builder.product = BuildProduct::Unselected; }
}"#.replace("HOST", target.target_name());
    for (flag, value) in [("true", 7), ("false", 8)] {
        for command in ["run", "build"] {
            let (ran, report) = check_project(
                &format!("conditional-wait-{flag}"),
                &build.replace("FLAG", flag),
                value,
                "",
                &[command, "build.omg"],
            );
            assert!(ran, "{report}");
        }
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
    let before = r#"    let input: BuildPath = BuildSource::resolve(@builder.source, "support/support.omg");
    let reader: i32 = BuildSource::open(@builder.source, input, 0);
    let mutable first: [u8; 1] = [0u8];
    let first_count: i64 = BuildSource::read(@builder.source, reader, @mutable first[..], 1u64);
    let saw_first: bool = first_count == 1i64 && first[0] == 112u8;
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
    let ready: bool = selected == 7 && saw_first && second_count == 1i64 && second[0] == 117u8 && denied == -1;
    let completion: OutputCompletion = BuildOutput::complete(@mutable builder.output, promised, output);
    transition completion {
        OutputCompletion::Sealed { receipt } -> checked(builder, ready),
        OutputCompletion::Retry { obligation, file } -> failed(builder, obligation)
    }
    machine failed(builder: @mutable BuildPlan, obligation: RequiredOutput) {
        BuildOutput::fail(@mutable builder.output, obligation, "staged bytes did not settle");
    }
    machine checked(builder: @mutable BuildPlan, ready: bool) {
        transition ready {
            true -> carried(builder), _ -> wrong(builder)
        }
    }"#;
    for command in ["run", "build"] {
        let (ran, report) = check_with_dependency_build(
            "open-files-across-mount",
            before,
            after,
            7,
            "",
            &[command, "build.omg"],
        );
        assert!(ran, "{report}");
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

        let forged = after.replace("promised, output);", "promised, BuildPath {});");
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
}

#[test]
fn an_issued_output_keeps_its_borrowed_path_and_obligation_across_preparation() {
    let target = target::TargetProfile::host_if_supported().expect("hosted test target");
    let build = include_str!("fixtures/build/run/required_output_after_mount/build.omg").replace(
        "linux_x86_64::ProgramEntry",
        &format!("{}::ProgramEntry", target.target_name()),
    );
    for command in ["run", "build"] {
        let (ran, report) = check_project(
            "issued-output-after-wait",
            &build,
            7,
            "",
            &[command, "build.omg"],
        );
        assert!(ran, "{report}");

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
                && report.contains("build.omg:19:")
                && !report.contains("ExecutionSite")
                && !report.contains("EffectRejected"),
            "{report}"
        );
    }
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
            && report.contains("build.omg:16:14")
            && !report.contains("while resuming the Build after dependency preparation"),
        "{report}"
    );
}

#[test]
fn a_user_record_named_dependency_registration_keeps_its_own_fields() {
    let target = target::TargetProfile::host_if_supported().expect("a hosted test target");
    let target = target.target_name();
    let build = format!(
        r#"use main.omg as app;
use counts.omg as counts;
use support/support.omg as issued;
data DependencyRegistration [copy] {{ mark: u32; }}
use {{omega}}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {{
    builder.product = BuildProduct::Application {{ name: "staged" }};
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path {{ location: "dep" }});
    |dependency|::with_alias(@mutable request, "kit");
    let shadow: DependencyRegistration = DependencyRegistration {{ mark: 7u32 }};
    let real: |issued|::IssuedRegistration = Build::dependency(builder, request);
    suspend Build::prepare_dependency(builder, real);
    builder.tests.disabled = shadow.mark == 7u32;
    let seven: i32 = |counts|::value();
    transition seven == 7 && builder.tests.disabled {{
        true -> carried(builder), _ -> wrong(builder)
    }}
    machine carried(builder: @mutable BuildPlan) {{ Build::bind(builder, {target}::ProgramEntry, |app|::main); }}
    machine wrong(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Unselected; }}
}}
"#
    );
    let (ran, report) = check_project_with_support(
        "same-spelled-user-record",
        &build,
        7,
        "",
        Some("public data IssuedRegistration = DependencyRegistration;"),
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
        "    builder.product = BuildProduct::Application { name: \"second_role\" };
    let seven: i32 = 1;",
        &after("|amounts|::value"),
        7,
    );
    assert!(!compiled, "{stderr}");
    assert!(
        stderr.contains("an earlier local with this name is still visible")
            && stderr.contains("build.omg:15:9"),
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
    assert!(
        !report.contains("while preparing the dependency"),
        "{report}"
    );
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
        "    let mutable lib_request: DependencyDescription = |dependency|::from_source(Source::Path { location: \"dep\" });
    |dependency|::with_alias(@mutable lib_request, \"lib\");
    let lib_registered: DependencyRegistration = Build::dependency(builder, lib_request);",
        &after("|lib|::v"),
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
        stderr.contains("while reading the signature of `build`"),
        "{stderr}"
    );
    let location = stderr.lines().map(str::trim).find_map(|line| {
        line.strip_prefix("at ")
            .and_then(|position| position.strip_suffix(":15:22"))
    });
    // `project::stage` names the directory `omega-<name>-<pid>-<sequence>`.
    let staged = format!("omega-product-dependency-{}-", std::process::id());
    let in_staged_project = |path: &str| {
        let path = std::path::Path::new(path);
        path.file_name() == Some("build.omg".as_ref())
            && path
                .parent()
                .and_then(std::path::Path::file_name)
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|directory| {
                    directory
                        .strip_prefix(&staged)
                        .is_some_and(|sequence| sequence.parse::<u64>().is_ok())
                })
    };
    assert!(
        location.is_some_and(in_staged_project),
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
    let prefix = "machine build(builder: @mutable BuildPlan) {\n    builder.product = BuildProduct::Package { name: \"kit_package\" };\n    ";
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
    let scratch = project::stage(
        "prepared-product-scope",
        &[
            (
                "vendor/build.omg",
                r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "kit" }; }"#,
            ),
            ("vendor/main.omg", "public machine value() -> i32 { 7 }"),
            (
                "main.omg",
                r#"use {kit}/main.omg as kit;
machine main() crashes Trap {
    transition |kit|::value() == 7 {
        true -> pass(),
        _ -> wrong()
    }
}
machine pass() {}
machine wrong() crashes Trap { crash Trap; }
"#,
            ),
        ],
    );
    let root = scratch.path();
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = r#"use main.omg as app;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "prepared_product" };
    let registered: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "vendor" }));
    suspend Build::prepare_dependency(builder, registered);
    Build::bind(builder, HOST_TARGET::ProgramEntry, |app|::main);
}
"#.replace("HOST_TARGET", host.target_name());
    for command in ["run", "build"] {
        let invoke = |contents: &str| {
            std::fs::write(root.join("build.omg"), contents).unwrap();
            project::observe(root, &[command, "build.omg"])
        };
        let output = invoke(&build);
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{report}");

        let leaked = format!(
            "use {{kit}}/main.omg as kit;\n{}",
            build.replace(
                "    Build::bind",
                "    let leaked: i32 = |kit|::value();\n    Build::bind"
            )
        );
        let output = invoke(&leaked);
        let report = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success()
                && report.contains("Build import `{kit}/main.omg as kit` has no supplied source")
                && report.contains("build.omg:1:5"),
            "{report}"
        );
        let unknown = leaked
            .replace(
                "use {kit}/main.omg as kit;",
                "use {kitchen}/main.omg as kitchen;",
            )
            .replace("|kit|::value()", "|kitchen|::value()");
        let output = invoke(&unknown);
        let report = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success()
                && report.contains(
                    "Build import `{kitchen}/main.omg as kitchen` has no supplied source"
                )
                && report.contains("build.omg:1:5"),
            "{report}"
        );
    }
}

#[test]
fn reflection_can_describe_a_product_file_without_a_host_import() {
    let build = r#"use {kit}/main.omg as api;
use {omega}/language/core/reflect/reflect.omg as describe;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "query" };
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable request, "kit");
    _ = Build::dependency(builder, request);
    let field: |describe|::FieldInfo = |describe|::field(|api|::Number::value);
    transition field.name == "value" && field.declaration_position == 0u64 {
        true -> done(), false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}"#;
    let args = [
        "--check",
        "--build-input",
        "build.omg",
        "--build-input",
        "main.omg",
        "build.omg",
    ];
    let (ok, report) = check_project("described-product-file", build, 7, "", &args);
    assert!(ok, "{report}");
    let host = build.replace("    let field:", "    _ = |api|::v();\n    let field:");
    let (ok, report) = check_project("described-file-is-not-host-code", &host, 7, "", &args);
    assert!(
        !ok && report.contains("Build import `{kit}/main.omg as api` has no supplied source"),
        "{report}"
    );
}

#[test]
fn an_unused_unbound_import_warns_once_at_the_authored_name() {
    let build = "use {absent}/helper.omg as missing;\nuse later.omg;\nuse main.omg as present;\nuse main.omg as entry;\nmachine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Application { name: \"never_used\" }; Build::bind(builder, linux_x86_64::ProgramEntry, |entry|::main); }\n";
    let scratch = project::stage(
        "unused-file-import",
        &[("main.omg", "machine main() {}"), ("build.omg", build)],
    );
    let output = project::omega(
        scratch.path(),
        &["--check", "--target", "linux_x86_64", "build.omg"],
    );
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success()
            && report.matches("unused file import `missing`").count() == 1
            && report.matches("unused file import `later`").count() == 1
            && report.matches("unused file import `present`").count() == 1
            && !report.contains("unused file import `entry`")
            && report.contains("build.omg:1:5"),
        "{report}"
    );
}

#[test]
fn a_wait_cannot_add_a_library_to_the_checked_build() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = r#"use main.omg as app;
use {kit}/main.omg as chosen;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "fixed_imports" };
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::dependency(builder, request);
    suspend Build::prepare_dependency(builder, registered);
    let value: i32 = |chosen|::v();
    Build::bind(builder, HOST::ProgramEntry, |app|::main);
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
                && report.contains("build.omg:2:5"),
            "{report}"
        );
    }
}

#[test]
fn a_build_binds_its_products_file_without_being_able_to_call_it() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let scratch = project::stage(
        "product-file-operand",
        &[
            (
                "vendor/build.omg",
                r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "kit" }; }"#,
            ),
            ("vendor/entry.omg", "public machine start() {}"),
            (
                "main.omg",
                "machine unreachable() crashes Trap { crash Trap; }",
            ),
        ],
    );
    let root = scratch.path();
    let build = r#"use {kit}/entry.omg as selected;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "product_file" };
    let registered: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "vendor" }));
    suspend Build::prepare_dependency(builder, registered);
    Build::bind(builder, HOST::ProgramEntry, |selected|::start);
}"#.replace("HOST", host.target_name());
    let invoke = |source: &str, args: &[&str]| {
        std::fs::write(root.join("build.omg"), source).unwrap();
        let output = project::observe(root, args);
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.success(), report)
    };
    for command in ["run", "build"] {
        let (ok, report) = invoke(&build, &[command, "build.omg"]);
        assert!(ok, "{report}");
        let with_unused = format!("use {{absent}}/other.omg as unused;\n{build}");
        let (ok, report) = invoke(&with_unused, &["--check", "build.omg"]);
        assert!(
            ok && report.matches("unused file import `unused`").count() == 1
                && !report
                    .contains("Build import `{kit}/entry.omg as selected` has no supplied source"),
            "{report}"
        );
        let called = build.replace(
            "    Build::bind",
            "    |selected|::start();\n    Build::bind",
        );
        let (ok, report) = invoke(&called, &["--check", "build.omg"]);
        assert!(
            !ok && report
                .contains("Build import `{kit}/entry.omg as selected` has no supplied source")
                && report.contains("build.omg:1:5"),
            "{report}"
        );
    }
}

#[test]
fn unused_private_declarations_warn_once_and_preserve_used_roots() {
    let build = r#"use main.omg as app;

machine used_build_helper() -> u32 { 1u32 }
machine dead_build_helper() -> u32 { 2u32 }

machine build(builder: @mutable BuildPlan) {
    let _: u32 = used_build_helper();
    builder.product = BuildProduct::Application { name: "unused_decl_check" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}
"#;
    let main = r#"const USED_CONST: u32 = 1u32;
const DEAD_CONST: u32 = 2u32;

data UsedData [copy] { value: u32; }
data DeadData [copy] { value: u32; }

domain u32::UsedDomain requires self > 0u32;
domain u32::DeadDomain requires self > 0u32;

machine used_generic<T>(x: T) -> T { x }
machine dead_generic<T>(x: T) -> T { x }

machine used_helper(x: u32 in UsedDomain) -> u32 { x as -UsedDomain }
machine dead_helper() -> u32 { 7u32 }

machine island_a() -> u32 { island_b() }
machine island_b() -> u32 { 3u32 }

machine contract_only(x: u32) -> bool { x > 0u32 }
machine guarded(x: u32) -> u32 requires contract_only(x) { x }

machine helper_for_public() -> u32 { 9u32 }
public machine exported_api() -> u32 { helper_for_public() }

machine main() {
    let item: UsedData = UsedData { value: USED_CONST };
    let val: u32 = used_generic::<u32>(item.value);
    let first: u32 = used_helper(USED_CONST as +UsedDomain);
    let second: u32 = guarded(USED_CONST);
    let _: u32 = val;
}
"#;
    let scratch = project::stage(
        "unused-private-declarations",
        &[("main.omg", main), ("build.omg", build)],
    );
    let output = project::omega(
        scratch.path(),
        &["--check", "--target", "linux_x86_64", "build.omg"],
    );
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success()
            && report.matches("unused machine `dead_build_helper`").count() == 1
            && report.matches("unused const `DEAD_CONST`").count() == 1
            && report.matches("unused data `DeadData`").count() == 1
            && report.matches("unused domain `u32::DeadDomain`").count() == 1
            && report.matches("unused machine `dead_generic`").count() == 1
            && report.matches("unused machine `dead_helper`").count() == 1
            && report.matches("unused machine `island_a`").count() == 1
            && report.matches("unused machine `island_b`").count() == 1
            && !report.contains("used_build_helper")
            && !report.contains("USED_CONST")
            && !report.contains("UsedData")
            && !report.contains("UsedDomain")
            && !report.contains("used_generic")
            && !report.contains("used_helper")
            && !report.contains("contract_only")
            && !report.contains("guarded")
            && !report.contains("helper_for_public")
            && !report.contains("exported_api")
            && !report.contains("`main`")
            && !report.contains("`build`"),
        "{report}"
    );
}

#[test]
fn unused_private_declaration_in_dependency_does_not_warn() {
    let dep_build = r#"machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "dep" };
}
"#;
    let dep_lib = r#"machine dep_dead() -> u32 { 42u32 }
public machine dep_used() -> u32 { 7u32 }
"#;
    let app_build = r#"use main.omg as app;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "consumer" };
    let mutable req: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable req, "dep");
    let _: DependencyRegistration = Build::dependency(builder, req);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}
"#;
    let app_main = r#"use {dep}/lib.omg as dep;
machine main() {
    let _: u32 = |dep|::dep_used();
}
"#;
    let scratch = project::stage(
        "unused-decl-dep-isolation",
        &[
            ("dep/build.omg", dep_build),
            ("dep/lib.omg", dep_lib),
            ("build.omg", app_build),
            ("main.omg", app_main),
        ],
    );
    let output = project::omega(
        scratch.path(),
        &["--check", "--target", "linux_x86_64", "build.omg"],
    );
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && !report.contains("dep_dead") && !report.contains("unused "),
        "{report}"
    );
}
