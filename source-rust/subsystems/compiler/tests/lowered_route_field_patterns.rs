//! Payload reads require the selected tag, whether set by a pattern or by case tests.

#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use source_compiler::checking::{RouteVerdict, judge};
use source_compiler::lowering::{Compilation, elaborate};

static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn verdict(name: &str, source: &str) -> RouteVerdict {
    let _serial = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::staged_frontier(name, source);
    judge_frontier(name, &frontier)
}

fn judge_frontier(name: &str, frontier: &compiler::TypedFrontier) -> RouteVerdict {
    let lowered = elaborate(
        &frontier.typed,
        Compilation {
            calling: frontier
                .normalization
                .calling
                .borrow(&frontier.typed)
                .selection(),
            providers: frontier.provider_selections(),
            declaration_metadata: &[],
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: declaration_contracts::source_identity::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
            supplies: &[],
        },
    );
    judge(lowered).unwrap_or_else(|defect| panic!("{name}: route defect: {defect:?}"))
}

#[test]
fn a_payload_read_requires_all_other_cases_to_be_excluded() {
    let positive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/fixtures/run/contracts/case_refutation/main.omg"
    ));
    let negative = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/fixtures/fail/contracts/case_refutation/main.omg"
    ));
    let expected = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/fixtures/fail/contracts/case_refutation/expected.txt"
    ));

    let found = verdict("case_refutation", positive);
    assert!(matches!(found, RouteVerdict::Accepted(_)), "{found:?}");
    let found = verdict("one_case_still_possible", negative);
    let RouteVerdict::Rejected { unestablished, .. } = found else {
        panic!("{found:?}");
    };
    assert!(
        unestablished
            .iter()
            .any(|failure| failure.detail == expected.trim()),
        "{unestablished:?}"
    );
}

#[test]
fn a_case_field_value_keeps_its_case_fallback() {
    let found = verdict(
        "case_field_value",
        r#"
        data Token [copy] { case Keyword(keyword: u8, offset: u32); case Other; }
        machine classify(token: @Token) -> u32 {
            match token {
                Token::Keyword { keyword: 7, offset as _ } -> 1
                Token::Keyword { keyword as _, offset as _ } -> 2
                _ -> 3
            }
        }
        machine main() {}
    "#,
    );
    assert!(matches!(found, RouteVerdict::Accepted(_)), "{found:?}");
}

const TOKEN: &str = "data Token [copy] { case Keyword(keyword: u8, offset: u32); case Other; }";

#[test]
fn ordered_case_tests_and_partial_wildcards_are_accepted() {
    for (name, body) in [
        (
            "repeated_case",
            "match token { Token::Keyword {keyword:7, offset as _}->1 Token::Keyword {keyword:8, offset as _}->2 Token::Keyword {keyword as _, offset as _}->3 Token::Other->4 }",
        ),
        (
            "transition_fields",
            "transition token { Token::Keyword {keyword:7, offset}->offset Token::Keyword {keyword:8, offset as _}->2 Token::Keyword {keyword as _, offset as _}->3 _->4 }",
        ),
        (
            "partial_wildcard",
            "match token { Token::Keyword {keyword:7, offset as _}->1 Token::Other->2 _->3 }",
        ),
    ] {
        let found = verdict(
            name,
            &format!("{TOKEN} machine pick(token:@Token)->u32{{{body}}} machine main(){{}}"),
        );
        let RouteVerdict::Accepted(closure) = found else {
            panic!("{name}: {found:?}")
        };
        assert!(
            closure
                .advisories()
                .iter()
                .all(|message| !message.message.contains("the `_` arm")),
            "{name}: {:?}",
            closure.advisories()
        );
    }
}

