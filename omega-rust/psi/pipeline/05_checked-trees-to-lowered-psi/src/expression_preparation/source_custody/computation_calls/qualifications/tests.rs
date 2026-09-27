use super::super::super::CheckedScalarExpressionRole;
use super::super::{CheckedScalarComputationKind, validate_computation_calls};
use super::{
    CheckedScalarComputationHandle, CheckedTrees, ExpressionNode, LoweringError, PrimitiveType,
    TypeReferenceHandle,
};
use crate::TerminalMachineSelection;
use language_semantics::SemanticDomainId;

fn root(checked: &CheckedTrees) -> checked_trees::CheckedScalarComputationRoot {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("choose");
    checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| {
            root.machine == machine.symbol && root.role == CheckedScalarExpressionRole::Return
        })
        .expect("qualified return graph")
        .clone()
}

fn replay(checked: &CheckedTrees) -> Result<(), LoweringError> {
    let root = root(checked);
    let source = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(root.root)
        .authored_root;
    validate_computation_calls(
        checked,
        root.machine,
        root.state,
        root.statement_ordinal,
        root.root,
        source,
    )
}

fn qualifications(checked: &CheckedTrees) -> Vec<CheckedScalarComputationHandle> {
    crate::expression_preparation::computation_graph::reachable_nodes(
        checked,
        &[root(checked).root],
    )
    .expect("closure")
    .into_iter()
    .filter(|handle| {
        matches!(
            checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get(*handle)
                .kind,
            CheckedScalarComputationKind::Qualification { .. }
        )
    })
    .collect()
}

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/expressions/match_domain_results/main.omg"
));

const ERASURE_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/expressions/explicit_scalar_tag_erasure/main.omg"
));

#[test]
fn explicit_erasure_replay_requires_its_authored_cast_and_bare_result() {
    for mutation in 0..3 {
        let mut checked = crate::front_end::checked_program(ERASURE_SOURCE);
        replay(&checked).expect("explicit erasure");
        let handle = qualifications(&checked)[0];
        let CheckedScalarComputationKind::Qualification {
            source_expression,
            operand,
            ..
        } = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(handle)
            .kind
        else {
            panic!("explicit cast node");
        };
        match mutation {
            0 => {
                let ExpressionNode::Cast(cast) =
                    checked.expression_table.expression(source_expression)
                else {
                    panic!("cast");
                };
                let replacement = checked.expression_table.expression(cast.value).clone();
                *checked
                    .typed
                    .expression_table
                    .expression_mut(source_expression) = replacement;
            }
            1 => {
                let ExpressionNode::Cast(cast) = checked
                    .typed
                    .expression_table
                    .expression_mut(source_expression)
                else {
                    panic!("cast");
                };
                cast.result_type = TypeReferenceHandle::invalid();
            }
            2 => {
                let mut replacement = checked
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(operand)
                    .clone();
                replacement.value_source = source_expression;
                *checked
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(handle) = replacement;
            }
            _ => unreachable!(),
        }
        assert!(replay(&checked).is_err(), "mutation {mutation}");
    }
}

#[test]
fn qualification_replay_rejects_missing_duplicate_or_redirected_evidence() {
    for mutation in 0..5 {
        let mut checked = crate::front_end::checked_program(SOURCE);
        let first = checked.facts.qualifications.vacuous_uses[0];
        match mutation {
            0 => {
                checked.facts.qualifications.vacuous_uses.clear();
            }
            1 => checked.facts.qualifications.vacuous_uses.push(first),
            2 => checked.facts.qualifications.vacuous_uses[0].statement_index += 1,
            3 => {
                checked.facts.qualifications.vacuous_uses[0].domain =
                    symbols::SymbolHandle::invalid()
            }
            4 => {
                checked.facts.qualifications.vacuous_uses[0].semantic_domain =
                    SemanticDomainId::NULL
            }
            _ => unreachable!(),
        }
        assert!(replay(&checked).is_err(), "mutation {mutation}");
    }
}

