//! Complete scheduled disposal from the exact transferred paths on each edge.
//!
//! This pass never chooses a root's lifetime. It changes an already scheduled
//! whole discard into the maximal complement of its earlier owned transfers
//! and ended reference leaves. Storage remains available to scalar siblings.
//! Joins require identical transfer frontiers; a result birth starts a fresh
//! occurrence. Selected nominal subjects remain intact boundaries within the
//! same ordered complement. This pass neither invents hooks nor disposes a
//! transferred subtree to make the remaining partition fit.
use super::{LoweringError, block_position, unsupported};
use crate::direct_calls::call_structural_arguments;
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use std::collections::{BTreeSet, VecDeque};
use terminal_psi::{
    Block, OperationKind, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, Terminator,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct MovedRoot {
    place: PlaceId,
    paths: BTreeSet<Vec<StructuralPathSegment>>,
}
type Frontier = Vec<MovedRoot>;

/// Extraction moves cells without an explicit owned operand. Retain that
/// transfer once for both whole-home lifetime and residual completion. An owned
/// descriptor does not own its referent or view elements: only inline storage
/// selected from an owned declaration belongs to this home's complement.
pub(crate) fn owned_extractions(
    parameters: &[StructuralParameterDeclaration],
    local_places: &[StructuralPlaceDeclaration],
    blocks: &[Block],
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<(OperationId, StructuralArgument)>, LoweringError> {
    let roots = parameters
        .iter()
        .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
        .map(|parameter| (parameter.place, parameter.structural_type, parameter.access))
        .chain(
            blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| {
                    operation.result.structural().map(|result| {
                        (
                            result.place,
                            result.structural_type,
                            StructuralAccess::Owned,
                        )
                    })
                }),
        )
        .chain(local_places.iter().filter_map(|local| match local.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } => Some((local.id, structural_type, StructuralAccess::Owned)),
            _ => None,
        }))
        .collect::<Vec<_>>();
    let mut extractions = Vec::new();
    for operation in blocks.iter().flat_map(|block| &block.operations) {
        let OperationKind::MoveStructuralField {
            source,
            path,
            field,
        } = &operation.kind
        else {
            continue;
        };
        let mut declarations = roots.iter().filter(|(place, _, _)| place == source);
        let (_, root_type, access) = declarations.next().ok_or(LoweringError::Unsupported(
            "extraction source has no storage declaration",
        ))?;
        if declarations
            .any(|(_, other_type, other_access)| other_type != root_type || other_access != access)
        {
            return unsupported("extraction source has inconsistent storage declarations");
        }
        if *access != StructuralAccess::Owned || path.contains(&StructuralPathSegment::Referent) {
            continue;
        }
        if types.iter().any(|declaration| {
            declaration.id == *root_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                )
        }) {
            continue;
        }
        let projection = terminal_semantics::structural_window_projection(
            types.iter(),
            *root_type,
            path,
            *field,
        )
        .ok_or(LoweringError::Unsupported(
            "owned extraction has no exact storage projection",
        ))?;
        extractions.push((
            operation.id,
            StructuralArgument {
                place: *source,
                path: projection.path,
                access: StructuralAccess::Owned,
            },
        ));
    }
    Ok(extractions)
}

