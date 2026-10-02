//! A shared borrowed-view field leaf: `place.field = "literal"` replaces the
//! field's descriptor with the literal's view of immutable image storage.
//!
//! A stored view must not outlive its backing
//! (wiki/spec/language/lifetimes.md). Image storage outlives every root, so a
//! quoted literal is the source this store admits; a parameter view, a
//! subslice or another field's view supplies no relation to the destination
//! root here and is declined. Each declared domain on the field must be a
//! plain byte-predicate domain the literal's own bytes grant. Terminal
//! replays both judgments independently (`StructuralByteViewFieldStore`).
use super::super::{ExpressionNode, TypedTrees};
use super::destination::Destination;
use super::value::AssignmentSource;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::terminal_field_identity;

/// Plan the descriptor replacement of one shared view field, or decline.
pub(super) fn literal_store(
    program: &TypedTrees,
    destination: &Destination<'_>,
    carrier_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedUnitEffectOperationPlan> {
    trace.phase("structural field store: borrowed view field: literal source");
    if !matches!(source, AssignmentSource::Authored) || destination.dynamic_index.is_some() {
        return None;
    }
    trace.phase("structural field store: borrowed view field: declared domains");
    member_store(
        program,
        destination,
        carrier_path,
        field,
        statement_index,
        assignment.value,
    )
}

/// One shared view member of a record literal, or a whole view field store:
/// the value expression must be a quoted literal whose bytes grant every
/// declared plain byte-predicate domain of the field.
pub(super) fn member_store(
    program: &TypedTrees,
    destination: &Destination<'_>,
    carrier_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    statement_index: u32,
    value: typed_trees::expression::ExpressionHandle,
) -> Option<checked_trees::CheckedUnitEffectOperationPlan> {
    let ExpressionNode::String(bytes) = program.expression_table.expression(value) else {
        return None;
    };
    if !crate::facts::field_domain::string_literal_grants_view_domains(
        program,
        field.type_reference,
        value,
    ) {
        return None;
    }
    let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
    value.root = value
        .nodes
        .insert(checked_trees::CheckedByteSequenceValue::Literal(
            bytes.to_vec(),
        ));
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
