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
//! The frontier contains invocation residency, an atomic write, or an unknown
//! intervening write/loan. Only one unanimous frontier supplies a retained edge;
//! branch-selected writes and opaque mutation refuse. The finite (block, root)
//! worklist follows cycles without treating a repeated visit as new evidence.
//! Event locations stay block-local so downstream authority and native address
//! checks use the actual arrival declaration, not an invented invocation alias.
use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::atomic::AbstractAtomicLocation;
use abstract_operations::{
    AbstractAtomicEvent, AbstractAtomicReadModifyWrite, AbstractOperation, AbstractResult,
    AtomicModificationAfter, AtomicReadsFrom,
};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, ScalarType};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, Operation, OperationKind, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, Terminator,
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
        StructuralFieldType::Atomic(scalar_type) => Some(scalar_type),
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
            if !successors(&predecessor.terminator).contains(&block) {
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
        // Dynamic descriptors can capture aliases outside this operation's
        // explicit projection roster. No event history crosses that call.
        OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicParameterUnit { .. } => return Some(Reaching::Unknown),
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
                return Some(Reaching::Unknown);
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
                return Some(Reaching::Unknown);
            }
        }
        // These observations do not write. A shared call is different: it
        // may modify this resident through the declared atomic cell.
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
                .any(|projection| projection.root == *root || may_alias(projection.root));
            if lends_root {
                return Some(Reaching::Unknown);
            }
        }
    }
    None
}

/// Actual control successors, including edges that carry no structural data.
fn successors(terminator: &Terminator) -> Vec<BlockId> {
    match terminator {
        Terminator::Jump { target, .. } => vec![*target],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            vec![when_true.target, when_false.target]
        }
        Terminator::StructuralCase { cases, .. } => {
            cases.iter().map(|successor| successor.target).collect()
        }
        _ => Vec::new(),
    }
}
