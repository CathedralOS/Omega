//! Preserve an authored state's exclusive aggregate inputs across private control.
//!
//! Scalar evaluation and guards may split a state into several blocks. An
//! exclusive block parameter is an edge-scoped name, not a dominating storage
//! home: every successor needing it must receive the loan and use its fresh name.
//! Do this after the state's complete graph is assembled, so all producers use
//! the same transfer rule. Invocation receivers and shared views retain their
//! existing custody; independent Terminal verification checks every new edge.
//! Byte and element views keep their dominating descriptor names until an
//! explicit transfer consumes them. They do not have record loans' implicit
//! end-at-edge rule and must not acquire extra forwarding here.
//! Fixed arrays share record loans' edge-scoped names: indexing in a later
//! short-circuit block must use that block's forwarded name, regardless of
//! whether the backing was initialized or zero-constructed.
//!
//! Both state-graph producers split authored states this way, so both call
//! this one transfer rule: the Unit state graph seals each state's blocks
//! through it (`unit/attached_unit/composed_control/state_graph/emission/seal.rs`),
//! and a scalar graph applies it to each authored state's private region
//! once the whole machine is emitted (`scalar_graph/scalar_graph_module/state_borrows.rs`).

use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, place_id};
use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
use terminal_psi::{
    Block, StructuralAccess, StructuralArgument, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction, Terminator,
};

#[cfg(test)]
mod tests;

