//! Structural block-parameter binding predicates shared by candidate
//! replays.
//!
//! A `StructuralPlaceKind::BlockParameter` place holds no producer of its
//! own: it is bound by every incoming edge's `structural_bindings` row.
//! When every incoming edge binds the parameter to the same place through
//! a whole `Owned`/`SharedBorrow` argument, and nothing in the function
//! can rewrite or vacate the parameter's contents or re-lend them with
//! write authority, the parameter's contents are exactly the bound place's
//! for the block's lifetime — an owned binding transfers the storage
//! itself and a shared loan freezes the bound place's contents for the
//! borrow. Replays use this to re-derive the establishment a
//! parameter-observing row claims, rather than trusting the row.

use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralArgument;

/// The one place `place` is bound to by every incoming edge when `place`
/// is a structural block parameter, or `None` when it is not a block
/// parameter, when any incoming edge fails to bind it or binds a different
/// place, when any bound argument is pathed or carries write authority, or
/// when the parameter's contents may still be rewritten or mutably re-lent
/// inside `function`.
pub(crate) fn bound_place(function: &PsiOptimizationFunction, place: PlaceId) -> Option<PlaceId> {
    optimization_unit::bound_place(function, place, &operation_rewrites_place)
}

/// Whether `operation` writes or vacates `place`, or re-lends it with
/// authority another party can write through.
fn operation_rewrites_place(operation: &O, place: PlaceId) -> bool {
    if match operation {
        // Bare-place destinations that replace or re-establish contents.
        O::PrimitiveLocalStore { destination, .. }
        | abstract_operations::byte_sequence_write_pair_pattern!(destination, ..)
        | abstract_operations::byte_sequence_store_pair_pattern!( destination, .. )
        | O::EstablishElementView { destination, .. } => *destination == place,
        // Parameter-row destinations and vacating move sources.
        O::WriteOnlyPrimitiveStore { destination, .. } => destination.place() == place,
        O::StoreStructuralField { destination, .. } => destination.place() == place,
        O::StructuralScalarFieldStore { destination, .. } => destination.place() == place,
        O::MoveStructuralField { source, .. } => source.place() == place,
        // An atomic resident's contents may change under another event.
        O::AtomicEvent { event, .. } => event.place() == Some(place),
        _ => false,
    } {
        return true;
    }
    structural_argument_operands(operation)
        .any(|argument| optimization_unit::argument_rewrites_place(argument, place))
}

/// Every structural-argument operand `operation` carries: record children
/// stored whole, element-view and reference sources, a reseating store's
/// delivered value, and call argument rows.
fn structural_argument_operands(operation: &O) -> impl Iterator<Item = &StructuralArgument> {
    let mut operands: Vec<&StructuralArgument> = Vec::new();
    match operation {
        O::EstablishStructuralArray { elements, .. } => operands.extend(elements),
        O::EstablishRecord { fields, .. } => {
            for initializer in fields {
                if let terminal_psi::RecordFieldValue::Structural(argument) = &initializer.value {
                    operands.push(argument);
                }
            }
        }
        O::EstablishElementView { source, .. } | O::EstablishReference { source, .. } => {
            operands.push(source);
        }
        O::StoreStructuralField { value, .. } => operands.push(value),
        abstract_operations::call_structural_or_boundary_call_pattern!(structural_arguments) => operands.extend(structural_arguments.iter()),
        _ => {}
    }
    operands.into_iter()
}
