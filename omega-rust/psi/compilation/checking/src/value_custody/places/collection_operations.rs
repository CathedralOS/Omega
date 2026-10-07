//! Authored collection operations, identified by their complete call shape
//! and receiver type rather than spelling alone. Preparation and checking
//! share these queries; checked consumers use the published selection.

use language_semantics::declaration_selection::{CollectionMeasure, CollectionViewOperation};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The compiler-owned collection or text view an authored call selects, or
/// `None` when the call is not a view.
///
/// A collection-view call is authored as a bare method spelling on a carrier
/// (`cells.as_slice()`, `text.as_view()`, `view.bytes()`). The language owns
/// the operation, no package declares it, and the result observes the
/// receiver's existing storage instead of producing a new value. The spelling
/// alone never establishes that: it is compared through
/// [`CollectionViewOperation::from_authored_spelling`], and the call shape
/// decides. Each condition rejects a different operation which happens to
/// share the name:
///
/// * `target_symbol` must be invalid. A resolved nominal machine spelled
///   `as_slice` is that machine's declared operation; its result may be any
///   projection of anything it can reach, so evidence taken from the receiver
///   would not describe it.
/// * `receiver` must be valid. A view needs the carrier it views.
/// * the call must carry no arguments, evidence arguments, machine arguments,
///   static requirement dispatch, quotient operation or private layout
///   operation. Every one of those names an operand or a dispatch decision
///   that a compiler-owned view does not accept, so a call carrying one is a
///   different operation regardless of how it is spelled.
///
/// The selected operation is returned rather than a boolean: a caller which
/// only claims element views of a collection (the ownership and projected
/// transfer lanes) matches on the exact operations it claims, keeping that
/// narrowing visible at its own site.
///
/// Checking records the operation once as
/// `AuthoredDeclarationSelectionIntrinsic::CollectionView`; consumers holding
/// the selection ledger read the retained identity instead of asking here.
pub fn collection_view_call(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<CollectionViewOperation> {
    if call.target_symbol.is_valid()
        || !call.receiver.is_valid()
        || !program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    CollectionViewOperation::from_authored_spelling(call.target.as_str())
}

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
