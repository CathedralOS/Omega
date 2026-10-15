//! Shared view descriptor replacement keeps the RHS descriptor's actual origin.
//!
//! A record field retains its declared slot and byte-predicate obligation.
//! Whole local descriptors use ordinary value establishment and rebinding.
//!
//! The common byte operand planner supplies the same completed operand used
//! by comparisons and predicate scans. This operation retains the exact source
//! expression; source loans and Terminal replay prove backing lifetime, while
//! a separate current-value obligation proves the slot's byte predicates.
use super::super::CheckFacts;
use super::destination::{Destination, StoreRoots};
use super::value::AssignmentSource;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::terminal_field_identity;

/// The descriptor slot one store replaces: the carrier path from the root,
/// the slot's declared type, and the field naming it.
pub(super) struct DescriptorSlot<'a> {
    pub(super) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
    pub(super) field: &'a typed_trees::data::DataField,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn store(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    roots: StoreRoots<'_>,
    destination: &Destination<'_>,
    slot: DescriptorSlot<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedUnitEffectOperationPlan> {
    trace.phase("structural field store: borrowed view field: descriptor source");
    if destination.dynamic_index.is_some() {
        return None;
    }
    descriptor_store(
        program,
        facts,
        machine,
        state,
        &|position| {
            roots
                .structural
                .iter()
                .position(|parameter| parameter.position as usize == position)
                .and_then(|dense| u32::try_from(dense).ok())
        },
        destination.root.destination(),
        slot,
        statement_index,
        assignment.value,
        source,
    )
}

/// Replace one descriptor slot with the completed descriptor `expression`
/// denotes. The root's write authority was resolved by its caller;
/// `dense_parameter` names the lane's dense structural argument
/// for an authored parameter position the source reads.
#[allow(clippy::too_many_arguments)]
pub(super) fn descriptor_store(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    dense_parameter: &dyn Fn(usize) -> Option<u32>,
    root: checked_trees::CheckedStoreRoot,
    slot: DescriptorSlot<'_>,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    source: AssignmentSource,
) -> Option<checked_trees::CheckedUnitEffectOperationPlan> {
    // Routed qualifications are not primitive byte-predicate evidence.
    for (_, predicate) in
        typed_trees::byte_predicates::type_reference_domain_predicates(program, slot.type_reference)
    {
        predicate?;
    }
    let target = if let AssignmentSource::StructuralValue { result_type, .. } = source {
        result_type
    } else if matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::String(_)
    ) {
        slot.type_reference
    } else {
        validation::expression_result_type_reference(program, machine, state, expression)?
    };
    let source = match source {
        AssignmentSource::StructuralValue {
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
            &facts.values.scalar_computations,
            &facts.borrow,
            machine,
            state,
            statement_index as usize,
            expression,
            target,
            dense_parameter,
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
    let mut path = slot.path;
    path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
        terminal_field_identity(program, slot.field.symbol)?,
    ));
    let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
    value.root = value
        .nodes
        .insert(checked_trees::CheckedByteSequenceValue::Descriptor { expression, source });
    Some(checked_trees::CheckedUnitEffectOperationPlan::new_store(
        statement_index,
        root,
        path,
        checked_trees::CheckedStoreLeaf::ByteDescriptor,
        checked_trees::CheckedStoreValue::Bytes(value),
    ))
}
