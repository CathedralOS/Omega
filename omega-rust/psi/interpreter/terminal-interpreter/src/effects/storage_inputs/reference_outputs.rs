//! Authenticate normal-completion reference outputs against this invocation's
//! captured inputs. Lifetime anchors bound retention; only an opaque selection
//! of a typed captured endpoint supplies an address. Every request is staged
//! before any input/result commit, so replacement cannot reread another output.

use super::contents::{ReferenceOutputWrite, StorageLeafKind, TerminalBoundaryReferenceSelection};
use super::{CapturedReferenceInput, StructuralLoanBinding, TerminalBoundaryStorageBuffer};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::record::storage::ConstructorStorage;
use std::sync::Arc;
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryMachineDeclaration, BoundaryReferenceDestination,
    StructuralAccess, StructuralPathSegment, StructuralTypeShape,
};

fn source<'captured>(
    selection: &TerminalBoundaryReferenceSelection,
    captures: &'captured [CapturedReferenceInput],
) -> Result<&'captured CapturedReferenceInput, TerminalInterpretError> {
    let mut matching = captures.iter().filter(|capture| {
        capture.argument_index == selection.argument_index
            && Arc::ptr_eq(&capture.identity, &selection.identity)
    });
    let selected = matching.next().ok_or_else(malformed)?;
    if matching.next().is_some() {
        return Err(malformed());
    }
    Ok(selected)
}