pub(super) fn complete(
    parameters: &[StructuralParameterDeclaration],
    local_places: &[StructuralPlaceDeclaration],
    entry: BlockId,
    blocks: &mut [Block],
    types: &[StructuralTypeDeclaration],
    extractions: &[(OperationId, StructuralArgument)],
) -> Result<(), LoweringError> {
    if partitioned_roots(blocks, extractions).is_empty() {
        return Ok(());
    }
    let roots = parameters
        .iter()
        .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
        .map(|parameter| (parameter.place, parameter.structural_type))
        .chain(
            blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| {
                    operation
                        .result
                        .structural()
                        .map(|result| (result.place, result.structural_type))
                }),
        )
        .chain(local_places.iter().filter_map(|local| match local.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } => Some((local.id, structural_type)),
            _ => None,
        }))
        .collect::<Vec<_>>();
    let mut incoming = vec![None; blocks.len()];
    let entry = block_position(blocks, entry)?;
    incoming[entry] = Some(Vec::new());
    let mut ready = VecDeque::from([entry]);
    // Stage terminators so a rejected join never leaves a partly rewritten graph.
    let mut completed = blocks
        .iter()
        .map(|block| block.terminator.clone())
        .collect::<Vec<_>>();
    while let Some(position) = ready.pop_front() {
        let mut frontier = incoming[position].clone().expect("queued frontier");
        for operation in &blocks[position].operations {
            for argument in operands(&operation.kind) {
                transfer(&mut frontier, argument)?;
            }
            if let Some((_, extraction)) = extractions
                .iter()
                .find(|(identity, _)| *identity == operation.id)
            {
                // Unlike a whole operand transfer, whole extraction leaves an
                // empty but allocated home until restoration or retirement.
                if extraction.path.is_empty() {
                    if frontier.iter().any(|root| root.place == extraction.place) {
                        return unsupported(
                            "whole extraction consumes a partially transferred root",
                        );
                    }
                    frontier.push(MovedRoot {
                        place: extraction.place,
                        paths: BTreeSet::from([Vec::new()]),
                    });
                    frontier.sort_by_key(|root| root.place);
                } else {
                    transfer(&mut frontier, extraction)?;
                }
            }
            if let OperationKind::ReleaseReference { source, path } = &operation.kind {
                remove_path(&mut frontier, *source, path)?;
            }
            // A store moves its whole value into the exact vacated subtree,
            // which rejoins its owner: later edges dispose it with the root.
            if let OperationKind::StoreStructuralField {
                destination,
                path,
                field,
                ..
            } = &operation.kind
                && frontier.iter().any(|root| root.place == *destination)
            {
                let root_type = roots
                    .iter()
                    .find_map(|(place, structural_type)| {
                        (*place == *destination).then_some(*structural_type)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "restoration root has no storage declaration",
                    ))?;
                let restored = terminal_semantics::structural_window_projection(
                    types.iter(),
                    root_type,
                    path,
                    *field,
                )
                .ok_or(LoweringError::Unsupported(
                    "restoration has no exact storage projection",
                ))?;
                restore_path(&mut frontier, *destination, &restored.path);
            }
            if let Some(result) = operation.result.structural() {
                frontier.retain(|root| root.place != result.place);
            }
        }
        let mut successors = Vec::new();
        match &mut completed[position] {
            Terminator::Jump {
                target,
                structural_arguments,
                cleanup_actions,
                ..
            } => {
                for argument in structural_arguments {
                    transfer(&mut frontier, argument)?;
                }
                complete_actions(&mut frontier, cleanup_actions, &roots, types)?;
                successors.push((*target, frontier));
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    let mut branch = frontier.clone();
                    for argument in &edge.structural_arguments {
                        transfer(&mut branch, argument)?;
                    }
                    complete_actions(&mut branch, &mut edge.cleanup_actions, &roots, types)?;
                    successors.push((edge.target, branch));
                }
            }
            Terminator::StructuralCase {
                source,
                path,
                cases,
            } => {
                // Inspection does not consume its carrier. Only transferred
                // regions overlapping the selected sum invalidate this read.
                if frontier.iter().any(|root| {
                    root.place == *source
                        && root.paths.iter().any(|moved| {
                            terminal_semantics::structural_paths_may_overlap(moved, path)
                        })
                }) {
                    return unsupported("case inspection overlaps a transferred subtree");
                }
                for edge in cases {
                    let mut branch = frontier.clone();
                    complete_actions(&mut branch, &mut edge.cleanup_actions, &roots, types)?;
                    successors.push((edge.target, branch));
                }
            }
            terminal_psi::terminator_return_pair_pattern!(cleanup_actions, ..) => {
                complete_actions(&mut frontier, cleanup_actions, &roots, types)?;
            }
            Terminator::ReturnStructural {
                source,
                cleanup_actions,
                ..
            } => {
                if frontier.iter().any(|root| root.place == *source) {
                    return unsupported("structural return consumes a partially transferred root");
                }
                complete_actions(&mut frontier, cleanup_actions, &roots, types)?;
            }
            Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => {}
        }
        for (target, mut outgoing) in successors {
            // Empty ownership complements do not end the storage occurrence:
            // scalar siblings may still be observed before scheduled cleanup.
            let target = block_position(blocks, target)?;
            // Destination parameters bind new occurrences simultaneously.
            outgoing.retain(|root| {
                !blocks[target]
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == root.place)
            });
            match &incoming[target] {
                Some(previous) if *previous != outgoing => {
                    return unsupported("projected transfer histories disagree at a control join");
                }
                Some(_) => {}
                None => {
                    incoming[target] = Some(outgoing);
                    ready.push_back(target);
                }
            }
        }
    }
    for (block, terminator) in blocks.iter_mut().zip(completed) {
        block.terminator = terminator;
    }
    Ok(())
}

