//! Nominal cleanup Boolean and caller requirements with their
//! diagnostics.

use crate::execution::terminal_unit::types::terminal_field_identity;
use crate::execution::terminal_unit::{
    CheckFacts, CheckedUnitNominalAffineCallerRequirementPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, ContractProofFactKind, ContractProofFactOwner,
    DataMember, Diagnostic, ExpressionNode, PrimitiveType, StateParameter, SymbolHandle,
};
use crate::places::place_at;

pub(crate) fn nominal_cleanup_boolean_requirements(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    cleanup_machine: &typed_trees::machine::Machine,
    cleanup_state: &typed_trees::state::State,
    cleanup_receiver: &StateParameter,
) -> Option<Vec<CheckedUnitNominalAffineCleanupRequirementPlan>> {
    let checked_requires =
        checked_requires_expressions(program, facts, cleanup_machine.symbol, cleanup_state.symbol)?;
    let requirements = checked_requires
        .into_iter()
        .map(|expression| {
            direct_boolean_field_requirement(
                program,
                cleanup_state.symbol,
                cleanup_receiver,
                expression,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    Some(canonical_nominal_cleanup_requirements(requirements))
}

pub(crate) fn nominal_cleanup_caller_boolean_requirements(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    source_parameters: &[StateParameter],
) -> Option<Vec<CheckedUnitNominalAffineCallerRequirementPlan>> {
    // The caller preserves its checked entry requirements unchanged, and this
    // plan admits only direct Boolean root facts. Any other requirement
    // refuses the plan: wider bodies must instead consult path-specific exit
    // contexts.
    let caller_requires =
        checked_requires_expressions(program, facts, caller_machine.symbol, caller_state.symbol)?;
    let mut structural_requirements = Vec::new();
    for expression in caller_requires {
        if let Some(requirement) = source_parameters.iter().enumerate().find_map(
            |(source_parameter_index, source_parameter)| {
                let source_parameter_index = u32::try_from(source_parameter_index).ok()?;
                direct_boolean_field_requirement(
                    program,
                    caller_state.symbol,
                    source_parameter,
                    expression,
                )
                .map(
                    |requirement| CheckedUnitNominalAffineCallerRequirementPlan {
                        source_parameter_index,
                        field_identity: requirement.field_identity,
                        expected: requirement.expected,
                    },
                )
            },
        ) {
            structural_requirements.push(requirement);
            continue;
        }
        return None;
    }
    // Bound generic prerequisites share the caller's invocation namespace but
    // remain separate from authored clauses. This plan represents only a direct
    // Boolean field: a deeper structural path requires its wider owning plan.
    let contextual = &facts.contextual_cleanup.entry_requirements;
    for (_, requirement) in contextual.rows.iter() {
        if requirement.machine != caller_machine.symbol {
            continue;
        }
        // This bounded plan disposes every listed owner unconditionally.
        // A guarded implication is not its consequent: only a general edge
        // cleanup plan may use the selected edge's premises to discharge it.
        if requirement.antecedent != checked_trees::CheckedBooleanExpression::Constant(true) {
            return None;
        }
        let (parameter_position, path, expected) = requirement.direct_boolean_subject()?;
        let [checked_trees::CheckedStructuralPredicatePathSegment::Field(field_identity)] = path
        else {
            return None;
        };
        structural_requirements.push(CheckedUnitNominalAffineCallerRequirementPlan {
            source_parameter_index: parameter_position,
            field_identity: field_identity.clone(),
            expected,
        });
    }
    structural_requirements.sort_by(|left, right| {
        left.source_parameter_index
            .cmp(&right.source_parameter_index)
            .then(left.field_identity.cmp(&right.field_identity))
            .then(left.expected.cmp(&right.expected))
    });
    structural_requirements.dedup();
    Some(structural_requirements)
}

pub(crate) fn nominal_cleanup_missing_requirement(
    source_parameter_index: u32,
    caller_requirements: &[CheckedUnitNominalAffineCallerRequirementPlan],
    required: &[CheckedUnitNominalAffineCleanupRequirementPlan],
) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
    required
        .iter()
        .find(|requirement| {
            !caller_requirements.iter().any(|caller| {
                caller.source_parameter_index == source_parameter_index
                    && caller.field_identity == requirement.field_identity
                    && caller.expected == requirement.expected
            })
        })
        .cloned()
}

pub(crate) fn canonical_nominal_cleanup_requirements(
    mut requirements: Vec<CheckedUnitNominalAffineCleanupRequirementPlan>,
) -> Vec<CheckedUnitNominalAffineCleanupRequirementPlan> {
    requirements.sort_by(|left, right| {
        left.field_identity
            .cmp(&right.field_identity)
            .then(left.expected.cmp(&right.expected))
    });
    requirements.dedup();
    requirements
}

pub(crate) fn nominal_cleanup_missing_requirement_diagnostic(
    program: &dyn ::validation::CheckingInput,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    source_parameter: &StateParameter,
    cleanup_machine: &typed_trees::machine::Machine,
    missing: CheckedUnitNominalAffineCleanupRequirementPlan,
) -> Diagnostic {
    let edge = format!(
        "automatic cleanup requires at Unit return edge from {} state {} after statement 0",
        crate::labels::machine_name(program, caller_machine.symbol),
        crate::labels::symbol_name(program, caller_state.symbol),
    );
    Diagnostic::error(format!(
        "cannot prove {edge}: missing {}.{} == {} required by {}",
        source_parameter.name.as_str(),
        missing.field_identity.spelling(),
        missing.expected,
        crate::labels::machine_name(program, cleanup_machine.symbol),
    ))
}

pub(crate) fn checked_requires_expressions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
    checked_contract_expressions(
        program,
        facts,
        machine,
        state,
        ContractProofFactKind::Requires,
    )
}

pub(crate) fn checked_contract_expressions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    kind: ContractProofFactKind,
) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
    let mut expressions = Vec::new();
    for (_, checked) in facts.proof.contract_facts.iter().filter(|(_, checked)| {
        matches!(checked.owner, ContractProofFactOwner::Machine { machine_symbol } if machine_symbol == machine)
            || matches!(checked.owner, ContractProofFactOwner::MachineState { machine_symbol, state_symbol } if machine_symbol == machine && state_symbol == state)
    }) {
        if checked.kind != kind {
            continue;
        }
        let expression = program.proof_facts.get(checked.fact).as_expression()?;
        expressions.push(*expression);
    }
    Some(expressions)
}

