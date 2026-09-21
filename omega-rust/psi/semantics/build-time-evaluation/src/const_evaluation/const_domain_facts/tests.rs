use super::{
    PendingMembership, evaluate_selected_domain_facts, pending_memberships,
    pending_memberships_need_operator_selection,
};
use crate::{SelectedBuildTimeOperators, SelectedBuildTimeProviderBody};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionNode;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    // Concrete generic instances are materialized by pre-resolution
    // evaluation: generic data normalization owns the named `FixedBuffer<7>`
    // definition and its carried membership facts.
    let evaluated = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: None,
    })
    .expect("fixture survives pre-resolution evaluation");
    let (syntax, _pre_check) = evaluated.into_syntax_and_pre_check();
    let resolved =
        crate::machine_execution::syntax_probes::resolve(&syntax, None, &[]).expect("resolve");
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

/// Every `where` fact on the named instance is the ordinary `true` fact a
/// proven membership folds to.
fn instance_facts_are_proven(typed: &TypedTrees, name: &str) -> bool {
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == name)
        .unwrap_or_else(|| panic!("instance `{name}`"));
    typed
        .proof_facts
        .span_or_empty(data.where_facts)
        .iter()
        .all(|fact| {
            matches!(fact, ProofFact::Expression(expression)
            if matches!(
                typed.expression_table.expression(*expression),
                ExpressionNode::Boolean(true)
            ))
        })
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
fn selected_provider_body_domain_fact_evaluates_its_ordinary_machine() {
    let (mut typed, rows) = provider_fixture();
    assert_eq!(rows.len(), 1);
    crate::validate_selected_provider_bodies(&typed, &rows).unwrap();
    let pending = pending_memberships(&typed);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].instance_name, "FixedBuffer<7>");

    evaluate_selected_domain_facts(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap_or_else(|errors| panic!("selected provider body must prove the fact: {errors:?}"));

    assert!(
        membership_is_proven(&typed, &pending[0]),
        "the provider's `left + right` body must run; builtin `%` folds 1 and `1 == 9` is false"
    );
    let instance = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "FixedBuffer<7>")
        .unwrap();
    assert!(
        !instance.zero_gated,
        "a fully discharged fact list clears the instance's zero gate"
    );
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

#[test]
fn builtin_only_memberships_never_defer() {
    let mut typed = typed(
        r#"
machine is_positive(value: u64) -> bool { value > 0 }
domain u64::Positive requires is_positive(self);
data FixedBuffer<const N: u64>
where
    N in Positive,
{
    values: [u8; N];
}
data Main { buffer: FixedBuffer<7>; }
machine Main::main(&mut self) {}
"#,
    );
    assert_eq!(pending_memberships(&typed).len(), 1);
    assert!(
        !pending_memberships_need_operator_selection(&typed, None).unwrap(),
        "a builtin-only fact machine retains its early route"
    );
    evaluate_selected_domain_facts(&mut typed, None, SelectedBuildTimeOperators::default())
        .expect("builtin-only memberships fold on the early route");
    assert!(
        instance_facts_are_proven(&typed, "FixedBuffer<7>"),
        "the membership folded to an ordinary `true` fact"
    );
}

/// A nested `self in Inner` fact defers when the inner domain's machine --
/// not the outer's -- is the one whose closure selects a boundary use: the
/// membership arm of the deferral scan must recurse rather than only reading
/// the outer domain's direct expression facts.
#[test]
fn nested_membership_defers_through_the_inner_domain() {
    let mut typed = typed(
        r#"
data Math {}
boundary operator % Math::remainder(left: u64, right: u64) -> u64;
data Provider {}
machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left + right }
machine is_sum(value: u64) -> bool { value % 2 == 9 }
domain u64::Inner requires is_sum(self);
domain u64::Outer requires self in Inner;
data FixedBuffer<const N: u64>
where
    N in Outer,
{
    values: [u8; N];
}
data Main { buffer: FixedBuffer<7>; }
machine Main::main(&mut self) {}
"#,
    );
    assert!(
        pending_memberships_need_operator_selection(&typed, None).unwrap(),
        "the boundary use lives inside Inner's machine; the outer pending membership must still defer"
    );
    let pending = pending_memberships(&typed);
    assert_eq!(pending.len(), 1);

    // Under the selected provider rows the nested membership evaluates
    // end-to-end: the provider's `left + right` body proves `7 + 2 == 9`.
    let rows = provider_rows(&typed);
    assert_eq!(rows.len(), 1);
    crate::validate_selected_provider_bodies(&typed, &rows).unwrap();
    evaluate_selected_domain_facts(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap_or_else(|errors| panic!("nested membership must prove under the provider: {errors:?}"));
    assert!(
        membership_is_proven(&typed, &pending[0]),
        "the nested Inner membership folded to `true` through the selected provider body"
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