#[test]
fn qualification_replay_rejects_relabelled_cast_operand_and_result() {
    for mutation in 0..4 {
        let mut checked = crate::front_end::checked_program(SOURCE);
        let handles = qualifications(&checked);
        let CheckedScalarComputationKind::Qualification {
            source_expression: other_source,
            operand: other_operand,
            ..
        } = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(handles[1])
            .kind
        else {
            panic!("qualification");
        };
        let node = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handles[0]);
        let CheckedScalarComputationKind::Qualification {
            source_expression,
            operand,
            result_type,
        } = &mut node.kind
        else {
            panic!("qualification");
        };
        match mutation {
            0 => *source_expression = other_source,
            1 => *operand = other_operand,
            2 => *result_type = TypeReferenceHandle::invalid(),
            3 => node.primitive_type = PrimitiveType::U64,
            _ => unreachable!(),
        }
        assert!(replay(&checked).is_err(), "mutation {mutation}");
    }
}

#[test]
fn qualification_replay_rejects_erasing_node_even_with_cast_source_on_pure_value() {
    for retain_cast_source in [false, true] {
        let mut checked = crate::front_end::checked_program(SOURCE);
        let handle = qualifications(&checked)[0];
        let CheckedScalarComputationKind::Qualification {
            source_expression,
            operand,
            ..
        } = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(handle)
            .kind
        else {
            panic!("qualification");
        };
        let mut replacement = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(operand)
            .clone();
        if retain_cast_source {
            replacement.value_source = source_expression;
        }
        *checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle) = replacement;
        assert!(
            replay(&checked).is_err(),
            "retain_cast_source={retain_cast_source}"
        );
    }
}

#[test]
fn qualification_replay_reconstructs_indices_when_retained_records_agree_on_a_substitution() {
    let source = "domain<const Axis: u64> i64::Coordinate<Axis>;
        machine choose(value: i64) -> i64 in Coordinate<7> { value as i64 in Coordinate<7> }
        machine other(value: i64) -> i64 in Coordinate<8> { value as i64 in Coordinate<8> }";
    for change_result in [false, true] {
        let mut checked = crate::front_end::checked_program(source);
        let uses = &checked.facts.qualifications.vacuous_uses;
        let first_source = uses[0].expression;
        let ExpressionNode::Cast(other) = checked.expression_table.expression(uses[1].expression)
        else {
            panic!("other cast");
        };
        let other = *other;
        let handle = qualifications(&checked)[0];
        if change_result {
            let ExpressionNode::Cast(first) =
                checked.typed.expression_table.expression_mut(first_source)
            else {
                panic!("first cast");
            };
            first.result_type = other.result_type;
            let CheckedScalarComputationKind::Qualification { result_type, .. } = &mut checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(handle)
                .kind
            else {
                panic!("qualification");
            };
            *result_type = other.result_type;
        } else {
            let ExpressionNode::Cast(first) =
                checked.typed.expression_table.expression_mut(first_source)
            else {
                panic!("first cast");
            };
            first.semantic_domain_id = other.semantic_domain_id;
            checked.facts.qualifications.vacuous_uses[0].semantic_domain = other.semantic_domain_id;
        }
        assert!(replay(&checked).is_err(), "change_result={change_result}");
    }
}

#[test]
fn qualification_replay_rechecks_declaration_instead_of_trusting_vacuous_use() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    let definitions = checked.typed.roots.domain_definitions;
    checked
        .typed
        .tables
        .domain_definitions
        .span_mut(definitions)
        .expect("declarations")[0]
        .predicate_body = language_semantics::DomainPredicateBody::Present;
    assert!(
        replay(&checked).is_err(),
        "an unchanged vacuous-use row cannot hide a predicate"
    );
}

#[test]
fn indexed_call_replay_rechecks_parameter_contract_instance() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/expressions/indexed_qualified_call_argument/main.omg"
    ));
    for erase_arguments in [false, true] {
        let mut checked = crate::front_end::checked_program(source);
        crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("original exact call contract");
        let membership = checked
            .typed
            .proof_facts
            .iter()
            .find_map(|(handle, fact)| {
                matches!(fact, typed_trees::domain::ProofFact::Membership(_)).then_some(handle)
            })
            .expect("qualified parameter requirement");
        let typed_trees::domain::ProofFact::Membership(membership) =
            checked.typed.proof_facts.get_mut(membership)
        else {
            panic!("membership");
        };
        if erase_arguments {
            membership.domain_arguments = arena::HandleSpan::empty();
        } else {
            membership.semantic_domain = SemanticDomainId::NULL;
        }
        assert!(
            crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).is_err(),
            "erase_arguments={erase_arguments}"
        );
    }
}