#[test]
fn an_unconditional_pattern_shadows_later_arms_with_an_advisory() {
    for (name, body) in [
        (
            "full_case_first",
            "match token { Token::Keyword {keyword as _, offset as _}->1 Token::Keyword {keyword:7, offset as _}->2 Token::Other->3 }",
        ),
        (
            "wildcard_first",
            "match token { _->1 Token::Keyword {keyword:7, offset as _}->2 Token::Other->3 }",
        ),
        (
            "transition_shadow",
            "transition token { Token::Keyword {keyword:7, offset as _}->1 _->2 Token::Keyword {keyword:8, offset as _}->3 }",
        ),
    ] {
        let found = verdict(
            name,
            &format!("{TOKEN} machine pick(token:@Token)->u32{{{body}}} machine main(){{}}"),
        );
        let RouteVerdict::Accepted(closure) = found else {
            panic!("{name}: {found:?}")
        };
        let notes = closure.advisories();
        assert!(
            notes.iter().any(
                |message| message.message.contains("earlier unconditional pattern")
                    && message.source_span.is_some()
            ),
            "{name}: {notes:?}"
        );
        assert!(
            notes.iter().any(
                |message| message.message.contains("earlier pattern already matches")
                    && message.source_span.is_some()
            ),
            "{name}: {notes:?}"
        );
    }
}

#[test]
fn tested_payload_fields_do_not_establish_complete_case_coverage() {
    let found = verdict(
        "missing_field_fallback",
        &format!(
            r#"{TOKEN}
        machine pick(token:@Token)->u32 {{ match token {{
            Token::Keyword {{keyword:7, offset as _}}->1
            Token::Other->2
        }} }} machine main() {{}}
    "#
        ),
    );
    assert!(!matches!(found, RouteVerdict::Accepted(_)), "{found:?}");
    assert!(format!("{found:?}").contains("uncovered"), "{found:?}");
}

#[test]
fn a_pattern_value_must_have_the_projected_fields_type() {
    let _serial = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = format!(
        r#"{TOKEN}
        machine pick(token:@Token)->u32 {{ match token {{
            Token::Keyword {{keyword:true, offset as _}}->1 _->2
        }} }} machine main() {{}}
    "#
    );
    let detail = match unbuilt_frontier::staged("field_type_mismatch", &source) {
        Err(errors) => errors.join("\n"),
        Ok(frontier) => {
            let found = judge_frontier("field_type_mismatch", &frontier);
            assert!(!matches!(found, RouteVerdict::Accepted(_)), "{found:?}");
            format!("{found:?}")
        }
    };
    assert!(
        detail.contains("bool") || detail.contains("equality") || detail.contains("operand"),
        "{detail}"
    );
}

#[test]
fn a_projected_comparison_selects_declared_equality() {
    let _serial = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = r#"
        data Key [copy] { case A; case B; }
        machine == pattern_key_equal(left:Key, right:Key)->bool { false }
        data Token [copy] { case Keyword(keyword:Key); case Other; }
        machine pick(token:@Token)->u32 { match token {
            Token::Keyword {keyword:Key::A}->1 _->2
        } }
        machine main() {}
    "#;
    let frontier = unbuilt_frontier::staged("declared_pattern_equality", source)
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    let operator = unbuilt_frontier::program_machine(&frontier.typed, "pattern_key_equal").unwrap();
    let selected = frontier.typed.expression_table.iter_expressions().any(|(_, node)|
        matches!(node, source_compiler::source_state::expression::ExpressionNode::Call(call) if call.target_symbol==operator));
    assert!(
        selected,
        "projected equality must select the declared operator: {:?}",
        frontier
            .typed
            .expression_table
            .iter_expressions()
            .filter(|(_, node)| matches!(
                node,
                source_compiler::source_state::expression::ExpressionNode::Binary(_)
                    | source_compiler::source_state::expression::ExpressionNode::Member(_)
                    | source_compiler::source_state::expression::ExpressionNode::SavedSubject(_)
            ))
            .collect::<Vec<_>>()
    );
    let found = judge_frontier("declared_pattern_equality", &frontier);
    assert!(matches!(found, RouteVerdict::Accepted(_)), "{found:?}");
}

