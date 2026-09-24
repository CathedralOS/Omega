//! Byte-sequence leaves: a whole borrowed byte view, or a bounded-owned byte
//! field below an exclusive root. These leaves have their own Terminal
//! operations (`ByteSequenceWrite`, `StructuralByteSequenceFieldStore`,
//! `StructuralByteSequenceFieldByteStore`), but the destination and the `u8`
//! value are resolved by the same owners as every other store.
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan, ExpressionNode,
    Multiplicity, TypeReferenceNode, TypedTrees,
};
use super::destination::{Destination, Root};
use super::value::{self, AssignmentSource};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{byte_sequence_carrier, terminal_field_identity};

/// `view[index] = byte` through a whole mutable borrowed byte view. Replacing
/// a byte observes the view's current length, so the root needs an ordinary
/// unrestricted, unqualified mutable borrow rather than write-only authority.
pub(super) fn view_write(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let Root::Parameter { plan, parameter } = destination.root else {
        return None;
    };
    let index = destination.dynamic_index?;
    if !destination.place.segments.is_empty()
        || plan.is_self
        || plan.access != CheckedStructuralAccess::MutableBorrow
        || plan.multiplicity != Multiplicity::Unrestricted
        || !plan.qualifications.is_empty()
        || plan.fused_service_erasure.is_some()
        || byte_sequence_carrier(program, parameter.type_reference, &[])
            != Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::ByteSequenceWrite(
        checked_trees::CheckedByteSequenceWritePlan {
            statement_index,
            destination_parameter_position: plan.position,
            index: value::runtime_index(facts, state, statement_index, index)?,
            value: value::byte_value(
                program,
                facts,
                machine,
                state,
                &destination.root,
                statement_index,
                assignment,
                source,
                trace,
            )?,
        },
    ))
}

/// A byte field leaf: `field[index] = byte` replaces one live byte of a
/// bounded-owned field or of a mutable borrowed view, and `field =
/// "literal"` replaces a bounded-owned field's live prefix (`capacity` is
/// that field's bound; a borrowed view has none and hosts no literal). A
/// domain on the field must be a plain predicate domain; a whole-literal
/// store must also satisfy it from the literal's own bytes.
pub(super) fn field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    carrier_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    capacity: Option<u64>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let destination_parameter_position = destination.root.parameter_position()?;
    let plain_domain = |symbol, literal_grant: bool| {
        program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .is_some_and(|domain| {
                domain.establishment_routes.is_empty()
                    && domain.semantic_roles == Default::default()
                    && (!literal_grant
                        || crate::facts::field_domain::string_literal_expression_grants_domain(
                            program,
                            assignment.value,
                            symbol,
                        ))
            })
    };
    let domains =
        crate::facts::field_domain::domain_constraint_symbols(program, field.type_reference);
    if let Some(index) = destination.dynamic_index {
        if !domains
            .into_iter()
            .all(|symbol| plain_domain(symbol, false))
        {
            return None;
        }
        return Some(
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(
                checked_trees::CheckedStructuralByteSequenceFieldByteStorePlan {
                    statement_index,
                    destination_parameter_position,
                    carrier_path,
                    field_identity: terminal_field_identity(program, field.symbol)?,
                    index: value::runtime_index(facts, state, statement_index, index)?,
                    value: value::byte_value(
                        program,
                        facts,
                        machine,
                        state,
                        &destination.root,
                        statement_index,
                        assignment,
                        source,
                        trace,
                    )?,
                },
            ),
        );
    }
    let (AssignmentSource::Authored, Some(capacity)) = (source, capacity) else {
        return None;
    };
    let ExpressionNode::String(bytes) = program.expression_table.expression(assignment.value)
    else {
        return None;
    };
    if u64::try_from(bytes.len()).ok()? > capacity
        || !domains.into_iter().all(|symbol| plain_domain(symbol, true))
    {
        return None;
    }
    Some(
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(
            checked_trees::CheckedStructuralByteSequenceFieldStorePlan {
                statement_index,
                destination_parameter_position,
                carrier_path,
                field_identity: terminal_field_identity(program, field.symbol)?,
                bytes: bytes.to_vec(),
            },
        ),
    )
}

/// Whether the field's declared type passes through an exclusive `&mut`
/// reference. The byte-sequence carrier collapses `&'r [u8]` and `&'r mut
/// [u8]` to the same `BorrowedView`, so the access distinction is read here
/// while the Reference node still exists.
pub(super) fn field_view_is_mutable(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { access, .. } => {
                return *access == language_semantics::ReferenceAccess::Mutable;
            }
            _ => return false,
        }
    }
}
