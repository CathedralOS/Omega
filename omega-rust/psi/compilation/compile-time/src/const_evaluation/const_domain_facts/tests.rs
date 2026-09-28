use super::{
    PendingMembership, evaluate_selected_domain_facts, pending_memberships,
    pending_memberships_need_operator_selection,
};
use crate::{SelectedBuildTimeOperators, SelectedBuildTimeProviderBody};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionNode;

fn typed(source: &str) -> TypedTrees {
    let syntax = crate::front_end::syntax_program(source);
    // Concrete generic instances are materialized by pre-resolution
    // evaluation: generic data normalization owns the named `FixedBuffer<7>`
    // definition and its carried membership facts.
    let evaluated = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: None,
    })
    .expect("fixture survives pre-resolution evaluation");
    let (syntax, _pre_check) = evaluated.into_syntax_and_pre_check();
    let resolved = crate::syntax_probes::resolve(&syntax, None, &[]).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

/// A concrete membership whose fact machine's closure holds a selected
/// boundary-operator use. The provider's body computes `left + right`, which
/// differs from the builtin `%` result, so `is_sum(7)` proving `7 + 2 == 9`
/// is the positive witness that the provider's machine -- not host
/// arithmetic -- ran.
fn provider_fixture() -> (TypedTrees, Vec<SelectedBuildTimeProviderBody>) {
    let source = r#"
data Math {}
boundary operator % Math::remainder(left: u64, right: u64) -> u64;
data Provider {}
machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left + right }
machine is_sum(value: u64) -> bool { value % 2 == 9 }
domain u64::Summable requires is_sum(self);
data FixedBuffer<const N: u64>
where
    N in Summable,
{
    values: [u8; N];
}
data Main { buffer: FixedBuffer<7>; }
machine Main::main(&mut self) {}
"#;
    let typed = typed(source);
    let rows = provider_rows(&typed);
    (typed, rows)
}

/// The selected provider bodies for every resolved boundary-operator use in
/// the fixture, rebound to `Provider::remainder`'s ordinary checked body.
fn provider_rows(typed: &TypedTrees) -> Vec<SelectedBuildTimeProviderBody> {
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed);
    facts
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
        .filter(|fact| {
            typed.operators().iter().any(|operator| {
                operator.symbol == fact.selected_operator_symbol && operator.is_boundary
            })
        })
        .map(|fact| {
            let provider = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Provider::remainder")
                .unwrap();
            let entry = typed.machine_states(provider).first().unwrap();
            SelectedBuildTimeProviderBody {
                expression: fact.expression,
                origin: fact.origin,
                requirement: fact.selected_operator_symbol,
                operands: fact.operands(typed).unwrap(),
                provider_machine: provider.symbol,
                provider_state: entry.symbol,
                provider_type: provider.attached_data.as_ref().unwrap().as_str().to_owned(),
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]),
            }
        })
        .collect()
}

fn membership_is_proven(typed: &TypedTrees, pending: &PendingMembership) -> bool {
    matches!(
        typed.proof_facts.get(pending.fact),
        ProofFact::Expression(expression)
            if matches!(
                typed.expression_table.expression(*expression),
                ExpressionNode::Boolean(true)
            )
    )
}

#[test]
fn provider_dependent_membership_defers_instead_of_failing_unselected() {
    let (mut typed, rows) = provider_fixture();
    assert_eq!(rows.len(), 1);
    assert!(
        pending_memberships_need_operator_selection(&typed, None).unwrap(),
        "a fact machine whose closure selects a boundary use must defer"
    );
    let pending = pending_memberships(&typed);
    assert_eq!(pending.len(), 1);

    // The early route still fails closed rather than borrowing builtin
    // semantics for a resolved boundary use.
    let diagnostics = evaluate_selected_domain_facts(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
    )
    .expect_err("an unselected boundary use must never reach builtin execution");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("requires exact authored selection")),
        "{diagnostics:?}"
    );
    assert!(
        !membership_is_proven(&typed, &pending[0]),
        "a rejected evaluation keeps the authored membership"
    );
}

/// A machine fact that evaluates to `false` names the generic instance in a
/// direct diagnostic -- it is a rejection, not a pending membership left
/// authored.
#[test]
fn false_membership_rejects_naming_the_instance() {
    let mut typed = typed(
        r#"
machine is_large(value: u64) -> bool { value > 100 }
domain u64::Big requires is_large(self);
data FixedBuffer<const N: u64>
where
    N in Big,
{
    values: [u8; N];
}
data Main { buffer: FixedBuffer<7>; }
machine Main::main(&mut self) {}
"#,
    );
    let pending = pending_memberships(&typed);
    assert_eq!(pending.len(), 1);
    let diagnostics =
        evaluate_selected_domain_facts(&mut typed, None, SelectedBuildTimeOperators::default())
            .expect_err("a fact machine returning `false` rejects the membership");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("const fact for generic instance `FixedBuffer<7>` is false")),
        "{diagnostics:?}"
    );
    assert!(
        !membership_is_proven(&typed, &pending[0]),
        "a rejected membership stays authored, never folded to `true`"
    );
}

/// A concrete literal beyond the evaluator's signed storage boundary is a
/// reported failure, not a truncated or silently skipped membership.
#[test]
fn literal_beyond_i64_reports_the_signed_boundary() {
    let mut typed = typed(
        r#"
machine is_anything(value: u64) -> bool { true }
domain u64::Any requires is_anything(self);
data Holder<const N: u64>
where
    N in Any,
{
    marker: u8;
}
data Main { holder: Holder<18446744073709551615>; }
machine Main::main(&mut self) {}
"#,
    );
    let pending = pending_memberships(&typed);
    assert_eq!(pending.len(), 1);
    let diagnostics =
        evaluate_selected_domain_facts(&mut typed, None, SelectedBuildTimeOperators::default())
            .expect_err("a literal past i64::MAX cannot evaluate silently");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("does not fit the build-time evaluator's signed integer boundary")),
        "{diagnostics:?}"
    );
}
