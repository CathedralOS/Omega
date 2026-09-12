//! Exact block-local parameter declarations and simultaneous edge bindings.

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
                || (declaration.access == StructuralAccess::Owned
                    && terminal_semantics::scalar_array_leaf_shape(
                        module.structural_types.iter(),
                        declaration.structural_type,
                    )
                    .is_some())
                || parameter(machine, declaration.place) != Some(declaration)
                || !matches!(
                    (declaration.access, declaration.multiplicity),
                    (
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow,
                        StructuralMultiplicity::Unrestricted
                    ) | (
                        StructuralAccess::Owned,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                )
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
                        && (declaration.access == StructuralAccess::Owned
                            || matches!(
                                row.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView
                                )
                            ))
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

/// Return permission follows the owned binding, not the syntax of its producer.
/// Control-flow validation must establish dominance and the frontier pass must
/// consume any affine obligation before using this claim-free classification.
pub(super) fn plain_owned_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    parameter(machine, source).is_some_and(|parameter| {
        parameter.access == StructuralAccess::Owned
            && matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            )
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty()
            && super::structural_result_contracts::has_plain_owned_shape(
                module,
                parameter.structural_type,
            )
    })
}

pub(super) fn validate_successor(
    module: &TerminalModule,
    machine: &TerminalMachine,
    edge: EdgeId,
    target: &terminal_psi::Block,
    arguments: &[StructuralArgument],
    available: &BTreeSet<PlaceId>,
    dominating_blocks: &BTreeSet<BlockId>,
) -> Result<(), ModuleError> {
    if arguments.len() != target.structural_parameters.len() {
        return Err(ModuleError::StructuralJumpArityMismatch {
            edge,
            expected: target.structural_parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut exclusive = BTreeSet::new();
    for (argument, expected) in arguments.iter().zip(&target.structural_parameters) {
        if expected.access == StructuralAccess::MutableBorrow
            && (!available.contains(&argument.place) || !exclusive.insert(argument.place))
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
        }
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
        } else if expected.access == StructuralAccess::Owned {
            // Whole plain records and sums use the same owned parameter move.
            // The constructor establishes its own exact type; a successor may
            // not substitute the selected result's type for an untouched owner.
            (super::scalar_case::plain_return_source(module, machine, argument.place)
                || super::record::plain_return_source(module, machine, argument.place))
                && machine.blocks.iter().any(|block| {
                    dominating_blocks.contains(&block.id)
                        && block.operations.iter().any(|operation| {
                            operation.result.structural().is_some_and(|result| {
                                result.place == argument.place
                                    && result.structural_type == expected.structural_type
                                    && result.multiplicity == expected.multiplicity
                                    && result.qualifications == expected.qualifications
                                    && result.projected_qualifications
                                        == expected.projected_qualifications
                            })
                        })
                })
        } else {
            expected.access == StructuralAccess::SharedBorrow
                && expected.multiplicity == StructuralMultiplicity::Unrestricted
                && available.contains(&argument.place)
                && super::byte_sequence_subslice::borrowed_result(machine, argument.place)
                    .is_some_and(|source| source.structural_type == expected.structural_type)
        };
        if !argument.path.is_empty() || argument.access != expected.access || !exact_source {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
        }
    }
    Ok(())
}

/// Mutable view names transfer at an edge; ordinary calls only reborrow them.
/// Intersect all actual arrivals, including backedges, so an old source name
/// cannot supply a second exclusive alias after a differently named transfer.
pub(super) fn mutable_availability(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> BTreeMap<BlockId, BTreeSet<PlaceId>> {
    let mutable_view = |parameter: &StructuralParameterDeclaration| {
        parameter.access == StructuralAccess::MutableBorrow
            && module.structural_types.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && declaration.shape
                        == StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView,
                        )
            })
    };
    let universe = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| mutable_view(parameter))
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    if universe.is_empty() {
        return BTreeMap::new();
    }
    let initial = machine
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .filter(|place| universe.contains(place))
        .collect::<BTreeSet<_>>();
    let edges = machine
        .blocks
        .iter()
        .flat_map(|block| {
            let successors: Vec<(BlockId, &[StructuralArgument])> = match &block.terminator {
                Terminator::Jump {
                    target,
                    structural_arguments,
                    ..
                } => vec![(*target, structural_arguments)],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    (when_true.target, &when_true.structural_arguments),
                    (when_false.target, &when_false.structural_arguments),
                ],
                Terminator::StructuralCase { cases, .. } => {
                    cases.iter().map(|case| (case.target, &[][..])).collect()
                }
                _ => Vec::new(),
            };
            successors
                .into_iter()
                .map(move |(target, arguments)| (block.id, target, arguments))
        })
        .collect::<Vec<_>>();
    let mut available = machine
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                if block.id == machine.entry {
                    initial.clone()
                } else {
                    universe.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in &machine.blocks {
            let mut common = if block.id == machine.entry {
                initial.clone()
            } else {
                universe.clone()
            };
            for (source, _, arguments) in edges.iter().filter(|(_, target, _)| *target == block.id)
            {
                let mut incoming = available[source].clone();
                let transfers = arguments
                    .iter()
                    .zip(&block.structural_parameters)
                    .filter(|(_, parameter)| universe.contains(&parameter.place))
                    .map(|(argument, parameter)| (argument.place, parameter.place))
                    .collect::<Vec<_>>();
                for (source, _) in &transfers {
                    incoming.remove(source);
                }
                for (source_place, destination) in transfers {
                    if available[source].contains(&source_place) {
                        incoming.insert(destination);
                    }
                }
                common.retain(|place| incoming.contains(place));
            }
            if available[&block.id] != common {
                available.insert(block.id, common);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    available
}

pub(super) fn is_mutable_parameter(machine: &TerminalMachine, place: PlaceId) -> bool {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .or_else(|| parameter(machine, place))
        .is_some_and(|parameter| parameter.access == StructuralAccess::MutableBorrow)
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