pub(crate) fn direct_boolean_field_requirement(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    root_parameter: &StateParameter,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};

    let (field_expression, expected) = match program.expression_table.expression(expression) {
        ExpressionNode::Member(_) => (expression, true),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            if !matches!(
                program.expression_table.expression(unary.operand),
                ExpressionNode::Member(_)
            ) {
                return None;
            }
            (unary.operand, false)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let (field, literal) = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), ExpressionNode::Member(_)) => {
                    (binary.right, *literal)
                }
                (ExpressionNode::Member(_), ExpressionNode::Boolean(literal)) => {
                    (binary.left, *literal)
                }
                _ => return None,
            };
            (
                field,
                if binary.operator == BinaryOperator::Equal {
                    literal
                } else {
                    !literal
                },
            )
        }
        _ => return None,
    };
    let place = place_at(program, state, 0, field_expression)?;
    let [facts::PlaceSegment::Field { symbol }] = place.segments.as_slice() else {
        return None;
    };
    if place.root != facts::PlaceRoot::Symbol(root_parameter.symbol)
        || !program.data_definitions().iter().any(|data| {
            program.data_members(data).iter().any(|member| {
                matches!(member, DataMember::Field(field)
                    if field.symbol == *symbol
                        && program.primitive_type_reference(field.type_reference)
                            == Some(PrimitiveType::Bool))
            })
        })
    {
        return None;
    }
    Some(CheckedUnitNominalAffineCleanupRequirementPlan {
        field_identity: terminal_field_identity(program, *symbol)?,
        expected,
    })
}