pub(crate) fn forward_private_state_borrows(
    root: BlockId,
    blocks: &mut [Block],
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
    places: &mut Vec<StructuralPlaceDeclaration>,
) -> Result<(), LoweringError> {
    let source = blocks
        .iter()
        .find(|block| block.id == root)
        .ok_or(LoweringError::Unsupported(
            "state borrow transport has no root",
        ))?
        .structural_parameters
        .iter()
        .filter(|parameter| {
            parameter.access == StructuralAccess::MutableBorrow
                // An explicitly carried name already has its custody path.
                // Terminal keeps that source available across intermediate
                // blocks until the transfer consumes it; do not invent a
                // second loan alongside existing projected or whole loans.
                && !has_explicit_transfer(blocks, parameter.place)
                && structural_types.iter().any(|declaration| {
                    declaration.id == parameter.structural_type
                        && matches!(declaration.shape,
                            StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. })
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    if source.is_empty() || blocks.len() == 1 {
        return Ok(());
    }
    // Carry only loans read in this block or a private successor. Forwarding
    // every root would prolong dead loans and could conflict with an existing
    // edge that delegates a projection to its own parameter.
    let mut needed = vec![vec![false; source.len()]; blocks.len()];
    let mut successors = Vec::with_capacity(blocks.len());
    let identities = blocks.iter().map(|block| block.id).collect::<Vec<_>>();
    for (position, block) in blocks.iter_mut().enumerate() {
        let mut observe = |place| {
            if let Some(ordinal) = source.iter().position(|parameter| parameter.place == place) {
                needed[position][ordinal] = true;
            }
            place
        };
        for operation in &mut block.operations {
            operation.kind.map_structural_uses(&mut observe);
        }
        let targets = match &block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    observe(argument.place);
                }
                vec![*target]
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    for argument in &edge.structural_arguments {
                        observe(argument.place);
                    }
                }
                vec![when_true.target, when_false.target]
            }
            Terminator::StructuralCase { source, cases } => {
                observe(*source);
                cases.iter().map(|case| case.target).collect()
            }
            Terminator::ReturnStructural { source, .. } => {
                observe(*source);
                Vec::new()
            }
            _ => Vec::new(),
        };
        successors.push(
            targets
                .into_iter()
                .filter(|target| *target != root)
                .filter_map(|target| identities.iter().position(|identity| *identity == target))
                .collect::<Vec<_>>(),
        );
    }
    loop {
        let mut changed = false;
        for (position, targets) in successors.iter().enumerate() {
            for target in targets {
                for ordinal in 0..source.len() {
                    if needed[*target][ordinal] && !needed[position][ordinal] {
                        needed[position][ordinal] = true;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut namespaces = Vec::with_capacity(blocks.len());
    for (position, block) in blocks.iter_mut().enumerate() {
        let mut names = Vec::with_capacity(source.len());
        for (ordinal, parameter) in source.iter().enumerate() {
            if !needed[position][ordinal] {
                continue;
            }
            let place = if block.id == root {
                parameter.place
            } else {
                let mut forwarded = parameter.clone();
                forwarded.place = place_id(allocate_dense(next_place)?);
                forwarded.position = u32::try_from(block.structural_parameters.len())
                    .map_err(|_| LoweringError::Unsupported("private borrow roster exceeds u32"))?;
                places.push(StructuralPlaceDeclaration {
                    id: forwarded.place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: block.id,
                        position: forwarded.position,
                    },
                });
                let place = forwarded.place;
                block.structural_parameters.push(forwarded);
                place
            };
            names.push((parameter.place, place));
        }
        namespaces.push((block.id, names));
    }
    for block in blocks {
        let names = &namespaces
            .iter()
            .find(|(identity, _)| *identity == block.id)
            .ok_or(LoweringError::Unsupported(
                "private borrow namespace is absent",
            ))?
            .1;
        let mut remap = |place| mapped(names, place);
        for operation in &mut block.operations {
            operation.kind.map_structural_uses(&mut remap);
        }
        let forward =
            |target: BlockId, arguments: &mut Vec<StructuralArgument>| {
                for argument in arguments.iter_mut() {
                    argument.place = mapped(names, argument.place);
                }
                if target != root
                    && let Some((_, destination_names)) =
                        namespaces.iter().find(|(identity, _)| *identity == target)
                {
                    arguments.extend(destination_names.iter().map(|(source, _)| {
                        StructuralArgument {
                            place: mapped(names, *source),
                            path: Vec::new(),
                            access: StructuralAccess::MutableBorrow,
                        }
                    }));
                }
            };
        match &mut block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                forward(*target, structural_arguments);
                remap_roots(trivial_affine_discards, names);
                for discard in residual_affine_discards {
                    discard.place = mapped(names, discard.place);
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    forward(edge.target, &mut edge.structural_arguments);
                    remap_roots(&mut edge.trivial_affine_discards, names);
                }
            }
            Terminator::ReturnStructural {
                source,
                trivial_affine_discards,
                ..
            } => {
                *source = mapped(names, *source);
                remap_roots(trivial_affine_discards, names);
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => {
                remap_roots(trivial_affine_discards, names);
            }
            Terminator::ReturnUnitPartialAffine {
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                remap_roots(trivial_affine_discards, names);
                for discard in residual_affine_discards {
                    discard.place = mapped(names, discard.place);
                }
            }
            Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                for cleanup in cleanups {
                    cleanup.place = mapped(names, cleanup.place);
                }
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
                for cleanup in cleanup_actions {
                    match cleanup {
                        TerminalAffineCleanupAction::DiscardRoot(place) => {
                            *place = mapped(names, *place);
                        }
                        TerminalAffineCleanupAction::DiscardResidual(discard) => {
                            discard.place = mapped(names, discard.place);
                        }
                        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                            cleanup.place = mapped(names, cleanup.place);
                        }
                    }
                }
            }
            Terminator::StructuralCase { source, cases } => {
                *source = mapped(names, *source);
                for case in cases {
                    if namespaces
                        .iter()
                        .any(|(identity, names)| *identity == case.target && !names.is_empty())
                    {
                        return unsupported(
                            "private case dispatch needs explicit exclusive state-borrow forwarding",
                        );
                    }
                    remap_roots(&mut case.trivial_affine_discards, names);
                }
            }
            // Guards and proof propositions keep their original storage
            // identities. Edge aliases establish the fresh runtime names;
            // no operation or proof result is manufactured by this transfer.
            Terminator::Crash { .. } => {}
        }
    }
    Ok(())
}

fn mapped(names: &[(PlaceId, PlaceId)], place: PlaceId) -> PlaceId {
    names
        .iter()
        .find(|(source, _)| *source == place)
        .map_or(place, |(_, target)| *target)
}

fn has_explicit_transfer(blocks: &[Block], place: PlaceId) -> bool {
    blocks.iter().any(|block| match &block.terminator {
        Terminator::Jump {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .any(|argument| argument.place == place),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false].iter().any(|edge| {
            edge.structural_arguments
                .iter()
                .any(|argument| argument.place == place)
        }),
        _ => false,
    })
}

fn remap_roots(places: &mut [PlaceId], names: &[(PlaceId, PlaceId)]) {
    for place in places {
        *place = mapped(names, *place);
    }
}
