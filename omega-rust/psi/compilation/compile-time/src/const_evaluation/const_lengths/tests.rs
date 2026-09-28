use super::{
    TypeReferenceHandle, TypedTrees, evaluate_const_array_lengths, evaluate_selected_array_lengths,
    validate_folded_array_lengths,
};
use crate::SelectedBuildTimeOperators;
use crate::{SelectedBuildTimeBinaryOperator, SelectedBuildTimeProviderBody};
use checked_interpreter::BuildMachineEntry;
use checked_interpreter::{
    BuildMachineEvaluationRequest, BuildTimeOperationEvaluation, InterpretOptions,
};

// Fold replay consumes the operator-occurrence portion of the checked result.
// These evaluator fixtures supply that source-derived portion only; complete
// source checking and provider settlement are exercised by the corpus callers.
fn checked_occurrences(typed: &TypedTrees) -> checked_trees::CheckedTrees {
    checked_trees::CheckedTrees::with_roots(
        typed.clone(),
        checked_trees::CheckFacts {
            operators: typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed),
            ..Default::default()
        },
    )
}

#[test]
fn final_fold_replay_requires_retained_checked_occurrences() {
    let (float, operators) = fixture("Float");
    let (spelled, spelled_bodies) = provider_fixture();
    let (named, named_bodies) = named_provider_fixture("operator %");
    let (token, token_bodies) = provider_fixture_with_introducer("machine");
    let (requirement, requirement_bodies) = named_provider_fixture("requirement");
    for (name, mut typed, operators, bodies) in [
        ("float", float, operators, Vec::new()),
        ("spelled body", spelled, Vec::new(), spelled_bodies),
        ("named body", named, Vec::new(), named_bodies),
        ("normalized token body", token, Vec::new(), token_bodies),
        (
            "named requirement body",
            requirement,
            Vec::new(),
            requirement_bodies,
        ),
    ] {
        let folds = evaluate_selected_array_lengths(
            &mut typed,
            None,
            SelectedBuildTimeOperators {
                operators: &operators,
                provider_bodies: &bodies,
            },
        )
        .expect(name);
        assert_eq!(folds.len(), 1);
        let checked = if typed.machine_token_bindings().is_empty() {
            checked_occurrences(&typed)
        } else {
            typed_trees_to_checked_trees::lower_typed_trees(
                typed,
                &typed_trees_to_checked_trees::CheckingRequest::settled(),
            )
            .expect(name)
        };
        validate_folded_array_lengths(&checked, &folds, &operators, &bodies, None).expect(name);
        let reject = |changed: &checked_trees::CheckedTrees, reason: &str| {
            let diagnostics =
                validate_folded_array_lengths(changed, &folds, &operators, &bodies, None)
                    .expect_err(name);
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.to_string().contains(reason)),
                "{name}: {diagnostics:?}",
            );
        };
        let mut missing = checked.clone();
        missing.facts.operators.uses.clear();
        missing.facts.operators.named_uses.clear();
        missing.facts.operators.named_requirement_uses.clear();
        reject(&missing, "no unique final checked occurrence");

        let expression = operators
            .first()
            .map_or_else(|| bodies[0].expression, |row| row.expression);
        let mut duplicate = checked.clone();
        let mut wrong_requirement = checked.clone();
        if let Some((handle, fact)) = checked
            .facts
            .operators
            .uses
            .iter()
            .find(|(_, fact)| fact.expression == expression)
        {
            duplicate.facts.operators.uses.append(*fact);
            wrong_requirement
                .facts
                .operators
                .uses
                .get_mut(handle)
                .selected_operator_symbol = symbols::SymbolHandle::invalid();
            if !operators.is_empty() {
                let mut wrong_policy = checked.clone();
                wrong_policy
                    .facts
                    .operators
                    .uses
                    .get_mut(handle)
                    .policy_adapter =
                    checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                        format: numerics::float_semantics::FloatFormat::BINARY64,
                    };
                reject(
                    &wrong_policy,
                    "differs from final checked requirement or arithmetic policy",
                );
            } else {
                let mut both_arenas = checked.clone();
                both_arenas.facts.operators.named_uses.append(
                    checked_trees::CheckedNamedOperatorUseFact {
                        expression: fact.expression,
                        origin: fact.origin,
                        selected_operator_symbol: fact.selected_operator_symbol,
                        policy_adapter: fact.policy_adapter,
                    },
                );
                reject(&both_arenas, "no unique final checked occurrence");
            }
        } else if let Some((handle, fact)) = checked
            .facts
            .operators
            .named_uses
            .iter()
            .find(|(_, fact)| fact.expression == expression)
        {
            duplicate.facts.operators.named_uses.append(*fact);
            wrong_requirement
                .facts
                .operators
                .named_uses
                .get_mut(handle)
                .selected_operator_symbol = symbols::SymbolHandle::invalid();
        } else {
            let (handle, fact) = checked
                .facts
                .operators
                .named_requirement_uses
                .iter()
                .find(|(_, fact)| fact.expression == expression)
                .expect(name);
            duplicate
                .facts
                .operators
                .named_requirement_uses
                .append(*fact);
            wrong_requirement
                .facts
                .operators
                .named_requirement_uses
                .get_mut(handle)
                .requirement_symbol = symbols::SymbolHandle::invalid();
        }
        reject(&duplicate, "no unique final checked occurrence");
        reject(&wrong_requirement, "differs from final checked requirement");
    }
}

