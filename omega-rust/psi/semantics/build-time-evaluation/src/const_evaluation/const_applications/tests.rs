//! Provider-dependent const-generic applications: the deferral gate, the
//! pending marks, and the selected-execution fold over generic argument
//! positions (fields, `let` annotations, return types).

use super::{
    const_application_plan, defer_pending_const_applications, evaluate_selected_const_applications,
    pending_const_applications_need_operator_selection,
};
use crate::SelectedBuildTimeOperators;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

/// A `Buffer<limit()>` data field whose `limit` callee computes `left | right`
/// = 7 through a selected boundary-operator provider where builtin `%` folds
/// 1, so the folded argument is the positive witness that the provider machine
/// -- not host arithmetic -- ran.
fn provider_application_fixture() -> (TypedTrees, Vec<crate::SelectedBuildTimeProviderBody>) {
    let program = typed(
        "data Math {}
         boundary operator % Math::remainder(left: u64, right: u64) -> u64;
         data Provider {}
         machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left | right }
         machine limit() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (left % right) } }
         data Buffer<const N: u64> { values: [u8; N]; }
         data Main { value: Buffer<limit()>; }",
    );
    let rows = provider_rows(&program);
    (program, rows)
}

/// The fixture's selected provider-body rows: every resolved boundary
/// operator use binds `Provider::remainder`'s exact entry under a fixed test
/// plan commitment.
fn provider_rows(program: &TypedTrees) -> Vec<crate::SelectedBuildTimeProviderBody> {
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(program);
    facts
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
        .filter(|fact| {
            program.operators().iter().any(|operator| {
                operator.symbol == fact.selected_operator_symbol && operator.is_boundary
            })
        })
        .map(|fact| {
            let provider = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Provider::remainder")
                .unwrap();
            let entry = program.machine_states(provider).first().unwrap();
            crate::SelectedBuildTimeProviderBody {
                expression: fact.expression,
                origin: fact.origin,
                requirement: fact.selected_operator_symbol,
                operands: fact.operands(program).unwrap(),
                provider_machine: provider.symbol,
                provider_state: entry.symbol,
                provider_type: provider.attached_data.as_ref().unwrap().as_str().to_owned(),
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]),
            }
        })
        .collect()
}

/// The folded literal spellings of every `Named` const argument under the
/// fixture's `Buffer` generic applications, in scan order.
fn folded_const_arguments(program: &TypedTrees) -> Vec<String> {
    let mut folded = Vec::new();
    for (_, _, arguments) in program.type_reference_table.generic_type_reference_sites() {
        for argument in program
            .type_reference_table
            .type_reference_handles(arguments)
        {
            if let TypeReferenceNode::Named { name, .. } =
                program.type_reference_table.type_reference(*argument)
            {
                if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    folded.push(name.as_str().to_owned());
                }
            }
        }
    }
    folded
}

#[test]
fn provider_boundary_const_application_waits_for_selected_execution() {
    let (program, rows) = provider_application_fixture();
    assert_eq!(rows.len(), 1);
    assert!(
        pending_const_applications_need_operator_selection(&program, None).unwrap(),
        "a boundary-operator callee must defer its const application until selected rows exist"
    );
    let independent = typed(
        "machine limit() -> u64 { 7 }
         data Buffer<const N: u64> { values: [u8; N]; }
         data Main { value: Buffer<limit()>; }",
    );
    assert!(
        !pending_const_applications_need_operator_selection(&independent, None).unwrap(),
        "an independent const application keeps its early route"
    );
}

#[test]
fn pending_const_application_marks_keep_the_consuming_tuple_open() {
    let (mut program, _) = provider_application_fixture();
    let plan = const_application_plan(&program);
    assert_eq!(plan.applications.len(), 1);
    let root = plan.applications[0].root;
    defer_pending_const_applications(&mut program).unwrap();
    assert!(program.pending_const_range_endpoints.contains(&root));
    for call in &plan.calls {
        assert!(
            program
                .pending_const_range_endpoints
                .contains(&call.expression)
        );
    }
}

#[test]
fn pending_const_application_executes_the_selected_provider_body() {
    let (mut program, rows) = provider_application_fixture();
    crate::validate_selected_provider_bodies(&program, &rows).unwrap();
    evaluate_selected_const_applications(
        &mut program,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap_or_else(|errors| panic!("provider-boundary const application: {errors:?}"));
    assert_eq!(
        folded_const_arguments(&program).as_slice(),
        &["7".to_owned()],
        "the provider's `left | right` body must run; builtin `%` would fold 1"
    );
    assert!(const_application_plan(&program).applications.is_empty());
    assert!(program.pending_const_range_endpoints.is_empty());
}

#[test]
fn pending_const_application_folds_let_and_return_destinations() {
    let program = typed(
        "data Math {}
         boundary operator % Math::remainder(left: u64, right: u64) -> u64;
         data Provider {}
         machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left | right }
         machine limit() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (left % right) } }
         data Buffer<const N: u64> { values: [u8; N]; }
         machine make() -> Buffer<limit()> { Buffer { values: [0, 0, 0, 0, 0, 0, 0] } }
         machine keep(&mut self) { let folded: Buffer<limit()> = make(); }",
    );
    let plan = const_application_plan(&program);
    assert_eq!(
        plan.applications.len(),
        2,
        "the return type and `let` annotation are both pending destinations"
    );
    let rows = provider_rows(&program);
    let mut program = program;
    evaluate_selected_const_applications(
        &mut program,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(
        folded_const_arguments(&program).as_slice(),
        &["7".to_owned(), "7".to_owned()]
    );
}

#[test]
fn unselected_boundary_const_application_never_falls_back_to_host_semantics() {
    let (mut program, _rows) = provider_application_fixture();
    let plan = const_application_plan(&program);
    let site = plan.applications[0].site;
    let errors = evaluate_selected_const_applications(
        &mut program,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
    )
    .expect_err("an unselected boundary use must not execute");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("requires exact authored selection")),
        "{errors:?}"
    );
    assert!(
        matches!(
            program.type_reference_table.type_reference(site),
            TypeReferenceNode::ConstExpression(_)
        ),
        "no fold may survive"
    );
}

#[test]
fn provider_free_const_application_folds_without_selected_rows() {
    // A retained application whose closure never needed a provider still folds
    // on this route: it reaches typed trees only when pre-resolution retained
    // it, and closed evaluation answers it exactly.
    let mut program = typed(
        "machine limit() -> u64 { 7 }
         data Buffer<const N: u64> { values: [u8; N]; }
         data Main { value: Buffer<limit()>; }",
    );
    assert!(!pending_const_applications_need_operator_selection(&program, None).unwrap());
    evaluate_selected_const_applications(
        &mut program,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
    )
    .unwrap_or_else(|errors| panic!("provider-free const application: {errors:?}"));
    assert_eq!(
        folded_const_arguments(&program).as_slice(),
        &["7".to_owned()]
    );
}
