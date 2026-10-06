//! Complete scheduled disposal from the exact transferred paths on each edge.
//!
//! This pass never chooses a root's lifetime. It changes an already scheduled
//! whole discard into the maximal complement of its earlier owned transfers
//! and ended reference leaves. Storage remains available to scalar siblings.
//! Joins require identical transfer frontiers; a result birth starts a fresh
//! occurrence. The independent receiver authenticates source custody and the
//! cleanup-free meaning of every residual.
use super::{LoweringError, block_position, unsupported};
use crate::direct_calls::call_structural_arguments;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeSet, VecDeque};
use terminal_psi::{
    Block, OperationKind, RecordFieldValue, StructuralAccess, StructuralAffineDiscard,
    StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, Terminator,
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
                || operands(&operation.kind).iter().any(|argument| {
                argument.access == StructuralAccess::Owned && !argument.path.is_empty()
            })
        }) || match &block.terminator {
            Terminator::Jump {
                structural_arguments,
                ..
            } => structural_arguments.iter().any(|argument| {
                argument.access == StructuralAccess::Owned && !argument.path.is_empty()
            }),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false].iter().any(|edge| {
                edge.structural_arguments.iter().any(|argument| {
                    argument.access == StructuralAccess::Owned && !argument.path.is_empty()
                })
            }),
            _ => false,
        }
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
            Terminator::StructuralCase { source, cases } => {
                if frontier.iter().any(|root| root.place == *source) {
                    return unsupported("case inspection consumes a partially transferred root");
                }
                for edge in cases {
                    let mut branch = frontier.clone();
                    complete_actions(&mut branch, &mut edge.cleanup_actions, &roots, types)?;
                    successors.push((edge.target, branch));
                }
            }
            Terminator::Return {
                cleanup_actions, ..
            }
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            } => {
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
            Terminator::Crash { .. } => {}
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

pub(super) fn operands(kind: &OperationKind) -> Vec<&StructuralArgument> {
    match kind {
        OperationKind::EstablishStructuralArray { elements } => elements.iter().collect(),
        OperationKind::EstablishRecord { fields }
        | OperationKind::EstablishStructuralCase { fields, .. } => fields
            .iter()
            .filter_map(|field| match &field.value {
                RecordFieldValue::Structural(argument) => Some(argument),
                _ => None,
            })
            .collect(),
        _ => call_structural_arguments(kind).iter().collect(),
    }
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

fn take_residuals(
    frontier: &mut Frontier,
    place: PlaceId,
    roots: &[(PlaceId, StructuralTypeId)],
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<StructuralAffineDiscard>, LoweringError> {
    let position = frontier
        .iter()
        .position(|root| root.place == place)
        .expect("selected partial root");
    let result = residuals(&frontier[position], roots, types)?;
    frontier.remove(position);
    Ok(result)
}

fn close_existing(
    frontier: &mut Frontier,
    scheduled: &[StructuralAffineDiscard],
    roots: &[(PlaceId, StructuralTypeId)],
    types: &[StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let mut closed = Vec::new();
    for root in frontier.iter() {
        let group = scheduled
            .iter()
            .filter(|discard| discard.place == root.place)
            .cloned()
            .collect::<Vec<_>>();
        if group.is_empty() {
            continue;
        }
        // Already authored cleanup remains owned by its existing exact replay.
        // When a declaration is available also ensure this pass's history agrees.
        if roots.iter().any(|(place, _)| *place == root.place)
            && group != residuals(root, roots, types)?
        {
            return unsupported("scheduled residual cleanup differs from current transfers");
        }
        closed.push(root.place);
    }
    frontier.retain(|root| !closed.contains(&root.place));
    Ok(())
}

/// All normal exits share the same ordered residual disposal. A
/// nominal owner cannot be partially moved before its whole-value hook.
fn complete_actions(
    frontier: &mut Frontier,
    cleanup_actions: &mut Vec<TerminalAffineCleanupAction>,
    roots: &[(PlaceId, StructuralTypeId)],
    types: &[StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let scheduled = cleanup_actions
        .iter()
        .filter_map(|action| match action {
            TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    close_existing(frontier, &scheduled, roots, types)?;
    let mut actions = Vec::new();
    for action in cleanup_actions.iter() {
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place)
                if frontier.iter().any(|root| root.place == *place) =>
            {
                actions.extend(
                    take_residuals(frontier, *place, roots, types)?
                        .into_iter()
                        .map(TerminalAffineCleanupAction::DiscardResidual),
                );
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup)
                if frontier.iter().any(|root| root.place == cleanup.place) =>
            {
                return unsupported("nominal cleanup consumes a partially transferred root");
            }
            _ => actions.push(action.clone()),
        }
    }
    *cleanup_actions = actions;
    Ok(())
}