#[test]
fn settled_checking_requires_array_length_evaluation_even_after_retained_preliminary_check() {
    let mut typed = crate::front_end::typed_program(
        "machine length() -> u64 { 4 } data Buffer { bytes: [u8; length()]; }",
    );
    let pending_spans = typed
        .type_reference_table
        .fixed_array_lengths()
        .filter_map(|(_, length)| match length {
            typed_trees::types::FixedArrayLength::ConstCall { source_span, .. } => {
                Some(*source_span)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(pending_spans.len(), 1);
    // This crate consumes checking as a dependency, so its actual memo is
    // active. A retained preliminary success must not admit unfinished work.
    let _retention = typed_trees_to_checked_trees::retain_repeated_checks();
    for retain_preliminary in [false, true] {
        if retain_preliminary {
            typed_trees_to_checked_trees::lower_typed_trees(
                typed.clone(),
                &typed_trees_to_checked_trees::CheckingRequest::preliminary(),
            )
            .expect("preliminary checking permits a deferred array length");
        }
        let Err(errors) = typed_trees_to_checked_trees::lower_typed_trees(
            typed.clone(),
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        ) else {
            panic!("settled checking requires evaluated array lengths");
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "array length `length` has an unresolved build-time evaluation dependency",
        );
        assert_eq!(errors[0].source_span, Some(pending_spans[0]));
    }
    evaluate_const_array_lengths(&mut typed, None).expect("ordinary length evaluates");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("the evaluated array length is ready for settled checking");
}

fn fixture(namespace: &str) -> (TypedTrees, Vec<SelectedBuildTimeBinaryOperator>) {
    fixture_with_receiver(namespace, "data Main { bytes:[u8;length()]; }")
}

#[test]
fn selected_float_calls_execute_and_replay_their_source_policy() {
    for (introducer, invocation, overloads) in [
        ("operator", "Float::multiply(left, right)", ""),
        ("machine", "left * right", ""),
        ("machine", "Float::multiply(left, right)", ""),
        (
            "machine",
            "Float::multiply(left, right)",
            "boundary machine * Float::multiply(left:f32, right:f32) -> f32; machine Other::multiply(format:u8, left:f64, right:f64) -> f64 { left }",
        ),
    ] {
        let source = format!(
            r#"
pub boundary {introducer} * Float::multiply(left:f64, right:f64) -> f64;
pub boundary {introducer} == Float::equal(left:f64, right:f64) -> bool;
{overloads}
machine length() -> u64 {{
    let left:f64 in Saturating = 2.0;
    let right:f64 in Saturating = 3.0;
    let product:f64 = ({invocation}) as f64;
    transition product == 6.0 {{ true -> 4 _ -> 5 }}
}}
data Main {{ bytes:[u8;length()]; }}
"#
        );
        let mut typed = crate::front_end::typed_program(&source);
        validation::land_float_literal_destinations(&mut typed);
        let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
        let rows: Vec<_> = facts
            .resolved_expression_selections()
            .map(|(expression, origin, requirement, policy)| {
                SelectedBuildTimeBinaryOperator::prepare(
                    &typed,
                    expression,
                    origin,
                    requirement,
                    policy,
                    checked_trees::CheckedProviderPlanCommitment::from_digest([1; 32]),
                )
                .unwrap()
            })
            .collect();
        assert_eq!(rows.len(), 2, "{introducer}: {invocation}");
        assert!(
            rows.iter()
                .all(|row| row.format == numerics::literals::FloatFormat::F64)
        );
        let multiply = rows
            .iter()
            .find(|row| row.operation == typed_trees::expression::BinaryOperator::Multiply)
            .unwrap();
        assert_eq!(
            multiply.policy,
            checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                format: numerics::float_semantics::FloatFormat::BINARY64,
            }
        );
        let folds = evaluate_selected_array_lengths(
            &mut typed,
            None,
            SelectedBuildTimeOperators {
                operators: &rows,
                provider_bodies: &[],
            },
        )
        .unwrap_or_else(|error| panic!("{introducer}: {invocation}: {error:?}"));
        assert_eq!(folds[0].value, 4);
        let mut checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap_or_else(|error| panic!("{introducer}: {invocation}: {error:?}"));
        validate_folded_array_lengths(&checked, &folds, &rows, &[], None).unwrap();
        if introducer == "machine" {
            let mut stale = checked.typed.clone();
            stale
                .machines_mut()
                .iter_mut()
                .find(|machine| machine.symbol == multiply.requirement)
                .unwrap()
                .spelling = None;
            assert!(
                crate::validate_selected_operators(&stale, &rows)
                    .unwrap_err()
                    .contains("token view differs from its actual requirement")
            );
        }
        let wrong_policy = checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
            format: numerics::float_semantics::FloatFormat::BINARY64,
        };
        if introducer == "machine" {
            let handle = checked
                .facts
                .operators
                .named_requirement_uses
                .iter()
                .next()
                .unwrap()
                .0;
            checked
                .facts
                .operators
                .named_requirement_uses
                .get_mut(handle)
                .policy_adapter = wrong_policy;
        } else {
            let handle = checked.facts.operators.named_uses.iter().next().unwrap().0;
            checked
                .facts
                .operators
                .named_uses
                .get_mut(handle)
                .policy_adapter = wrong_policy;
        }
        let diagnostics =
            validate_folded_array_lengths(&checked, &folds, &rows, &[], None).unwrap_err();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .to_string()
                .contains("differs from final checked requirement or arithmetic policy")),
            "{diagnostics:?}"
        );
    }

    let ordinary = crate::front_end::typed_program(
        "machine unchanged(value:f64) -> f64 { value }
         machine caller(value:f64 in Saturating) -> f64 { unchanged(value) }",
    );
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(
        ordinary,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap_err();
    for rejection in ["implicitly converts", "implicit domain weakening"] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.to_string().contains(rejection)),
            "ordinary calls must retain their carrier and domain checks: {diagnostics:?}"
        );
    }
}

