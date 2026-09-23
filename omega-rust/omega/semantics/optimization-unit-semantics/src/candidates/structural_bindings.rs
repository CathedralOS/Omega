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

use crate::O;
use crate::PlaceId;
use crate::PsiOptimizationFunction;
use crate::StructuralPlaceKind;
use terminal_psi::{StructuralAccess, StructuralArgument};

/// The one place `place` is bound to by every incoming edge when `place`
/// is a structural block parameter, or `None` when it is not a block
/// parameter, when any incoming edge fails to bind it or binds a different
/// place, when any bound argument is pathed or carries write authority, or
/// when the parameter's contents may still be rewritten or mutably re-lent
/// inside `function`.
pub(crate) fn bound_place(function: &PsiOptimizationFunction, place: PlaceId) -> Option<PlaceId> {
    let StructuralPlaceKind::BlockParameter { block, .. } = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?
        .kind
    else {
        return None;
    };
    if place_is_rewritten(function, place) {
        return None;
    }
    let mut bound = None;
    for edge in function
        .blocks
        .iter()
        .flat_map(|owner| owner.nodes.iter())
        .flat_map(|node| node.successors.iter())
        .filter(|edge| edge.target == block)
    {
        let binding = edge
            .structural_bindings
            .iter()
            .find(|binding| binding.parameter == place)?;
        let argument = &binding.argument;
        if !argument.path.is_empty()
            || !matches!(
                argument.access,
                StructuralAccess::Owned | StructuralAccess::SharedBorrow
            )
        {
            return None;
        }
        match bound {
            None => bound = Some(argument.place),
            Some(seen) if seen != argument.place => return None,
            _ => {}
        }
    }
    bound
}

/// Whether `place`'s contents can change inside `function` after its
/// binding: a store or vacating move naming it, a mutable or pathed
/// structural argument re-lending it, or an atomic event joining its
/// residence each break the uniform-binding proof.
fn place_is_rewritten(function: &PsiOptimizationFunction, place: PlaceId) -> bool {
    function.blocks.iter().any(|block| {
        block.nodes.iter().any(|node| {
            operation_rewrites_place(&node.operation, place)
                || node.successors.iter().any(|edge| {
                    edge.structural_bindings
                        .iter()
                        .any(|binding| argument_rewrites_place(&binding.argument, place))
                })
        })
    })
}

/// Whether `operation` writes or vacates `place`, or re-lends it with
/// authority another party can write through.
fn operation_rewrites_place(operation: &O, place: PlaceId) -> bool {
    if match operation {
        // Bare-place destinations that replace or re-establish contents.
        O::PrimitiveLocalStore { destination, .. }
        | O::ByteSequenceWrite { destination, .. }
        | O::StructuralByteSequenceFieldByteStore { destination, .. }
        | O::StructuralByteSequenceFieldStore { destination, .. }
        | O::EstablishElementView { destination, .. } => *destination == place,
        // Parameter-row destinations and vacating move sources.
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::WriteOnlyIndexedPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. }
        | O::StoreStructuralField { destination, .. } => destination.place == place,
        O::MoveStructuralField { source, .. } => source.place == place,
        // The stored descriptor's selection source is the written aggregate:
        // whatever the argument's access, a field beneath it is established.
        O::StoreDynamicDescriptor { stored, .. } => stored.selection.source.place == place,
        // An atomic resident's contents may change under another event.
        O::AtomicEvent { event, .. } => event.place() == Some(place),
        _ => false,
    } {
        return true;
    }
    structural_argument_operands(operation).any(|argument| argument_rewrites_place(argument, place))
}

/// Every structural-argument operand `operation` carries: record children
/// stored whole, element-view and reference sources, a reseating store's
/// delivered value, call argument rows, and the conformance-selection
/// sources dynamic dispatches and stored descriptors retain.
fn structural_argument_operands(operation: &O) -> impl Iterator<Item = &StructuralArgument> {
    let mut operands: Vec<&StructuralArgument> = Vec::new();
    match operation {
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
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallUnitWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalarWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => operands.extend(structural_arguments.iter()),
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | O::CallDynamicUnit {
            dynamic_dispatch, ..
        } => {
            operands.push(&dynamic_dispatch.initial.source);
            operands.push(&dynamic_dispatch.rebound.source);
        }
        O::CallStoredDynamicScalar {
            dynamic_dispatch, ..
        } => operands.push(&dynamic_dispatch.stored.selection.source),
        _ => {}
    }
    operands.into_iter()
}

/// Whether `argument` gives `place`'s contents away: a pathed argument
/// projects or vacates only a subtree, while a whole `MutableBorrow` or
/// `WriteOnlyBorrow` argument lends write authority over the whole place.
/// Whole `Owned`/`SharedBorrow` arguments preserve the observed contents —
/// a move transfers the storage itself and a shared loan freezes it.
fn argument_rewrites_place(argument: &StructuralArgument, place: PlaceId) -> bool {
    argument.place == place
        && (!argument.path.is_empty()
            || matches!(
                argument.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ))
}
