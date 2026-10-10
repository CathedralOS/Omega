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
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeSet, VecDeque};
use terminal_psi::{
    Block, OperationKind, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    TerminalAffineCleanupAction, Terminator,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct MovedRoot {
    place: PlaceId,
    paths: BTreeSet<Vec<StructuralPathSegment>>,
}
type Frontier = Vec<MovedRoot>;

pub(super) fn complete(
    parameters: &[StructuralParameterDeclaration],
    entry: BlockId,
    blocks: &mut [Block],
    types: &[StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    if !blocks.iter().any(|block| {
        block.operations.iter().any(|operation| {
            matches!(&operation.kind, OperationKind::ReleaseReference { path, .. } if !path.is_empty())
                || operands(&operation.kind).any(|argument| {
                argument.access == StructuralAccess::Owned && !argument.path.is_empty()
            })
        }) || block
            .terminator
            .successor_block_arguments()
            .iter()
            .any(|arguments| {
                arguments.iter().any(|argument| {
                    argument.access == StructuralAccess::Owned && !argument.path.is_empty()
                })
            })
    }) {
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
            {
                let mut stored = path.clone();
                if let Some(field) = field {
                    stored.push(StructuralPathSegment::Field(
                        crate::structural_types::field_identity(types, *field)?,
                    ));
                }
                restore_path(&mut frontier, *destination, &stored);
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
    root.paths.remove(path);
    if root.paths.is_empty() {
        frontier.remove(position);
    }
}

fn residuals(
    root: &MovedRoot,
    roots: &[(PlaceId, StructuralTypeId)],
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<StructuralAffineDiscard>, LoweringError> {
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
    }
    // Exact checked region actions can span conditional cleanup blocks. They
    // neither finish the owner nor form an entire complement on each edge.
    // Reconstructing that complement here used to close one case arm early
    // and reject its join with inactive arms. This producer only expands whole
    // placeholders; home retirement, not a projected leaf, ends the transfer
    // history. Source checking and boundary verifiers own partition validity.
    frontier.retain(|root| {
        !actions.iter().any(|action| match action {
            TerminalAffineCleanupAction::RetireHome(place)
            | TerminalAffineCleanupAction::DiscardRoot(place) => *place == root.place,
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                cleanup.place == root.place
                    && cleanup.path.is_empty()
                    && !cleanup.retains_descendants
            }
            TerminalAffineCleanupAction::DiscardResidual(_) => false,
        })
    });
    *cleanup_actions = actions;
    Ok(())
}