fn fixture_with_receiver(
    namespace: &str,
    receiver: &str,
) -> (TypedTrees, Vec<SelectedBuildTimeBinaryOperator>) {
    fixture_with_operator_contract(namespace, receiver, "")
}

fn fixture_with_operator_contract(
    namespace: &str,
    receiver: &str,
    crash_contract: &str,
) -> (TypedTrees, Vec<SelectedBuildTimeBinaryOperator>) {
    let source = format!(
        r#"
boundary operator * {namespace}::multiply(left:f64, right:f64) -> f64 {crash_contract};
boundary operator * {namespace}::multiply(left:f32, right:f32) -> f32 crashes Trap;
boundary operator == {namespace}::equal(left:f64, right:f64) -> bool;
machine length() -> u64 {{
    let left:f64 = 2.0;
    let right:f64 = 3.0;
    transition left * right == 6.0 {{ true -> 4 _ -> 5 }}
}}
{receiver}
"#
    );
    let mut typed = crate::front_end::typed_program(&source);
    validation::land_float_literal_destinations(&mut typed);
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
    let rows = facts
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
        .filter_map(|fact| {
            let typed_trees::expression::ExpressionNode::Binary(binary) =
                typed.expression_table.expression(fact.expression)
            else {
                return None;
            };
            Some(SelectedBuildTimeBinaryOperator {
                expression: fact.expression,
                origin: fact.origin,
                requirement: fact.selected_operator_symbol,
                operation: binary.operator,
                operands: [binary.left, binary.right],
                format: numerics::literals::FloatFormat::F64,
                policy: fact.policy_adapter,
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([1; 32]),
            })
        })
        .collect();
    (typed, rows)
}

