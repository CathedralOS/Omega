//! A verified Terminal `AtomicAccess` becomes one normalized abstract atomic
//! event, with its retained coherence edges.
//!
//! The event keeps its exact location (the live arrival root, static carrier
//! path, scalar field), operation, orderings, operands and observed prior, so
//! optimization cannot split it into a read and a store and target refinement
//! can choose one instruction for it.
//!
//! # Coherence edges are produced here, checked downstream
//!
//! The optimization unit rechecks every event's `reads_from` and
//! `modification_after` claim against its own happens-before replay
//! (`happens_before_atomic_coherence_violation`), so this producer must name
//! exactly the write the event observes or immediately follows. Trace the
//! location backward from the event, stopping at the latest write on each path.
//! Whole-storage edge bindings rename the root while retaining its path and
//! field. Ordinary control edges preserve a dominating name. Projected or
//! ambiguous bindings cannot establish this exact-location relation.
//!
//! The frontier contains invocation residency, an atomic write, or an exact
//! intervening effect. An effect boundary expires knowledge of residency; the
//! next instruction observes current backing without claiming an internal
//! writer or reusing old facts. Independent replay requires that exact latest
//! effect on every path. Only one unanimous frontier supplies a retained edge;
//! branch-selected writes and ambiguous mutation refuse. The finite (block, root)
//! worklist follows cycles without treating a repeated visit as new evidence.
//! Event locations stay block-local so downstream authority and native address
//! checks use the actual arrival declaration, not an invented invocation alias.
use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::atomic::AbstractAtomicLocation;
use abstract_operations::{
    AbstractAtomicEvent, AbstractOperation, AtomicModificationAfter, AtomicReadsFrom,
};
use semantic_vocabulary::{OperationId, PlaceId, ScalarType};
use terminal_psi::{
    AtomicAccessEvent, Operation, OperationKind, StructuralTypeDeclaration, TerminalMachine,
};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<AbstractOperation, LoweringError> {
    let OperationKind::AtomicAccess {
        place,
        path,
        field,
        event,
    } = &operation.kind
    else {
        unreachable!("the atomic router dispatches only AtomicAccess");
    };
    let malformed = || LoweringError::VerifiedAtomicAccessMalformed(operation.id);
    let root = terminal_semantics::arrival_parameter(machine, *place)
        .ok_or(LoweringError::UnsupportedAtomicAccess(operation.id))?;
    let leaf =
        field_type(structural_types, root.structural_type, path, *field).ok_or_else(malformed)?;
    let location = AbstractAtomicLocation {
        root: *place,
        path: path.clone(),
        field: *field,
    };
    let prior = || {
        operation
            .result
            .scalar()
            .filter(|result| result.scalar_type == leaf)
            .map(|result| result.into())
            .ok_or_else(malformed)
    };
    let lowered = match *event {
        AtomicAccessEvent::Load { ordering } => AbstractAtomicEvent::Load {
            location,
            ordering,
            result: prior()?,
        },
        AtomicAccessEvent::Store { value, ordering } => {
            if operation.result != terminal_psi::OperationResult::Unit {
                return Err(malformed());
            }
            AbstractAtomicEvent::Store {
                location,
                ordering,
                value,
            }
        }
        AtomicAccessEvent::ReadModifyWrite {
            operation: arithmetic,
            operand,
            ordering,
        } => AbstractAtomicEvent::ReadModifyWrite {
            location,
            operation: arithmetic.into(),
            ordering,
            operand,
            prior: prior()?,
        },
        AtomicAccessEvent::Swap { value, ordering } => AbstractAtomicEvent::Swap {
            location,
            ordering,
            value,
            prior: prior()?,
        },
        AtomicAccessEvent::CompareExchange {
            expected,
            replacement,
            success,
            failure,
        } => AbstractAtomicEvent::CompareExchange {
            location,
            success,
            failure,
            expected,
            replacement,
            observed: prior()?,
        },
    };
    if !lowered.ordering_is_legal() || !lowered.custody_is_consistent() {
        return Err(malformed());
    }
    let reached = reaching_fact(machine, operation.id)?;
    let edge = |fact: &BTreeSet<Reaching>| match fact.iter().collect::<Vec<_>>().as_slice() {
        [reaching @ (Reaching::Initial | Reaching::Atomic(_) | Reaching::Effect(_))] => {
            Ok(**reaching)
        }
        _ => Err(LoweringError::UnsupportedAtomicCoherence(operation.id)),
    };
    let reads_from = if lowered.observes_resident() {
        Some(match edge(&reached)? {
            Reaching::Initial => AtomicReadsFrom::InitialResidency,
            Reaching::Atomic(operation) => AtomicReadsFrom::Write { operation },
            Reaching::Effect(operation) => AtomicReadsFrom::AfterEffect { operation },
            Reaching::Unknown => return Err(malformed()),
        })
    } else {
        None
    };
    let modification_after = if lowered.joins_modification_order() {
        Some(match edge(&reached)? {
            Reaching::Initial => AtomicModificationAfter::InitialResidency,
            Reaching::Atomic(operation) => AtomicModificationAfter::Write { operation },
            Reaching::Effect(operation) => AtomicModificationAfter::AfterEffect { operation },
            Reaching::Unknown => return Err(malformed()),
        })
    } else {
        None
    };
    Ok(AbstractOperation::AtomicEvent {
        psi_operation: operation.id,
        event: lowered,
        reads_from,
        modification_after,
    })
}