pub(super) fn stage(
    execution: &TerminalExecution,
    declaration: &BoundaryMachineDeclaration,
    buffers: &[TerminalBoundaryStorageBuffer],
    bindings: &mut [StructuralLoanBinding],
    captures: &[CapturedReferenceInput],
    identity_cursor: &mut Option<u64>,
) -> Result<(), TerminalInterpretError> {
    if declaration.reference_outputs.is_empty() {
        if buffers.iter().any(|buffer| matches!(buffer, TerminalBoundaryStorageBuffer::Structural(buffer) if !buffer.references.is_empty())) {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant("boundary has an undeclared reference output request"));
        }
        return Ok(());
    }
    let types = execution.program.structural_types();
    // Every published output needs a captured mutable destination, even when
    // the host supplied no request for it. Absence cannot erase an obligation.
    for output in &declaration.reference_outputs {
        let BoundaryReferenceDestination::Parameter(index) = output.destination;
        if bindings
            .iter()
            .filter(|binding| binding.argument_index == index as usize)
            .count()
            != 1
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "boundary reference output has no unique captured destination",
            ));
        }
    }
    let mut staged = Vec::new();
    let mut request_count = 0usize;
    for (binding_index, binding) in bindings.iter().enumerate() {
        let mut matching = buffers.iter().filter_map(|buffer| match buffer {
            TerminalBoundaryStorageBuffer::Structural(buffer)
                if buffer.argument_index == binding.argument_index
                    && Arc::ptr_eq(&buffer.identity, &binding.identity) =>
            {
                Some(buffer)
            }
            _ => None,
        });
        let buffer = matching.next().ok_or_else(malformed)?;
        if matching.next().is_some() {
            return Err(malformed());
        }
        request_count = request_count
            .checked_add(buffer.references.len())
            .ok_or_else(malformed)?;
        let destination_index = u32::try_from(binding.argument_index).map_err(|_| malformed())?;
        let mut outputs = declaration
            .reference_outputs
            .iter()
            .filter(|output| {
                output.destination == BoundaryReferenceDestination::Parameter(destination_index)
            })
            .peekable();
        if outputs.peek().is_none() {
            continue;
        }
        let leaves = binding.completed_leaves.as_deref().ok_or_else(malformed)?;
        let cases = leaves
            .iter()
            .filter_map(|leaf| match &leaf.kind {
                StorageLeafKind::Case { case, .. } => Some((leaf, *case)),
                _ => None,
            })
            .map(|(leaf, case)| {
                terminal_semantics::canonical_structural_storage_path(
                    types.values(),
                    binding.referent.structural_type,
                    &leaf.path,
                )
                .map(|path| (path, case))
                .ok_or_else(malformed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for output in outputs {
            let endpoint = terminal_semantics::runtime_structural_path_tip(
                types.values(),
                binding.referent.structural_type,
                &output.path,
            )
            .ok_or_else(malformed)?;
            let StructuralTypeShape::Reference { access, .. } = endpoint.shape else {
                return Err(malformed());
            };
            if access != output.access || binding.access != StructuralAccess::MutableBorrow {
                return Err(malformed());
            }
            let active = crate::structural_inputs::case_membership::case_projection_with_membership_is_active(
                &types, binding.referent.structural_type, &output.path,
                &|prefix| cases.iter().find(|(path, _)| path == prefix).map(|(_, case)| *case),
            )?;
            if !active {
                continue;
            }
            let mut requests = buffer
                .references
                .iter()
                .filter(|request| request.path == output.path);
            let request = requests.next().ok_or_else(malformed)?;
            if requests.next().is_some() {
                return Err(malformed());
            }
            let selected = source(&request.source, captures)?;
            if request.source.access != output.access
                || !crate::reference::access_covers(selected.access, output.access)
            {
                return Err(malformed());
            }
            let relative = terminal_semantics::canonical_structural_storage_path(
                types.values(),
                selected.referent.structural_type,
                &request.source.path,
            )
            .ok_or_else(malformed)?;
            // Storage owned by the selected input must still contain the
            // requested case after its staged writes. The first Referent is
            // the boundary to independently captured pointee storage; ordinary
            // reference formation below authenticates those physical loads.
            if let Some(source_binding) = bindings
                .iter()
                .find(|binding| binding.argument_index == selected.argument_index)
            {
                let leaves = source_binding
                    .completed_leaves
                    .as_deref()
                    .ok_or_else(malformed)?;
                let prefix_end = relative
                    .iter()
                    .position(|step| matches!(step, StructuralPathSegment::Referent))
                    .unwrap_or(relative.len());
                let active = crate::structural_inputs::case_membership::case_projection_with_membership_is_active(
                    &types, selected.referent.structural_type, &relative[..prefix_end],
                    &|prefix| leaves.iter().find_map(|leaf| match &leaf.kind {
                        StorageLeafKind::Case { case, .. } => {
                            let path = terminal_semantics::canonical_structural_storage_path(types.values(), selected.referent.structural_type, &leaf.path)?;
                            (path == prefix).then_some(*case)
                        },
                        _ => None,
                    }),
                )?;
                if !active {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "reference output selects storage invalidated by staged input case replacement",
                    ));
                }
            }
            let mut anchor_matches = false;
            for dependency in &output.dependencies {
                // The declaration retains every anchor, not one convenient
                // alternative. Validate every actual argument and descriptor.
                let (argument_index, expected_access, path) = match dependency {
                    BoundaryLifetimeDependency::Referent {
                        argument_index,
                        access,
                    } => (*argument_index as usize, *access, None),
                    BoundaryLifetimeDependency::Stored {
                        argument_index,
                        path,
                        access,
                    } => (*argument_index as usize, *access, Some(path)),
                };
                let parameter = declaration
                    .structural_parameters
                    .get(argument_index)
                    .ok_or_else(malformed)?;
                let mut anchor_captures = captures
                    .iter()
                    .filter(|capture| capture.argument_index == argument_index);
                let capture = anchor_captures.next().ok_or_else(malformed)?;
                if anchor_captures.next().is_some() {
                    return Err(malformed());
                }
                if let Some(path) = path {
                    let tip = terminal_semantics::runtime_structural_path_tip(
                        types.values(),
                        capture.referent.structural_type,
                        path,
                    )
                    .ok_or_else(malformed)?;
                    if !matches!(tip.shape, StructuralTypeShape::Reference { access, .. } if crate::reference::access_covers(access, expected_access))
                    {
                        return Err(malformed());
                    }
                    let mut full = capture.source_argument.path.clone();
                    full.extend_from_slice(path);
                    execution.resolve_live_projection(
                        capture.source_argument.place,
                        &full,
                        StructuralAccess::SharedBorrow,
                    )?;
                } else if capture.access == StructuralAccess::Owned
                    || !crate::reference::access_covers(parameter.access, expected_access)
                {
                    // Referent grants come from the outer formal. Stored
                    // descriptor ceilings were checked above, independently
                    // of the readable containing path; neither is the other.
                    return Err(malformed());
                }
                if argument_index == request.source.argument_index
                    && crate::reference::access_covers(expected_access, output.access)
                {
                    anchor_matches |= match (
                        relative
                            .iter()
                            .rposition(|step| matches!(step, StructuralPathSegment::Referent)),
                        path,
                    ) {
                        (None, None) => selected.access != StructuralAccess::Owned,
                        (Some(position), Some(anchor)) => {
                            &relative[..position] == anchor.as_slice()
                        }
                        _ => false,
                    };
                }
            }
            if !anchor_matches {
                return Err(malformed());
            }
            let mut argument = selected.source_argument.clone();
            argument.path.extend(relative);
            argument.access = output.access;
            // Empty requested qualifications means this capture cannot mint a
            // predicate merely because the output slot asks for it. Existing
            // parameter/field qualification admission remains mandatory.
            let captured =
                execution.capture_reference(&argument, endpoint.id, &[], None, identity_cursor)?;
            let mut leaves = leaves.iter().filter_map(|leaf| match &leaf.kind {
                StorageLeafKind::Opaque {
                    destination,
                    declaration,
                } if *declaration == endpoint.id => {
                    let path = terminal_semantics::canonical_structural_storage_path(
                        types.values(),
                        binding.referent.structural_type,
                        &leaf.path,
                    )?;
                    (path == output.path).then_some(destination)
                }
                _ => None,
            });
            let destination = leaves.next().ok_or_else(malformed)?.clone();
            if leaves.next().is_some() {
                return Err(malformed());
            }
            if request.write == ReferenceOutputWrite::Address {
                let previous = execution
                    .reference_referents
                    .get(&destination)
                    .ok_or_else(malformed)?;
                // Scalar references carry their exact selected storage codec,
                // including IntegerAt/recast views. Logical referent equality
                // cannot justify preserving that word after an address write.
                let stored_scalar = |reference: &crate::reference::ReferenceReferent| {
                    reference
                        .primitive_loan
                        .as_ref()
                        .map(|loan| loan.storage.storage_scalar().unwrap_or(loan.scalar_type))
                };
                let StructuralTypeShape::Reference { referent, .. } = endpoint.shape else {
                    return Err(malformed());
                };
                let primitive = matches!(
                    types.get(&referent).ok_or_else(malformed)?.shape,
                    StructuralTypeShape::PrimitiveScalar(_)
                );
                let old_scalar = stored_scalar(previous);
                let new_scalar = stored_scalar(&captured.referent);
                if old_scalar != new_scalar || (primitive && old_scalar.is_none()) {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "address-only reference output changes its captured scalar storage interpretation",
                    ));
                }
            }
            staged.push((binding_index, destination, captured.referent));
        }
    }
    if request_count != staged.len() {
        return Err(malformed());
    }
    // No reference capture sees a write from this invocation. The ordinary
    // completion owner commits these only after all other input validation.
    for (index, destination, reference) in staged {
        bindings[index]
            .reference_replacement
            .get_or_insert_with(|| ConstructorStorage::new(execution.account.as_ref()))
            .stage_reference(destination, reference)?;
    }
    Ok(())
}
