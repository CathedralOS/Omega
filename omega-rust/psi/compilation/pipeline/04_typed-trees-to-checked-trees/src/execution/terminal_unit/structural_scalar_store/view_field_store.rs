//! Shared view field replacement keeps the RHS descriptor's actual origin.
//!
//! The common byte operand planner supplies the same completed operand used
//! by comparisons and predicate scans. This operation retains the exact source
//! expression; source loans and Terminal replay prove backing lifetime, while
//! a separate current-value obligation proves the field's byte predicates.
use super::super::{CheckFacts, TypedTrees};
use super::destination::{Destination, StoreRoots};
use super::value::AssignmentSource;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::terminal_field_identity;

#[allow(clippy::too_many_arguments)]
pub(super) fn store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    roots: StoreRoots<'_>,
    destination: &Destination<'_>,
    carrier_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedUnitEffectOperationPlan> {
    trace.phase("structural field store: borrowed view field: descriptor source");
    if destination.dynamic_index.is_some() {
        return None;
    }
    member_store(
        program,
        facts,
        machine,
        state,
        roots,
        destination,
        carrier_path,
        field,
        statement_index,
        assignment.value,
        source,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn member_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    roots: StoreRoots<'_>,
    destination: &Destination<'_>,
    carrier_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    source: AssignmentSource,
) -> Option<checked_trees::CheckedUnitEffectOperationPlan> {
    // Routed qualifications are not primitive byte-predicate evidence.
    for (_, predicate) in typed_trees::byte_predicates::type_reference_domain_predicates(
        program,
        field.type_reference,
    ) {
        predicate?;
    }
    let target = if let AssignmentSource::StructuralCallResult { result_type, .. } = source {
        result_type
    } else if matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::String(_)
    ) {
        field.type_reference
    } else {
        validation::expression_result_type_reference(program, machine, state, expression)?
    };
    let source = match source {
        AssignmentSource::StructuralCallResult {
            binding_ordinal,
            result_type,
        } => {
            use crate::execution::terminal_unit::types;
            if types::structural_access_for_type_reference(program, result_type)
                != Some(checked_trees::CheckedStructuralAccess::SharedBorrow)
                || !matches!(
                    types::byte_sequence_carrier(program, result_type, &[]),
                    Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
                )
                || !matches!(
                    program.expression_table.expression(expression),
                    typed_trees::expression::ExpressionNode::Call(_)
                )
            {
                return None;
            }
            checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                },
                path: Vec::new(),
                type_identity: types::byte_sequence_type_identity(program, result_type, &[], &[])?,
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            }
        }
        AssignmentSource::CallResult { .. } => return None,
        AssignmentSource::Authored => crate::execution::terminal_unit::byte_operands::plan(
            program,
            &facts.operators,
            &facts.values.scalar_expressions,
            machine,
            state,
            statement_index as usize,
            expression,
            target,
            &|position| match roots {
                StoreRoots::Parameters { structural, .. } => structural
                    .iter()
                    .position(|parameter| parameter.position as usize == position)
                    .and_then(|dense| u32::try_from(dense).ok()),
                StoreRoots::RecordLocals => {
                    crate::execution::terminal_unit::types::dense_structural_parameter_index(
                        program,
                        program.state_parameters(state),
                        position,
                    )
                }
            },
            &|selector| {
                let checked_trees::CheckedScalarExpression::Local { position, .. } =
                    super::value::runtime_index(facts, state, statement_index, selector)?
                else {
                    return None;
                };
                Some(checked_trees::CheckedRuntimeIndex::Local {
                    position: u32::try_from(position).ok()?,
                })
            },
        )?,
    };
    let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
    value.root = value
        .nodes
        .insert(checked_trees::CheckedByteSequenceValue::Descriptor { expression, source });
    Some(
        checked_trees::CheckedUnitEffectOperationPlan::ByteSequenceStore(
            checked_trees::CheckedByteSequenceStorePlan {
                statement_index,
                root: destination.root.destination(),
                destination: checked_trees::CheckedByteSequenceStoreDestination::ViewField {
                    carrier_path,
                    field_identity: terminal_field_identity(program, field.symbol)?,
                },
                value,
            },
        ),
    )
}