/// The atomic member `field` of the carrier `path` selects from `root`.
/// Record fields and mixed-case common fields share this namespace; a case
/// payload still needs its independently checked selected-case coordinates.
fn field_type(
    structural_types: &[StructuralTypeDeclaration],
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<ScalarType> {
    terminal_semantics::static_field_projection(structural_types.iter(), root, path, field)?
        .1
        .field_type
        .atomic_scalar_type()
}

/// One fact about the latest write to a location on some execution path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Reaching {
    /// The residency the activation received.
    Initial,
    /// The atomic writing event under this operation identity.
    Atomic(OperationId),
    /// Exact operation whose opaque access invalidated knowledge of residency.
    Effect(OperationId),
    /// A write no retained edge can name.
    Unknown,
}

/// The reaching facts of the location `target` accesses, just before
/// `target` executes.
fn reaching_fact(
    machine: &TerminalMachine,
    target: OperationId,
) -> Result<BTreeSet<Reaching>, LoweringError> {
    let unsupported = || LoweringError::UnsupportedAtomicCoherence(target);
    let (target_block, target_index, location) = machine
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .find_map(|(index, operation)| {
                    (operation.id == target).then(|| match &operation.kind {
                        OperationKind::AtomicAccess {
                            place, path, field, ..
                        } => Some((block.id, index, (*place, path.clone(), *field))),
                        _ => None,
                    })
                })
        })
        .flatten()
        .ok_or_else(unsupported)?;
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut reached = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = vec![(target_block, target_index, location.0)];
    while let Some((block, before, root)) = pending.pop() {
        // The target prefix and a full revisit through a backedge are different
        // execution points. A write in the latter belongs to an earlier turn.
        if !visited.insert((block, before, root)) {
            continue;
        }
        let declaration = blocks.get(&block).ok_or_else(unsupported)?;
        let local = (root, location.1.clone(), location.2);
        if let Some(write) = declaration.operations[..before]
            .iter()
            .rev()
            .find_map(|operation| latest_write(operation, &local, machine))
        {
            reached.insert(write);
            continue;
        }
        if block == machine.entry {
            reached.insert(
                if machine
                    .structural_parameters
                    .iter()
                    .any(|row| row.place == root)
                {
                    Reaching::Initial
                } else {
                    Reaching::Unknown
                },
            );
        }
        let mut has_predecessor = false;
        for predecessor in &machine.blocks {
            if !predecessor.terminator.successor_targets().contains(&block) {
                continue;
            }
            has_predecessor = true;
            match terminal_semantics::root_before_edge(machine, block, predecessor, root) {
                Some(source) => {
                    pending.push((predecessor.id, predecessor.operations.len(), source))
                }
                None => {
                    reached.insert(Reaching::Unknown);
                }
            }
        }
        if block != machine.entry && !has_predecessor {
            reached.insert(Reaching::Unknown);
        }
    }
    Ok(reached)
}

type Location = (
    PlaceId,
    Vec<terminal_psi::StructuralPathSegment>,
    semantic_vocabulary::StructuralFieldId,
);

/// The first preceding operation that prevents looking farther for a write.
fn latest_write(
    operation: &Operation,
    location: &Location,
    machine: &TerminalMachine,
) -> Option<Reaching> {
    let (root, path, field) = location;
    let may_alias = |other| {
        abstract_operations::atomic::atomic_roots_may_alias(
            &machine.structural_places,
            std::iter::once(machine.structural_parameters.as_slice()).chain(
                machine
                    .blocks
                    .iter()
                    .map(|block| block.structural_parameters.as_slice()),
            ),
            *root,
            other,
        )
    };
    match &operation.kind {
        OperationKind::AtomicAccess {
            place: other_root,
            path: other_path,
            field: other_field,
            event,
        } => {
            if other_root == root
                && other_path == path
                && other_field == field
                && event.modifies_resident()
            {
                return Some(Reaching::Atomic(operation.id));
            }
            if other_root != root && event.modifies_resident() && may_alias(*other_root) {
                return Some(Reaching::Effect(operation.id));
            }
        }
        OperationKind::StructuralScalarFieldStore {
            destination,
            path: other_path,
            field: other_field,
            ..
        } => {
            if (destination != root && may_alias(*destination))
                || (destination == root
                    && terminal_semantics::structural_paths_may_overlap(other_path, path)
                    && other_field == field)
            {
                return Some(Reaching::Effect(operation.id));
            }
        }
        // These observations do not write. A shared call is different: it
        // may modify this resident through the declared atomic cell.
        OperationKind::PrimitiveScalarRead { .. }
        | OperationKind::ScalarStructuralField { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | terminal_psi::structural_leaf_copy_pair_pattern!()
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::StructuralByteSequenceFieldRead { .. } => {}
        _ => {
            let lends_root = operation
                .kind
                .structural_projections()
                .iter()
                .any(|projection| projection.root == *root || may_alias(projection.root));
            if lends_root {
                return Some(Reaching::Effect(operation.id));
            }
        }
    }
    None
}
