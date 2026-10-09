//! Descriptor payload homes do not allocate their backing. Reconstruct the
//! completed calls reached by a payload access, then retain an opaque read of
//! those calls' authenticated origin rosters. Callee-local scalar coordinates
//! cannot bound this reach in the caller; consumers conservatively preserve it.
use crate::SelectedInstructionError;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind as I, LegalizedScalarTerminator as T,
};
use selected_instructions::{SelectedMemoryAccess, SelectedMemoryAccessRole as R};
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
use std::collections::BTreeSet;

pub(super) fn replay_memory_origins(
    source: &LegalizedScalarFunction,
    accesses: &mut Vec<SelectedMemoryAccess>,
) -> Result<(), SelectedInstructionError> {
    let mut replayed = Vec::with_capacity(accesses.len());
    for access in accesses.iter() {
        if !matches!(
            access.role,
            R::ReadByteSequence { .. }
                | R::ReadByteSpan { .. }
                | R::ReadElementView { .. }
                | R::ReadIndexedPrimitive { .. }
        ) {
            replayed.push(access.clone());
            continue;
        }
        let mut producers = BTreeSet::new();
        collect_calls(source, access.place, &mut BTreeSet::new(), &mut producers)?;
        if producers.is_empty() {
            replayed.push(access.clone());
        } else {
            // The original read's bounds still validate its physical operation.
            // Its access row must not turn descriptor identity into a separate
            // backing allocation or reuse the shortened view extent at root 0.
            for (producer, place) in producers {
                let mut row = access.clone();
                row.place = place;
                row.byte_offset = 0;
                row.byte_count = 0;
                row.role = R::ReadReturnedView { producer };
                replayed.push(row);
            }
        }
    }
    *accesses = replayed;
    Ok(())
}

fn collect_calls(
    source: &LegalizedScalarFunction,
    place: PlaceId,
    visited: &mut BTreeSet<PlaceId>,
    calls: &mut BTreeSet<(OperationId, PlaceId)>,
) -> Result<(), SelectedInstructionError> {
    if !visited.insert(place) {
        return Ok(());
    }
    if let Some((producer, _)) = super::established_view_input::descriptor_result(source, place) {
        calls.insert((producer, place));
        return Ok(());
    }
    let Some(signature) = &source.structural else {
        return Ok(());
    };
    let Some(declaration) = signature.structural_place(place) else {
        return Ok(());
    };
    match declaration.kind {
        StructuralPlaceKind::OperationResult { producer, .. } => {
            let row = source
                .instructions()
                .find(|row| row.operation == producer)
                .ok_or(SelectedInstructionError::custody())?;
            match &row.kind {
                I::ByteSequenceSubslice {
                    source: parent,
                    indices,
                    ..
                }
                | I::ElementViewSubslice {
                    source: parent,
                    indices,
                    ..
                } => {
                    // Origin accounting is opaque, not a backing bound or
                    // permission. First retain the exact selected descriptor
                    // coordinate and its own selector certificates; then account
                    // conservatively for the parent home's completed calls.
                    let root = super::reference_input::root_type(source, parent.place)
                        .ok_or(SelectedInstructionError::custody())?;
                    let geometry = crate::structural_inputs::structural_reference_input::descriptor_source_geometry(
                        root, &parent.path, &signature.structural_types,
                    ).ok_or(SelectedInstructionError::custody())?;
                    if !geometry.readable || !crate::structural_inputs::structural_reference_input::runtime_indices_match(indices, &geometry.elements) {
                        return Err(SelectedInstructionError::custody());
                    }
                    collect_calls(source, parent.place, visited, calls)?;
                }
                I::StructuralLeafCopy { source: parent, .. } => {
                    collect_calls(source, *parent, visited, calls)?;
                }
                I::EstablishElementView { source: parent, .. } => {
                    collect_calls(source, parent.place, visited, calls)?
                }
                // Any other producer owns its storage. A call with descriptor
                // type must have passed the exact result/origin query above.
                I::Call(call)
                    if call.structural_result.as_ref().is_some_and(|result| {
                        result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                            && terminal_semantics::descriptor_results::is_shared_descriptor(
                                signature.structural_types.iter(),
                                result.structural_type,
                            )
                    }) =>
                {
                    return Err(SelectedInstructionError::custody());
                }
                _ => {}
            }
        }
        StructuralPlaceKind::BlockParameter { block, .. } => {
            for owner in &source.blocks {
                let mut follow = |successor: &legalized_operations::LegalizedScalarSuccessor| -> Result<(), SelectedInstructionError> {
                    if successor.target == block {
                        let binding = successor.structural_binding(place)
                            .ok_or(SelectedInstructionError::custody())?;
                        collect_calls(source, binding.argument.place, visited, calls)?;
                    }
                    Ok(())
                };
                match &owner.terminator {
                    T::Jump { successor, .. } => follow(successor)?,
                    T::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        follow(when_true)?;
                        follow(when_false)?;
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}
