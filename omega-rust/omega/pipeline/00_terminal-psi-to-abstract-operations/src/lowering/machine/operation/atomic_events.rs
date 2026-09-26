//! A verified Terminal `AtomicAccess` becomes one normalized abstract atomic
//! event, with its retained coherence edges.
//!
//! The event keeps its exact location (machine parameter root, static carrier
//! path, scalar field), operation, orderings, operands and observed prior, so
//! optimization cannot split it into a read and a store and target refinement
//! can choose one instruction for it.
//!
//! # Coherence edges are produced here, checked downstream
//!
//! The optimization unit rechecks every event's `reads_from` and
//! `modification_after` claim against its own happens-before replay
//! (`happens_before_atomic_coherence_violation`), so this producer must name
//! exactly the write the event observes or immediately follows in its
//! location's modification order. It computes that from the Terminal graph by
//! a forward reaching-writes fixpoint whose facts are `Initial` (the residency
//! the activation received), one atomic write, or `Unknown`:
//!
//! - an atomic writing event on the same location replaces the fact with
//!   itself;
//! - any other operation that may write the location's root — a scalar field
//!   store to the same field, a call or projection that lends the root with
//!   write authority, a reference that could alias it — makes it `Unknown`,
//!   because the retained edges name only atomic events in this activation;
//! - a join unions the facts of its arrivals.
//!
//! An event whose reaching fact is exactly `Initial` or exactly one atomic
//! write gets that edge; anything else (a join of different writes, or an
//! intervening non-atomic write or lending call) has no single checkable
//! predecessor, and lowering refuses rather than retaining a claim the
//! concurrency contract would not support. Only machine-parameter roots are
//! admitted: a block parameter may alias the same storage under another place
//! identity, which would split one modification order in two.
use std::collections::{BTreeMap, BTreeSet};