#[test]
fn selected_operator_crash_fences_cover_admission_and_direct_execution() {
    for contract in ["crashes Trap", "crashes Abort", "crashes Trap false"] {
        let (typed, rows) = fixture_with_operator_contract("Float", "", contract);
        assert!(!rows.is_empty());
        let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
        assert!(facts.has_crash_qualified_uses(&typed));
        assert!(
            crate::validate_selected_operators(&typed, &rows)
                .unwrap_err()
                .contains("no build-time execution support")
        );
        let machine = rows[0].origin.machine_symbol().unwrap();
        assert!(
            checked_interpreter::evaluate_build_time_machine(
                &typed,
                BuildMachineEvaluationRequest {
                    product_entry_compatibility: None,
                    operators: &rows,
                    ..BuildMachineEvaluationRequest::symbol(machine, Vec::new())
                }
            )
            .map(BuildTimeOperationEvaluation::into_measured)
            .unwrap_err()
            .contains("no build-time execution support")
        );
        let checked = checked_trees::CheckedTrees::with_roots(
            typed,
            checked_trees::CheckFacts {
                operators: facts,
                ..Default::default()
            },
        );
        assert!(
            checked_interpreter::interpret_entry(
                &checked,
                BuildMachineEntry::Name("length"),
                &[],
                InterpretOptions::default()
            )
            .error
            .unwrap()
            .contains("no checked execution support")
        );
    }
}

#[test]
fn folded_result_replay_rejects_paired_literal_forgery_and_lost_owner() {
    let (mut typed, rows) = fixture("Float");
    let folds = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &rows,
            provider_bodies: &[],
        },
    )
    .unwrap();
    assert_eq!(folds.len(), 1);
    validate_folded_array_lengths(&checked_occurrences(&typed), &folds, &rows, &[], None).unwrap();
    let mut forged = folds.clone();
    forged[0].value = 5;
    typed
        .type_reference_table
        .set_fixed_array_length(forged[0].type_reference, 5);
    assert!(
        validate_folded_array_lengths(&checked_occurrences(&typed), &forged, &rows, &[], None)
            .is_err()
    );
    typed
        .type_reference_table
        .set_fixed_array_length(forged[0].type_reference, 4);
    let owner = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .unwrap()
        .clone();
    let typed_trees::data::DataMember::Field(field) =
        typed.tables.data_members.get_mut(owner.members.start())
    else {
        panic!("bytes");
    };
    field.type_reference = TypeReferenceHandle::invalid();
    assert!(
        validate_folded_array_lengths(&checked_occurrences(&typed), &folds, &rows, &[], None)
            .is_err()
    );
}

