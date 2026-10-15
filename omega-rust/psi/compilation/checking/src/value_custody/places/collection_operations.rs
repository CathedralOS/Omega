//! Authored collection measures, identified by their member shape and
//! receiver type rather than spelling alone. Preparation and checking
//! share these queries; checked consumers use the published selection.

use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The receiver a collection measure is asked of.
///
/// A range-indexed window (`xs[a..b]`) is a slice, but the immutable typed
/// table stores no type-reference row for a window, so a caller which
/// inferred one names it as such instead of inventing a handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureReceiver {
    Declared(TypeReferenceHandle),
    Window,
}

/// The compiler-owned collection measure an authored member selects, or
/// `None` when the member is not a measure.
///
/// A collection measure is authored as a bare member spelling on a carrier
/// (`cells.len`, `buffer.capacity`). The language owns the measure, no
/// package declares it, and the result is the carrier's value metadata
/// rather than stored content. The spelling alone never establishes that: it
/// is compared through [`CollectionMeasure::from_authored_spelling`], and the
/// receiver decides. Each condition rejects a different member which happens
/// to share the name:
///
/// * the receiver must be a fixed array or slice (through references and
///   constraint shells) or a range-indexed window. A record field spelled
///   `len` is that record's declared field, so evidence about a collection
///   extent would not describe it.
/// * the member must carry no case variant. A case projection selects one
///   variant's payload field, and no variant carries a measure.
///
/// Checking records the measure once as its intrinsic selection target
/// ([`CollectionMeasure::intrinsic`]); consumers holding the selection ledger
/// read the retained identity instead of asking here. A consumer holding a
/// spelling but no receiver type asks the map directly and keeps its own
/// narrowing visible at its site.
pub fn collection_measure_member(
    program: &dyn crate::CheckingInput,
    member: &typed_trees::expression::TableMemberExpression,
    receiver: MeasureReceiver,
) -> Option<CollectionMeasure> {
    if member.case_variant.is_some() {
        return None;
    }
    let receiver_is_collection = match receiver {
        MeasureReceiver::Declared(type_reference) => {
            type_reference_is_collection(program, type_reference)
        }
        MeasureReceiver::Window => true,
    };
    if !receiver_is_collection {
        return None;
    }
    CollectionMeasure::from_authored_spelling(member.member.as_str())
}

/// Whether a declared type reference names a collection carrier: a fixed
/// array or slice, reached through references and constraint shells.
pub fn type_reference_is_collection(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_is_collection(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_collection(program, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => true,
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => false,
    }
}