use crate::abstract_operations::atomic::AbstractAtomicLocation;
use crate::abstract_operations::{
    AbstractAtomicEvent, AbstractAtomicReadModifyWrite, AbstractOperation, AbstractResult,
    AtomicModificationAfter, AtomicReadsFrom,
};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, ScalarType};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, Operation, OperationKind, StructuralAccess,
    StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    Terminator,
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
    let root = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *place)
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
            .map(|result| AbstractResult {
                value: result.id,
                scalar_type: result.scalar_type,
            })
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
            operation: match arithmetic {
                AtomicReadModifyWrite::FetchAdd => AbstractAtomicReadModifyWrite::FetchAdd,
                AtomicReadModifyWrite::FetchSub => AbstractAtomicReadModifyWrite::FetchSub,
                AtomicReadModifyWrite::FetchAnd => AbstractAtomicReadModifyWrite::FetchAnd,
                AtomicReadModifyWrite::FetchOr => AbstractAtomicReadModifyWrite::FetchOr,
                AtomicReadModifyWrite::FetchXor => AbstractAtomicReadModifyWrite::FetchXor,
            },
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
        [Reaching::Initial] => Ok(None),
        [Reaching::Atomic(write)] => Ok(Some(*write)),
        _ => Err(LoweringError::UnsupportedAtomicCoherence(operation.id)),
    };
    let reads_from = if lowered.observes_resident() {
        Some(match edge(&reached)? {
            None => AtomicReadsFrom::InitialResidency,
            Some(operation) => AtomicReadsFrom::Write { operation },
        })
    } else {
        None
    };
    let modification_after = if lowered.joins_modification_order() {
        Some(match edge(&reached)? {
            None => AtomicModificationAfter::InitialResidency,
            Some(operation) => AtomicModificationAfter::Write { operation },
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

/// The plain scalar field `field` of the record `path` selects from `root`.
fn field_type(
    structural_types: &[StructuralTypeDeclaration],
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<ScalarType> {
    let declaration = |id| {
        structural_types
            .iter()
            .find(|declaration| declaration.id == id)
    };
    let mut carrier = root;
    for segment in path {
        carrier = match (segment, &declaration(carrier)?.shape) {
            (
                terminal_psi::StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields },
            ) => match fields
                .iter()
                .find(|candidate| {
                    candidate.identity == *identity && !candidate.relevance.is_erased()
                })?
                .field_type
            {
                StructuralFieldType::Structural(child) => child,
                _ => return None,
            },
            (
                terminal_psi::StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    let StructuralTypeShape::Record { fields } = &declaration(carrier)?.shape else {
        return None;
    };
    match fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?
        .field_type
    {
        StructuralFieldType::Scalar(scalar_type) => Some(scalar_type),
        _ => None,
    }
}

/// One fact about the latest write to a location on some execution path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Reaching {
    /// The residency the activation received.
    Initial,
    /// The atomic writing event under this operation identity.
    Atomic(OperationId),
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
    let mut arrivals: BTreeMap<BlockId, BTreeSet<Reaching>> =
        BTreeMap::from([(machine.entry, BTreeSet::from([Reaching::Initial]))]);
    let mut pending = vec![machine.entry];
    while let Some(block) = pending.pop() {
        let mut fact = arrivals.get(&block).cloned().unwrap_or_default();
        for operation in &blocks.get(&block).ok_or_else(unsupported)?.operations {
            transfer(operation, &location, &mut fact);
        }
        for (successor, lends_root) in successors(
            &blocks.get(&block).ok_or_else(unsupported)?.terminator,
            location.0,
        ) {
            // An edge binding the root to a target block parameter hands the
            // storage to another place identity whose writes this fact does
            // not follow.
            let arriving = if lends_root {
                BTreeSet::from([Reaching::Unknown])
            } else {
                fact.clone()
            };
            let entry = arrivals.entry(successor).or_default();
            let before = entry.len();
            entry.extend(arriving);
            if entry.len() != before || before == 0 {
                pending.push(successor);
            }
        }
    }
    let mut fact = arrivals
        .get(&target_block)
        .cloned()
        .ok_or_else(unsupported)?;
    for operation in &blocks[&target_block].operations[..target_index] {
        transfer(operation, &location, &mut fact);
    }
    Ok(fact)
}

type Location = (
    PlaceId,
    Vec<terminal_psi::StructuralPathSegment>,
    semantic_vocabulary::StructuralFieldId,
);

/// Apply one operation's effect on `location`'s latest-write fact.
fn transfer(operation: &Operation, location: &Location, fact: &mut BTreeSet<Reaching>) {
    let (root, path, field) = location;
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
                *fact = BTreeSet::from([Reaching::Atomic(operation.id)]);
            }
        }
        OperationKind::StructuralScalarFieldStore {
            destination,
            path: other_path,
            field: other_field,
            ..
        } => {
            if destination == root && other_path == path && other_field == field {
                *fact = BTreeSet::from([Reaching::Unknown]);
            }
        }
        // Observations never write, and a shared loan never admits a write
        // (a modifying atomic event through one does not verify).
        OperationKind::PrimitiveScalarRead { .. }
        | OperationKind::IntegerStructuralField { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | OperationKind::StructuralLeafCopy { .. }
        | OperationKind::StructuralCaseLeafCopy { .. }
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::StructuralByteSequenceFieldRead { .. } => {}
        _ => {
            let lends_root = operation
                .kind
                .structural_projections()
                .iter()
                .any(|projection| projection.root == *root)
                && !shared_only(&operation.kind, *root);
            if lends_root {
                *fact = BTreeSet::from([Reaching::Unknown]);
            }
        }
    }
}

/// Whether every structural argument naming `root` lends it shared.
fn shared_only(kind: &OperationKind, root: PlaceId) -> bool {
    let arguments = match kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => return false,
    };
    arguments
        .iter()
        .filter(|argument| argument.place == root)
        .all(|argument| argument.access == StructuralAccess::SharedBorrow)
}

/// Each successor block, and whether its edge binds `root` to one of the
/// target's structural parameters.
fn successors(terminator: &Terminator, root: PlaceId) -> Vec<(BlockId, bool)> {
    let binds = |arguments: &[terminal_psi::StructuralArgument]| {
        arguments.iter().any(|argument| argument.place == root)
    };
    match terminator {
        Terminator::Jump {
            target,
            structural_arguments,
            ..
        } => vec![(*target, binds(structural_arguments))],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|edge| (edge.target, binds(&edge.structural_arguments)))
            .collect(),
        Terminator::StructuralCase { source, cases } => cases
            .iter()
            .map(|successor| (successor.target, *source == root))
            .collect(),
        Terminator::Return { .. }
        | Terminator::ReturnUnit { .. }
        | Terminator::ReturnUnitPartialAffine { .. }
        | Terminator::ReturnUnitNominalAffine { .. }
        | Terminator::ReturnStructural { .. }
        | Terminator::Crash { .. } => Vec::new(),
    }
}