#[test]
fn independent_and_selected_lengths_share_full_width_integer_decoding() {
    for (carrier, result) in [("u64", "18446744073709551615"), ("i64", "-1")] {
        let source = format!(
            "machine length() -> {carrier} {{ {result} }} data Main {{ bytes: [u8; length()]; }}"
        );
        let mut selected = crate::front_end::typed_program(&source);
        let mut independent = selected.clone();
        let independent_result = evaluate_const_array_lengths(&mut independent, None);
        let selected_result = evaluate_selected_array_lengths(
            &mut selected,
            None,
            SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &[],
            },
        );
        if carrier == "i64" || usize::BITS < 64 {
            assert!(independent_result.is_err());
            assert!(selected_result.is_err());
            continue;
        }
        // This checks length substitution only, not permission to allocate an
        // enormous array. Placement and resource supply remain later checks.
        independent_result.expect("unsigned length is not a negative host integer");
        let folds = selected_result.expect("selected fold uses the same integer value");
        assert_eq!(folds.len(), 1);
        assert_eq!(folds[0].value, usize::MAX);
        assert_eq!(
            independent
                .type_reference_table
                .fixed_array_lengths()
                .collect::<Vec<_>>(),
            selected
                .type_reference_table
                .fixed_array_lengths()
                .collect::<Vec<_>>()
        );
        validate_folded_array_lengths(&checked_occurrences(&selected), &folds, &[], &[], None)
            .expect("independent replay retains the unsigned result");
    }
}

#[test]
fn equal_signature_authored_boundary_cannot_claim_float_semantics() {
    let (typed, rows) = fixture("Math");
    assert!(!rows.is_empty());
    assert!(
        crate::validate_selected_operators(&typed, &rows)
            .unwrap_err()
            .contains("sealed Float meaning")
    );
}

#[test]
fn single_supplied_row_cannot_authorize_another_live_expression_origin() {
    let (mut typed, rows) = fixture("Float");
    crate::validate_selected_operators(&typed, &rows).unwrap();
    let row = rows[0];
    let machine = typed
        .machines()
        .iter()
        .find(|machine| Some(machine.symbol) == row.origin.machine_symbol())
        .unwrap()
        .clone();
    let mut statements = typed.machine_states(&machine)[0].statement_nodes;
    typed.statement_table.push_statement(
        &mut statements,
        typed_trees::statement::StatementNode::Expression(row.expression),
    );
    typed.machine_states_mut(&machine)[0].statement_nodes = statements;
    assert!(
        crate::validate_selected_operators(&typed, &rows)
            .unwrap_err()
            .contains("distinct current origins")
    );
}

/// A selected boundary-operator use whose settled plan binds an ordinary
/// checked adapter. The provider's body computes `left | right`, which differs
/// from the builtin `%` result, so a fold of 7 instead of 1 is the positive
/// witness that the provider's machine -- not host arithmetic -- ran.
fn provider_fixture() -> (TypedTrees, Vec<SelectedBuildTimeProviderBody>) {
    provider_fixture_with_introducer("operator")
}

fn provider_fixture_with_introducer(
    introducer: &str,
) -> (TypedTrees, Vec<SelectedBuildTimeProviderBody>) {
    let source = r#"
data Math {}
boundary operator % Math::remainder(left: u64, right: u64) -> u64;
data Provider {}
machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left | right }
machine length() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (left % right) } }
data Main { bytes:[u8;length()]; }
"#;
    let source = source.replace("boundary operator", &format!("boundary {introducer}"));
    let typed = crate::front_end::typed_program(&source);
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
    let rows = facts
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
        .filter(|fact| {
            typed
                .operators()
                .iter()
                .chain(typed.machine_token_bindings())
                .any(|operator| {
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
                operands: fact.operands(&typed).unwrap(),
                provider_machine: provider.symbol,
                provider_state: entry.symbol,
                provider_type: provider.attached_data.as_ref().unwrap().as_str().to_owned(),
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]),
            }
        })
        .collect();
    (typed, rows)
}