/// The actual emitted transfers and disposals that partition an owned home.
/// Continuation preparation uses the same roster before physical identities
/// are allocated, so case-selected residual deaths cannot rejoin too early.
pub(crate) fn partitioned_roots(
    blocks: &[Block],
    extractions: &[(OperationId, StructuralArgument)],
) -> BTreeSet<PlaceId> {
    let mut roots = extractions
        .iter()
        .map(|(_, argument)| argument.place)
        .collect::<BTreeSet<_>>();
    for block in blocks {
        for argument in block
            .operations
            .iter()
            .flat_map(|operation| operands(&operation.kind))
            .chain(
                block
                    .terminator
                    .successor_block_arguments()
                    .into_iter()
                    .flatten(),
            )
        {
            if argument.access == StructuralAccess::Owned && !argument.path.is_empty() {
                roots.insert(argument.place);
            }
        }
        for operation in &block.operations {
            if let OperationKind::ReleaseReference { source, path } = &operation.kind
                && !path.is_empty()
            {
                roots.insert(*source);
            }
        }
        // Ending a carried leaf removes that region just as a move does.
        // Ignoring edge cleanup here skips complement completion in bodies
        // with no projected operand, leaving a later whole discard to consume
        // an already-disposed descriptor a second time.
        for (_, actions) in block.terminator.cleanup_rosters() {
            for action in actions {
                let ends_region = match action {
                    TerminalAffineCleanupAction::DiscardResidual(_) => true,
                    TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        !cleanup.retains_descendants
                    }
                    _ => false,
                };
                if ends_region && !action.path().is_empty() {
                    roots.insert(action.place());
                }
            }
        }
    }
    roots
}

pub(crate) fn operands(kind: &OperationKind) -> impl Iterator<Item = &StructuralArgument> {
    let stored = match kind {
        OperationKind::StoreStructuralField { value, .. } => Some(value),
        _ => None,
    };
    kind.constructor_arguments()
        .chain(stored)
        .chain(call_structural_arguments(kind))
}

fn transfer(frontier: &mut Frontier, argument: &StructuralArgument) -> Result<(), LoweringError> {
    if argument.access != StructuralAccess::Owned {
        return Ok(());
    }
    remove_path(frontier, argument.place, &argument.path)
}

