//! Exact block-local immutable view declarations and simultaneous edge bindings.

use super::*;

pub(super) fn parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
        return None;
    };
    machine
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .structural_parameters
        .get(position as usize)
        .filter(|parameter| {
            parameter.place == place && parameter.position == position && !parameter.is_self
        })
}

pub(super) fn validate_declarations(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), ModuleError> {
    for block in &machine.blocks {
        if block.id == machine.entry && !block.structural_parameters.is_empty() {
            return Err(ModuleError::EntryBlockCannotHaveParameters(block.id));
        }
        for (position, declaration) in block.structural_parameters.iter().enumerate() {
            if u32::try_from(position).ok() != Some(declaration.position)
                || parameter(machine, declaration.place) != Some(declaration)
                || declaration.access != StructuralAccess::SharedBorrow
                || declaration.multiplicity != StructuralMultiplicity::Unrestricted
                || !declaration.qualifications.is_empty()
                || !declaration.projected_qualifications.is_empty()
                || !machine.structural_places.iter().any(|place| {
                    place.id == declaration.place
                        && place.kind
                            == StructuralPlaceKind::BlockParameter {
                                block: block.id,
                                position: declaration.position,
                            }
                })
                || !module.structural_types.iter().any(|row| {
                    row.id == declaration.structural_type
                        && matches!(
                            row.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView
                            )
                        )
                })
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == declaration.place)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == declaration.place)
            {
                return Err(ModuleError::InvalidBlockStructuralParameter {
                    block: block.id,
                    place: declaration.place,
                });
            }
        }
    }
    for place in &machine.structural_places {
        if let StructuralPlaceKind::BlockParameter { block, .. } = place.kind
            && parameter(machine, place.id).is_none()
        {
            return Err(ModuleError::InvalidBlockStructuralParameter {
                block,
                place: place.id,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_successor(
    machine: &TerminalMachine,
    edge: EdgeId,
    target: &terminal_psi::Block,
    arguments: &[StructuralArgument],
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    if arguments.len() != target.structural_parameters.len() {
        return Err(ModuleError::StructuralJumpArityMismatch {
            edge,
            expected: target.structural_parameters.len(),
            actual: arguments.len(),
        });
    }
    for (argument, expected) in arguments.iter().zip(&target.structural_parameters) {
        let source = machine
            .structural_parameters
            .iter()
            .find(|row| row.place == argument.place)
            .or_else(|| {
                parameter(machine, argument.place).filter(|_| available.contains(&argument.place))
            });
        let exact_source = if let Some(source) = source {
            source.structural_type == expected.structural_type
                && source.access == expected.access
                && source.multiplicity == expected.multiplicity
                && source.qualifications == expected.qualifications
                && source.projected_qualifications == expected.projected_qualifications
        } else {
            available.contains(&argument.place)
                && super::byte_sequence_subslice::borrowed_result(machine, argument.place)
                    .is_some_and(|source| source.structural_type == expected.structural_type)
        };
        if !argument.path.is_empty()
            || argument.access != StructuralAccess::SharedBorrow
            || !exact_source
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
        }
    }
    Ok(())
}

/// Ranked countdown authorities still exclude descriptor successor bindings.
pub(super) fn has_bindings(machine: &TerminalMachine) -> bool {
    machine.blocks.iter().any(|block| {
        !block.structural_parameters.is_empty()
            || match &block.terminator {
                Terminator::Jump {
                    structural_arguments,
                    ..
                } => !structural_arguments.is_empty(),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    !when_true.structural_arguments.is_empty()
                        || !when_false.structural_arguments.is_empty()
                }
                _ => false,
            }
    })
}