/// The same selected provider-body execution, invoked through a uniquely
/// resolved named call rather than the spelled token. The provider's body
/// still computes `left | right` = 7 where builtin `%` would fold 1.
fn named_provider_fixture(introducer: &str) -> (TypedTrees, Vec<SelectedBuildTimeProviderBody>) {
    let source = r#"
data Math {}
boundary operator % Math::remainder(left: u64, right: u64) -> u64;
data Provider {}
machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left | right }
machine length() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (Math::remainder(left, right)) } }
data Main { bytes:[u8;length()]; }
"#;
    let source = source.replace("operator %", introducer);
    let typed = crate::front_end::typed_program(&source);
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
    let rows = facts
        .named_uses()
        .map(|fact| (fact.expression, fact.origin, fact.selected_operator_symbol))
        .chain(
            facts
                .named_requirement_uses()
                .map(|fact| (fact.expression, fact.origin, fact.requirement_symbol)),
        )
        .map(|(expression, origin, requirement)| {
            let provider = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Provider::remainder")
                .unwrap();
            SelectedBuildTimeProviderBody::prepare(
                &typed,
                expression,
                origin,
                requirement,
                provider,
                provider.attached_data.as_ref().unwrap().as_str().to_owned(),
                checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]),
            )
            .unwrap()
        })
        .collect();
    (typed, rows)
}

#[test]
fn token_machine_provider_body_executes_and_replays() {
    let (mut typed, rows) = provider_fixture_with_introducer("machine");
    assert_eq!(rows.len(), 1);
    crate::validate_selected_provider_bodies(&typed, &rows).unwrap();
    let folds = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap();
    assert_eq!(folds.len(), 1);
    assert_eq!(
        folds[0].value, 7,
        "execute the selected body, not builtin modulo"
    );
    validate_folded_array_lengths(&checked_occurrences(&typed), &folds, &[], &rows, None).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check the complete folded source before replay");
    assert_eq!(checked.facts.operators.named_requirement_uses().count(), 1);
    validate_folded_array_lengths(&checked, &folds, &[], &rows, None)
        .expect("normalizing a token machine to a requirement call preserves the original fold");
    let mut stale = checked.typed;
    stale
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == rows[0].requirement)
        .unwrap()
        .spelling = None;
    assert!(
        crate::validate_selected_provider_bodies(&stale, &rows).is_err(),
        "normalization must not hide a stale token view of the actual requirement"
    );
}

#[test]
fn machine_provider_body_rechecks_the_actual_requirement() {
    for (mut typed, rows) in [
        provider_fixture_with_introducer("machine"),
        named_provider_fixture("requirement"),
    ] {
        crate::validate_selected_provider_bodies(&typed, &rows).unwrap();
        let mut substituted = typed.clone();
        let provider = substituted
            .machines()
            .iter()
            .find(|machine| machine.symbol == rows[0].provider_machine)
            .unwrap();
        let conformances = provider.satisfies;
        substituted
            .machine_trait_conformances
            .span_mut_or_empty(conformances)[0]
            .symbol = rows[0].provider_machine;
        assert!(
            crate::validate_selected_provider_bodies(&substituted, &rows)
                .unwrap_err()
                .contains("exactly one checked conformance"),
            "matching names and signatures cannot replace the exact satisfaction target"
        );
        if !typed.machine_token_bindings().is_empty() {
            let mut stale_binding = typed.clone();
            stale_binding
                .machines_mut()
                .iter_mut()
                .find(|machine| machine.symbol == rows[0].requirement)
                .unwrap()
                .spelling = None;
            assert!(
                crate::validate_selected_provider_bodies(&stale_binding, &rows).is_err(),
                "the signature view must retain the actual machine's token binding"
            );
        }
        // Leave the use untouched while replacing the actual requirement's supply.
        // A plain named call loses its requirement fact immediately; a token
        // view retains the occurrence but fails actual conformance replay.
        let expected_rejection = if typed.machine_token_bindings().is_empty() {
            "distinct current origins"
        } else {
            "exactly one checked conformance"
        };
        let requirement = typed
            .machines_mut()
            .iter_mut()
            .find(|machine| machine.symbol == rows[0].requirement)
            .unwrap();
        requirement.supply_mode = language_semantics::MachineSupplyMode::CheckedBody;
        let rejection = crate::validate_selected_provider_bodies(&typed, &rows).unwrap_err();
        assert!(rejection.contains(expected_rejection), "{rejection}");
    }
}