fn remove_path(
    frontier: &mut Frontier,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<(), LoweringError> {
    if path.is_empty() {
        if frontier.iter().any(|root| root.place == place) {
            return unsupported("whole transfer consumes a partially transferred root");
        }
        return Ok(());
    }
    if let Some(root) = frontier.iter_mut().find(|root| root.place == place) {
        if root
            .paths
            .iter()
            .any(|earlier| earlier.starts_with(path) || path.starts_with(earlier))
        {
            return unsupported("projected custody overlaps an earlier transfer or release");
        }
        root.paths.insert(path.to_vec());
    } else {
        frontier.push(MovedRoot {
            place,
            paths: BTreeSet::from([path.to_vec()]),
        });
        frontier.sort_by_key(|root| root.place);
    }
    Ok(())
}

/// Rejoin an exactly restored subtree to its owner's frontier. An empty path
/// restores the whole referent a root window vacated.
fn restore_path(frontier: &mut Frontier, place: PlaceId, path: &[StructuralPathSegment]) {
    let Some(position) = frontier.iter().position(|root| root.place == place) else {
        return;
    };
    if path.is_empty() {
        frontier.remove(position);
        return;
    }
    let root = &mut frontier[position];
    // The establishing store replaces this entire region. Dispositions below
    // it describe the old occurrence, while a moved ancestor and outside
    // siblings still belong to the containing owner's outstanding frontier.
    root.paths.retain(|removed| !removed.starts_with(path));
    if root.paths.is_empty() {
        frontier.remove(position);
    }
}

fn residuals(
    root: &MovedRoot,
    roots: &[(PlaceId, StructuralTypeId)],
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<StructuralAffineDiscard>, LoweringError> {
    // A whole extraction vacates all cells but retains their allocation. No
    // residual can be disposed; only the already scheduled retirement remains.
    if root.paths.contains(&Vec::new()) {
        return Ok(Vec::new());
    }
    let mut matching = roots.iter().filter(|(place, _)| *place == root.place);
    let (_, root_type) = matching.next().ok_or(LoweringError::Unsupported(
        "projected cleanup root has no exact declaration",
    ))?;
    if matching.any(|(_, other)| other != root_type) {
        return unsupported("projected cleanup root has inconsistent declarations");
    }
    let paths =
        terminal_semantics::partial_affine_residuals(types, *root_type, &root.paths, usize::MAX)
            .ok_or(LoweringError::Unsupported(
                "projected cleanup has no bounded residual complement",
            ))?;
    Ok(paths
        .into_iter()
        .map(|(path, structural_type)| StructuralAffineDiscard {
            place: root.place,
            path,
            structural_type,
        })
        .collect())
}

/// Every normal exit retains one ordered subject stream. An intact nominal
/// child can die after its sibling moves, but no hook can cover a moved path.
fn complete_actions(
    frontier: &mut Frontier,
    cleanup_actions: &mut Vec<TerminalAffineCleanupAction>,
    roots: &[(PlaceId, StructuralTypeId)],
    types: &[StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let mut actions = Vec::new();
    let mut placeholders = BTreeSet::new();
    for action in cleanup_actions.iter() {
        let first_new_action = actions.len();
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place)
                if frontier.iter().any(|root| root.place == *place) =>
            {
                if !placeholders.insert(*place) {
                    return unsupported("duplicate scheduled whole cleanup");
                }
                let root = frontier.iter().find(|root| root.place == *place).ok_or(
                    LoweringError::Unsupported("scheduled partial cleanup root is absent"),
                )?;
                actions.extend(
                    residuals(root, roots, types)?
                        .into_iter()
                        .map(TerminalAffineCleanupAction::DiscardResidual),
                );
                // Expanding the already selected whole death into residuals
                // must preserve its home retirement, including an empty
                // complement. Region actions alone do not carry that lifetime.
                actions.push(TerminalAffineCleanupAction::RetireHome(*place));
            }
            _ => actions.push(action.clone()),
        }
        // Compose each selected action before expanding the next one. A leaf
        // death earlier on this very edge changes the complement just as a
        // death on a predecessor does; a two-pass expansion would dispose it
        // twice when a whole-home death follows in the same roster.
        for completed in &actions[first_new_action..] {
            match completed {
                TerminalAffineCleanupAction::RetireHome(place)
                | TerminalAffineCleanupAction::DiscardRoot(place) => {
                    frontier.retain(|root| root.place != *place);
                }
                TerminalAffineCleanupAction::InvokeNominal(cleanup)
                    if !cleanup.retains_descendants =>
                {
                    if cleanup.path.is_empty() {
                        frontier.retain(|root| root.place != cleanup.place);
                    } else {
                        remove_path(frontier, cleanup.place, &cleanup.path)?;
                    }
                }
                TerminalAffineCleanupAction::DiscardResidual(discard) => {
                    remove_path(frontier, discard.place, &discard.path)?;
                }
                TerminalAffineCleanupAction::InvokeNominal(_) => {}
            }
        }
    }
    *cleanup_actions = actions;
    Ok(())
}
