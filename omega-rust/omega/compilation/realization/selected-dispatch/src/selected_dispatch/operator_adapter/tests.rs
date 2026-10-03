//! The Terminal-custody admission gate's covered and refused shapes. A
//! crash-qualified selected comparison — spelled or named — whose checked
//! crash site a lowered operation can carry enters production; every shape
//! the stage-05 carrier cannot reconstruct keeps the refusal.

use super::{CheckedTrees, validate_selected_operator_terminal_custody};

fn checked(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize crash-custody fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse crash-custody fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .expect("resolve crash-custody fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type crash-custody fixture");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check crash-custody fixture")
}

/// The guarded integer comparison shape the `crash_routes` corpus fixture
/// carries: an authored `crashes` route on a selected `==`, covered by the
/// calling machine's own `crashes` declaration.
const GUARDED_INTEGER_COMPARISON_SOURCE: &str = r#"
    boundary operator == Comparison::equal(left: i32, right: i32) -> bool
        crashes Trap !(right >= 0);
    data Comparison { }
    machine compare(left: i32, right: i32) -> bool crashes Trap { left == right }
"#;

/// A named `Namespace::requirement(...)` call's contract rides its
/// `BoundaryCall` declaration rather than an operation crash-contract row.
const NAMED_CRASH_CALL_SOURCE: &str = r#"
    boundary operator == Meaning::equal(left: u16, right: u16) -> bool crashes Trap;
    machine choose(left: u16, right: u16) -> bool { Meaning::equal(left, right) }
"#;

#[test]
fn a_scalar_comparison_crash_use_admits() {
    let checked = checked(GUARDED_INTEGER_COMPARISON_SOURCE);
    let uses = checked
        .facts
        .operators
        .crash_qualified_uses(&checked.typed)
        .collect::<Vec<_>>();
    let [checked_trees::CheckedCrashQualifiedUse::Spelled(_)] = uses.as_slice() else {
        panic!("the fixture's selected == is its one spelled crash-qualified use");
    };
    validate_selected_operator_terminal_custody(&checked)
        .expect("a scalar comparison site has an emitted operation carrier");
}

#[test]
fn a_named_crash_call_admits() {
    let checked = checked(NAMED_CRASH_CALL_SOURCE);
    let uses = checked
        .facts
        .operators
        .crash_qualified_uses(&checked.typed)
        .collect::<Vec<_>>();
    let [checked_trees::CheckedCrashQualifiedUse::Named(_)] = uses.as_slice() else {
        panic!("the fixture's named call is its one crash-qualified use");
    };
    validate_selected_operator_terminal_custody(&checked)
        .expect("a named crash call has its boundary-call carrier");
}

#[test]
fn a_non_scalar_signature_keeps_the_refusal() {
    // A spelled `==` on a structural type never reaches checking, so the
    // signature refusal is exercised through the named call's boundary
    // requirement, which checking admits on any parameter roster.
    let checked = checked(
        "data Box { value: u8; }
         boundary operator == Meaning::equal(left: Box, right: Box) -> bool crashes Trap;
         machine choose(left: Box, right: Box) -> bool { Meaning::equal(left, right) }",
    );
    let diagnostics = validate_selected_operator_terminal_custody(&checked)
        .expect_err("no Terminal scalar telescope carries a structural operator's routes");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not all-scalar")),
        "{diagnostics:?}"
    );
}

#[test]
fn a_non_comparison_spelling_keeps_the_refusal() {
    let checked = checked(
        "boundary operator + Meaning::add(left: i32, right: i32) -> i32 crashes Trap;
         machine choose(left: i32, right: i32) -> i32 crashes Trap { left + right }",
    );
    let diagnostics = validate_selected_operator_terminal_custody(&checked)
        .expect_err("no emitted operation carries an arithmetic operator's routes");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not a selected comparison")),
        "{diagnostics:?}"
    );
}

#[test]
fn a_dropped_site_keeps_the_refusal() {
    let mut checked = checked(GUARDED_INTEGER_COMPARISON_SOURCE);
    let compare = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "compare")
        .expect("compare machine")
        .symbol;
    let plan = checked
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == compare)
        .expect("compare contract plan");
    plan.crash = plan
        .crash
        .clone()
        .with_checked_operators(Vec::new())
        .expect("an empty site roster");
    let diagnostics = validate_selected_operator_terminal_custody(&checked)
        .expect_err("a use with no checked crash site cannot enter production");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("no checked crash site")),
        "{diagnostics:?}"
    );
}