#[test]
fn named_provider_body_executes_its_ordinary_machine_and_replays() {
    for introducer in ["operator %", "requirement"] {
        let (mut typed, rows) = named_provider_fixture(introducer);
        assert_eq!(rows.len(), 1);
        crate::validate_selected_provider_bodies(&typed, &rows).unwrap();
        let folds = evaluate_selected_array_lengths(
            &mut typed,
            None,
            SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &rows,
            },
        )
        .unwrap();
        assert_eq!(folds.len(), 1);
        assert_eq!(
            folds[0].value, 7,
            "the provider's `left | right` body must run; builtin `%` would fold 1"
        );
        validate_folded_array_lengths(&checked_occurrences(&typed), &folds, &[], &rows, None)
            .unwrap();
    }
}

#[test]
fn unselected_boundary_use_cannot_fall_back_to_host_semantics() {
    let (mut typed, _rows) = provider_fixture();
    let diagnostics = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
    )
    .unwrap_err();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("requires exact authored selection")),
        "an unselected boundary use must never reach builtin execution: {diagnostics:?}"
    );
}

#[test]
fn provider_body_row_rejects_stale_and_substituted_custody() {
    for (typed, rows) in [
        provider_fixture(),
        provider_fixture_with_introducer("machine"),
        named_provider_fixture("requirement"),
    ] {
        assert_provider_body_row_rejects_stale_and_substituted_custody(typed, rows);
    }
}

fn assert_provider_body_row_rejects_stale_and_substituted_custody(
    typed: TypedTrees,
    rows: Vec<SelectedBuildTimeProviderBody>,
) {
    let mut invalid_entry = rows.clone();
    invalid_entry[0].provider_state = symbols::SymbolHandle::invalid();
    assert!(
        crate::validate_selected_provider_bodies(&typed, &invalid_entry)
            .unwrap_err()
            .contains("executable entry")
    );

    let caller = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "length")
        .unwrap();
    let mut substituted = rows.clone();
    substituted[0].provider_machine = caller.symbol;
    substituted[0].provider_state = typed.machine_states(caller).first().unwrap().symbol;
    assert!(
        crate::validate_selected_provider_bodies(&typed, &substituted).is_err(),
        "a substituted provider machine must never satisfy the row"
    );

    let mut anonymous = rows.clone();
    anonymous[0].provider = checked_trees::CheckedProviderPlanCommitment::default();
    assert!(
        crate::validate_selected_provider_bodies(&typed, &anonymous)
            .unwrap_err()
            .contains("unique exact provider custody")
    );

    let mut swapped = rows.clone();
    swapped[0].operands.swap(0, 1);
    assert!(
        crate::validate_selected_provider_bodies(&typed, &swapped)
            .unwrap_err()
            .contains("differ")
    );

    let mut stale = rows.clone();
    stale[0].expression = stale[0].operands[0];
    assert!(
        crate::validate_selected_provider_bodies(&typed, &stale).is_err(),
        "a stale expression coordinate must never rejoin a current use"
    );
}

#[test]
fn folded_payload_retains_its_exact_variant_parent() {
    let (mut typed, rows) = fixture_with_receiver(
        "Float",
        "data Main { case First(bytes:[u8;length()]); case Second; }",
    );
    let folds = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &rows,
            provider_bodies: &[],
        },
    )
    .unwrap();
    validate_folded_array_lengths(&checked_occurrences(&typed), &folds, &rows, &[], None).unwrap();
    let owner = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .unwrap()
        .clone();
    let members = typed.tables.data_members.span_mut(owner.members).unwrap();
    let [
        typed_trees::data::DataMember::Variant(first),
        typed_trees::data::DataMember::Variant(second),
    ] = members
    else {
        panic!("two cases");
    };
    std::mem::swap(&mut first.payload, &mut second.payload);
    assert!(
        validate_folded_array_lengths(&checked_occurrences(&typed), &folds, &rows, &[], None)
            .is_err()
    );
}