#[test]
fn guarded_cases_emit_the_wildcard_call_once() {
    let found = verdict(
        "one_shared_fallback",
        r#"
        data Choice [copy] { case A(value:u8); case B(value:u8); case C; }
        machine pattern_default(value:u32)->u32 { value }
        machine prepare(choice:Choice)->Choice { choice }
        machine pick(choice:Choice)->u32 { match prepare(choice) {
            Choice::A {value:1}->1 Choice::B {value:2}->2 _->pattern_default(91)
        } }
        machine main() {}
    "#,
    );
    let RouteVerdict::Accepted(closure) = found else {
        panic!("{found:?}")
    };
    let named = |name: &str| {
        closure
            .elaboration()
            .names
            .iter()
            .find_map(|(id, names)| (names.machine.rsplit("::").next() == Some(name)).then_some(id))
            .unwrap()
    };
    let fallback = named("pattern_default");
    let pick = closure.elaboration().module.machine(named("pick")).unwrap();
    let calls = |callee| {
        pick.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                terminal_psi::direct_call_parts(&operation.kind)
                    .is_some_and(|call| call.callee == callee)
            })
            .count()
    };
    assert_eq!(
        calls(fallback),
        1,
        "one authored wildcard has one emitted body"
    );
    assert_eq!(
        calls(named("prepare")),
        1,
        "the completed subject is evaluated once"
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/joined_product_execution.rs"]
mod joined_product_execution;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn native_and_psi_observe_ordered_field_tests_and_fallback_effects() {
    let _serial = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = fixture_package_inputs::repo_root()
        .join("tests/fixtures/run/control/case_field_value_decisions/main.omg");
    let discovered = fixture_package_inputs::discover_fixture(&root, Default::default())
        .unwrap_or_else(|errors| panic!("pattern fixture discovery: {errors:#?}"));
    let product = compiler::CompilationProduct::Native(products::NativeProductRequest {
        installation_assumptions: Vec::new(),
        retain_native_inputs: false,
        retain_native_stage_inputs: false,
        image: Default::default(),
        private_entry_ram: Vec::new(),
        terminal_authority_policy:
            provider_admission::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut prepared = fixture_package_inputs::review_fixture(
        discovered,
        &[target::TargetProfile::LinuxX64],
        product,
    )
    .unwrap_or_else(|errors| panic!("pattern fixture review: {errors:#?}"));
    let output = fixture_package_inputs::produce_fixture(&mut prepared, Some("linux_x86_64"))
        .unwrap_or_else(|errors| panic!("pattern fixture production: {errors:#?}"));
    let report = orchestration::complete_product(output)
        .and_then(compiler::CompilationOutput::into_report)
        .unwrap_or_else(|error| panic!("pattern native completion: {error:?}"));
    let output = platform_custody::temporary_directory::TemporaryDirectory::create(
        "field-pattern-runtime",
        false,
    )
    .unwrap();
    joined_product_execution::execute_report(report, &output.path().join("native"));
}

#[test]
fn an_enclosing_case_remains_selected_after_a_guard_updates_its_payload() {
    let source = r#"
        data Outer { case Hold(value:u32); case Empty; }
        data Token { case Keyword(keyword:u8); case Other; }
        machine touch(state:@mut u32)->u8 { state=9; 0 }
        machine pick(outer:@mut Outer, token:@Token)->u32 { match outer {
            Outer::Hold {value}->match token {
                Token::Keyword {keyword:touch(@mut outer.value)}->99 _->value
            }
            Outer::Empty->0
        } }
        machine main() {}
    "#;
    let found = verdict("nested_guard_case", source);
    assert!(matches!(found, RouteVerdict::Accepted(_)), "{found:?}");
    let replacement = source
        .replace("state:@mut u32", "state:@mut Outer")
        .replace("state=9", "state=Outer::Empty")
        .replace("touch(@mut outer.value)", "touch(@mut outer)");
    let found = verdict("nested_guard_replaces_case", &replacement);
    let RouteVerdict::Rejected { unestablished, .. } = found else {
        panic!("{found:?}")
    };
    assert!(
        unestablished.iter().any(|failure| failure
            .detail
            .contains("cannot prove selected case for payload access")),
        "{unestablished:?}"
    );
}
