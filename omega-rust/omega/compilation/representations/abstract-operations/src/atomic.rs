/*
We represent normalized atomic memory events, their proof-static ordering
plans, and the independent happens-before coherence verifier over `reads_from`
and `modification_after` residency edges.

Each atomic event retains its admitted ordering (`AbstractAtomicEvent::ordering_is_legal`)
and result custody (`AbstractAtomicEvent::custody_is_consistent`) so downstream
stages re-verify concurrency legality rather than trusting producer assertion.
For per-location coherence inside an activation, `happens_before_atomic_coherence_violation`
replays a backward worklist across the control-flow graph to verify that every
claimed `AtomicResidencyEdge` (`InitialResidency`, `Write`, `AfterEffect`, or
`PathAlternatives`) matches the exact reaching modification-order frontier.
*/

use std::collections::{BTreeMap, BTreeSet};

use language_core::atomic::{
    AtomicCompareExchangeOnceResultCustody, AtomicExpressionResultCustody, AtomicOrderingPlan,
    MemoryOrdering,
};
use semantic_vocabulary::{
    BlockId, OperationId, PlaceId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralOperationResult,
    StructuralPathSegment as PathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

use crate::{AbstractOperation, AbstractResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractAtomicReadModifyWrite {
    FetchAdd,
    FetchSub,
    FetchXor,
    FetchOr,
    FetchAnd,
}

impl From<terminal_psi::AtomicReadModifyWrite> for AbstractAtomicReadModifyWrite {
    fn from(operation: terminal_psi::AtomicReadModifyWrite) -> Self {
        match operation {
            terminal_psi::AtomicReadModifyWrite::FetchAdd => Self::FetchAdd,
            terminal_psi::AtomicReadModifyWrite::FetchSub => Self::FetchSub,
            terminal_psi::AtomicReadModifyWrite::FetchAnd => Self::FetchAnd,
            terminal_psi::AtomicReadModifyWrite::FetchOr => Self::FetchOr,
            terminal_psi::AtomicReadModifyWrite::FetchXor => Self::FetchXor,
        }
    }
}

impl From<AbstractAtomicReadModifyWrite> for terminal_psi::AtomicReadModifyWrite {
    fn from(operation: AbstractAtomicReadModifyWrite) -> Self {
        match operation {
            AbstractAtomicReadModifyWrite::FetchAdd => Self::FetchAdd,
            AbstractAtomicReadModifyWrite::FetchSub => Self::FetchSub,
            AbstractAtomicReadModifyWrite::FetchAnd => Self::FetchAnd,
            AbstractAtomicReadModifyWrite::FetchOr => Self::FetchOr,
            AbstractAtomicReadModifyWrite::FetchXor => Self::FetchXor,
        }
    }
}

pub use language_core::atomic::FenceOrdering as AbstractAtomicFenceOrdering;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AtomicResidencyEdge {
    InitialResidency, // Pre-activation residency; no earlier write to the place may precede on any path.
    Write {
        operation: OperationId, // Atomic write event standing under this operation ID in the checked activation.
    },
    AfterEffect {
        operation: OperationId, // Latest potentially modifying opaque operation on every incoming path.
    },
    PathAlternatives {
        sources: Vec<AtomicResidencySource>, // Sorted, distinct latest residencies across actual incoming paths.
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicResidencySource {
    Initial,
    Write { operation: OperationId },
    AfterEffect { operation: OperationId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicEdgeViolation {
    NonMemberEdge,
    MissingEdge,
    InitialResidencyAfterWrite,
    InvalidPathAlternatives,
    Unresolved { claimed: OperationId },
    OutsideModificationOrder { claimed: OperationId },
    NotHappensBefore { claimed: OperationId },
    Overwritten { claimed: OperationId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicCoherenceViolation {
    ReadsFrom(AtomicEdgeViolation),
    ModificationAfter(AtomicEdgeViolation),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbstractAtomicLocation {
    pub root: PlaceId,
    pub path: Vec<PathSegment>,
}

fn contains_only_scalars<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> bool {
    fn visit<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
    ) -> bool {
        if active.contains(&root) {
            return false;
        }
        let mut matching = types.clone().filter(|declaration| declaration.id == root);
        let Some(declaration) = matching.next() else {
            return false;
        };
        if matching.next().is_some()
            || declaration.owns_linear_claim
            || declaration.quotient_carrier.is_some()
            || declaration.scalar_restriction.is_some()
        {
            return false;
        }
        active.push(root);
        let mut field = |field: &terminal_psi::StructuralFieldDeclaration| {
            if field.storage.is_some() {
                return false;
            }
            if field.relevance.is_erased() {
                return matches!(field.field_type, StructuralFieldType::Erased { .. });
            }
            match field.field_type {
                StructuralFieldType::Scalar(_) => true,
                StructuralFieldType::Structural(child) => visit(types.clone(), child, active),
                _ => false,
            }
        };
        let result = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => true,
            StructuralTypeShape::Record {
                fields,
                storage: terminal_psi::StructuralRecordStorage::Packed,
            } => fields.iter().all(&mut field),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| &case.fields).all(&mut field)
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .all(&mut field),
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(types.clone(), *element, active)
            }
            _ => false,
        };
        active.pop();
        result
    }
    visit(types, root, &mut Vec::new())
}

pub fn atomic_projections_may_alias<'parameters, 'types>(
    places: &[terminal_psi::StructuralPlaceDeclaration],
    arrivals: impl Iterator<Item = &'parameters [terminal_psi::StructuralParameterDeclaration]>,
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    left: PlaceId,
    left_path: &[PathSegment],
    right: PlaceId,
    right_path: &[PathSegment],
) -> bool {
    let immutable_image = |root| {
        places.iter().any(|place| {
            place.id == root
                && matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. })
        })
    };
    let scalar_owned = |root, path: &[PathSegment]| {
        !path.contains(&PathSegment::Referent)
            && places.iter().any(|place| {
                place.id == root
                    && match place.kind {
                        StructuralPlaceKind::OperationResult {
                            structural_type, ..
                        } => contains_only_scalars(types.clone(), structural_type),
                        _ => false,
                    }
            })
    };
    left == right
        || (!immutable_image(left)
            && !immutable_image(right)
            && !scalar_owned(left, left_path)
            && !scalar_owned(right, right_path)
            && (left_path.contains(&PathSegment::Referent)
                || right_path.contains(&PathSegment::Referent)
                || !arrivals.into_iter().any(|parameters| {
                    let exclusive = |root| {
                        parameters.iter().any(|parameter| {
                            parameter.place == root
                                && matches!(
                                    parameter.access,
                                    StructuralAccess::Owned | StructuralAccess::MutableBorrow
                                )
                        })
                    };
                    let closed_exclusive = |root| {
                        parameters.iter().any(|parameter| {
                            parameter.place == root
                                && matches!(
                                    parameter.access,
                                    StructuralAccess::Owned
                                        | StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow
                                )
                                && contains_only_scalars(types.clone(), parameter.structural_type)
                        })
                    };
                    let has_root =
                        |root| parameters.iter().any(|parameter| parameter.place == root);
                    exclusive(left) && exclusive(right)
                        || closed_exclusive(left) && has_root(right)
                        || closed_exclusive(right) && has_root(left)
                })))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractAtomicEvent {
    Load {
        location: AbstractAtomicLocation,
        ordering: MemoryOrdering,
        result: AbstractResult,
    },
    Store {
        location: AbstractAtomicLocation,
        ordering: MemoryOrdering,
        value: ValueId,
    },
    ReadModifyWrite {
        location: AbstractAtomicLocation,
        operation: AbstractAtomicReadModifyWrite,
        ordering: MemoryOrdering,
        operand: ValueId,
        prior: AbstractResult,
    },
    Swap {
        location: AbstractAtomicLocation,
        ordering: MemoryOrdering,
        value: ValueId,
        prior: AbstractResult,
    },
    CompareExchange {
        location: AbstractAtomicLocation,
        success: MemoryOrdering,
        failure: MemoryOrdering,
        expected: ValueId,
        replacement: ValueId,
        observed: AbstractResult,
    },
    CompareExchangeOnce {
        location: AbstractAtomicLocation,
        success: MemoryOrdering,
        failure: MemoryOrdering,
        expected: ValueId,
        replacement: ValueId,
        outcome: StructuralOperationResult,
        custody: AtomicCompareExchangeOnceResultCustody,
    },
    Fence {
        ordering: AbstractAtomicFenceOrdering,
    },
}

impl AbstractAtomicEvent {
    pub const fn replacement(&self) -> Option<ValueId> {
        match self {
            Self::Store { value, .. } | Self::Swap { value, .. } => Some(*value),
            Self::CompareExchange { replacement, .. }
            | Self::CompareExchangeOnce { replacement, .. } => Some(*replacement),
            _ => None,
        }
    }

    pub fn for_each_input_mut(&mut self, mut visit: impl FnMut(&mut ValueId)) {
        match self {
            Self::Store { value, .. } | Self::Swap { value, .. } => visit(value),
            Self::ReadModifyWrite { operand, .. } => visit(operand),
            Self::CompareExchange {
                expected,
                replacement,
                ..
            }
            | Self::CompareExchangeOnce {
                expected,
                replacement,
                ..
            } => {
                visit(expected);
                visit(replacement);
            }
            Self::Load { .. } | Self::Fence { .. } => {}
        }
    }

    pub const fn location(&self) -> Option<&AbstractAtomicLocation> {
        match self {
            Self::Load { location, .. }
            | Self::Store { location, .. }
            | Self::ReadModifyWrite { location, .. }
            | Self::Swap { location, .. }
            | Self::CompareExchange { location, .. }
            | Self::CompareExchangeOnce { location, .. } => Some(location),
            Self::Fence { .. } => None,
        }
    }

    pub fn place(&self) -> Option<PlaceId> {
        self.location().map(|location| location.root)
    }

    pub const fn ordering_plan(&self) -> Option<AtomicOrderingPlan> {
        match self {
            Self::Load { ordering, .. } => Some(AtomicOrderingPlan::Load(*ordering)),
            Self::Store { ordering, .. } => Some(AtomicOrderingPlan::Store(*ordering)),
            Self::ReadModifyWrite { ordering, .. } => {
                Some(AtomicOrderingPlan::ReadModifyWrite(*ordering))
            }
            Self::Swap { ordering, .. } => Some(AtomicOrderingPlan::Swap(*ordering)),
            Self::CompareExchange {
                success, failure, ..
            } => Some(AtomicOrderingPlan::CompareExchange {
                success: *success,
                failure: *failure,
            }),
            Self::CompareExchangeOnce {
                success, failure, ..
            } => Some(AtomicOrderingPlan::CompareExchangeOnce {
                success: *success,
                failure: *failure,
            }),
            Self::Fence { .. } => None,
        }
    }

    pub const fn result_custody(&self) -> Option<AtomicExpressionResultCustody> {
        match self {
            Self::Load { .. }
            | Self::ReadModifyWrite { .. }
            | Self::Swap { .. }
            | Self::CompareExchange { .. } => Some(AtomicExpressionResultCustody::Scalar),
            Self::CompareExchangeOnce { custody, .. } => {
                Some(AtomicExpressionResultCustody::CompareExchangeOnce(*custody))
            }
            Self::Store { .. } | Self::Fence { .. } => None,
        }
    }

    pub const fn ordering_is_legal(&self) -> bool {
        match self {
            Self::Load { ordering, .. } => ordering.valid_for_load(),
            Self::Store { ordering, .. } => ordering.valid_for_store(),
            Self::ReadModifyWrite { .. } | Self::Swap { .. } => true,
            Self::CompareExchange {
                success, failure, ..
            }
            | Self::CompareExchangeOnce {
                success, failure, ..
            } => failure.valid_compare_exchange_failure(*success),
            Self::Fence { .. } => true,
        }
    }

    pub const fn custody_is_consistent(&self) -> bool {
        match (self.ordering_plan(), self.result_custody()) {
            (Some(plan), Some(custody)) => custody.is_valid_for(plan),
            (None, None) | (_, None) => true,
            (None, Some(_)) => false,
        }
    }

    pub const fn observes_resident(&self) -> bool {
        match self {
            Self::Load { .. }
            | Self::ReadModifyWrite { .. }
            | Self::Swap { .. }
            | Self::CompareExchange { .. }
            | Self::CompareExchangeOnce { .. } => true,
            Self::Store { .. } | Self::Fence { .. } => false,
        }
    }

    pub const fn joins_modification_order(&self) -> bool {
        match self {
            Self::Store { .. }
            | Self::ReadModifyWrite { .. }
            | Self::Swap { .. }
            | Self::CompareExchange { .. }
            | Self::CompareExchangeOnce { .. } => true,
            Self::Load { .. } | Self::Fence { .. } => false,
        }
    }
}

pub fn happens_before_atomic_coherence_violation(
    blocks: &BTreeMap<BlockId, Vec<&AbstractOperation>>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    block_dominates: impl Fn(BlockId, BlockId) -> bool,
    entry: BlockId,
    projection_before_edge: impl Fn(BlockId, BlockId, PlaceId) -> Option<(PlaceId, Vec<PathSegment>)>,
    projections_may_alias: impl Fn(PlaceId, &[PathSegment], PlaceId, &[PathSegment]) -> bool,
    field_projection: impl Fn(PlaceId, &[PathSegment], StructuralFieldId) -> Option<Vec<PathSegment>>,
) -> Option<(BlockId, usize, AtomicCoherenceViolation)> {
    let mut events: BTreeMap<OperationId, Vec<(BlockId, usize, &AbstractAtomicEvent)>> =
        BTreeMap::new();
    for (block, operations) in blocks {
        for (index, operation) in operations.iter().enumerate() {
            if let AbstractOperation::AtomicEvent {
                psi_operation,
                event,
                ..
            } = *operation
            {
                events
                    .entry(*psi_operation)
                    .or_default()
                    .push((*block, index, event));
            }
        }
    }
    for (block, operations) in blocks {
        for (index, operation) in operations.iter().enumerate() {
            let AbstractOperation::AtomicEvent {
                event,
                reads_from,
                modification_after,
                ..
            } = *operation
            else {
                continue;
            };
            let reached = event
                .location()
                .map(|location| {
                    reaching_writes(
                        blocks,
                        predecessors,
                        entry,
                        *block,
                        index,
                        location,
                        &projection_before_edge,
                        &projections_may_alias,
                        &field_projection,
                    )
                })
                .unwrap_or_default();
            let rosters: [(
                bool,
                Option<&AtomicResidencyEdge>,
                fn(AtomicEdgeViolation) -> AtomicCoherenceViolation,
            ); 2] = [
                (
                    event.observes_resident(),
                    reads_from.as_ref(),
                    AtomicCoherenceViolation::ReadsFrom,
                ),
                (
                    event.joins_modification_order(),
                    modification_after.as_ref(),
                    AtomicCoherenceViolation::ModificationAfter,
                ),
            ];
            let violation = rosters.into_iter().find_map(|(member, edge, coherence)| {
                let refusal = match (member, edge) {
                    (false, Some(_)) => Some(AtomicEdgeViolation::NonMemberEdge),
                    (true, None) => Some(AtomicEdgeViolation::MissingEdge),
                    (false, None) => None,
                    (true, Some(edge)) => residency_edge_violation(
                        event,
                        edge,
                        *block,
                        index,
                        &events,
                        &reached,
                        &block_dominates,
                    ),
                };
                refusal.map(coherence)
            });
            if let Some(violation) = violation {
                return Some((*block, index, violation));
            }
        }
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ReachingWrite {
    Initial,
    Atomic(BlockId, usize),
    Effect(OperationId),
    Unknown,
}

#[allow(clippy::too_many_arguments)]
fn reaching_writes(
    blocks: &BTreeMap<BlockId, Vec<&AbstractOperation>>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    entry: BlockId,
    block: BlockId,
    index: usize,
    location: &AbstractAtomicLocation,
    projection_before_edge: &impl Fn(BlockId, BlockId, PlaceId) -> Option<(PlaceId, Vec<PathSegment>)>,
    projections_may_alias: &impl Fn(PlaceId, &[PathSegment], PlaceId, &[PathSegment]) -> bool,
    field_projection: &impl Fn(PlaceId, &[PathSegment], StructuralFieldId) -> Option<Vec<PathSegment>>,
) -> BTreeSet<ReachingWrite> {
    let mut reached = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = vec![(block, index, location.clone(), Vec::new())];
    while let Some((block, end, location, mut ancestry)) = pending.pop() {
        if !visited.insert((block, end, location.clone())) {
            continue;
        }
        let coordinate = (block, end, location.root);
        if ancestry.contains(&coordinate) {
            reached.insert(ReachingWrite::Unknown);
            continue;
        }
        ancestry.push(coordinate);
        let Some(operations) = blocks.get(&block) else {
            reached.insert(ReachingWrite::Unknown);
            continue;
        };
        let mut stopped = false;
        for (index, operation) in operations[..end].iter().enumerate().rev() {
            if let AbstractOperation::AtomicEvent { event, .. } = operation
                && event.joins_modification_order()
                && event.location() == Some(&location)
            {
                reached.insert(ReachingWrite::Atomic(block, index));
                stopped = true;
                break;
            }
            if invalidates_location(
                operation,
                &location,
                projections_may_alias,
                field_projection,
            ) {
                reached.insert(
                    operation
                        .psi_operation()
                        .map_or(ReachingWrite::Unknown, ReachingWrite::Effect),
                );
                stopped = true;
                break;
            }
        }
        if stopped {
            continue;
        }
        if block == entry {
            reached.insert(ReachingWrite::Initial);
        }
        let incoming = predecessors.get(&block);
        if block != entry && incoming.is_none_or(BTreeSet::is_empty) {
            reached.insert(ReachingWrite::Unknown);
        }
        for predecessor in incoming.into_iter().flatten() {
            let Some((root, mut path)) = projection_before_edge(*predecessor, block, location.root)
            else {
                reached.insert(ReachingWrite::Unknown);
                continue;
            };
            let Some(operations) = blocks.get(predecessor) else {
                reached.insert(ReachingWrite::Unknown);
                continue;
            };
            path.extend_from_slice(&location.path);
            pending.push((
                *predecessor,
                operations.len(),
                AbstractAtomicLocation { root, path },
                ancestry.clone(),
            ));
        }
    }
    if reached.is_empty() {
        reached.insert(ReachingWrite::Unknown);
    }
    reached
}

fn invalidates_location(
    operation: &AbstractOperation,
    location: &AbstractAtomicLocation,
    projections_may_alias: &impl Fn(PlaceId, &[PathSegment], PlaceId, &[PathSegment]) -> bool,
    field_projection: &impl Fn(PlaceId, &[PathSegment], StructuralFieldId) -> Option<Vec<PathSegment>>,
) -> bool {
    if operation
        .structural_result()
        .is_some_and(|(_, result)| result.place == location.root)
    {
        return true;
    }
    if operation.invocation_arguments().iter().any(|argument| {
        argument.place == location.root
            || projections_may_alias(
                argument.place,
                &argument.path,
                location.root,
                &location.path,
            )
    }) {
        return true;
    }
    operation.storage_mutations().into_iter().any(|mutation| {
        if mutation.place != location.root {
            return projections_may_alias(
                mutation.place,
                mutation.path,
                location.root,
                &location.path,
            );
        }
        match operation {
            AbstractOperation::StructuralScalarFieldStore { .. }
            | AbstractOperation::AtomicEvent { .. } => {
                let complete;
                let path = if let Some(field) = mutation.field {
                    let Some(path) = field_projection(mutation.place, mutation.path, field) else {
                        return true;
                    };
                    complete = path;
                    complete.as_slice()
                } else {
                    mutation.path
                };
                path.contains(&PathSegment::Referent)
                    || location.path.contains(&PathSegment::Referent)
                    || carrier_paths_may_overlap(path, &location.path)
            }
            _ => true,
        }
    })
}

fn carrier_paths_may_overlap(left: &[PathSegment], right: &[PathSegment]) -> bool {
    for (left, right) in left.iter().zip(right) {
        match (left, right) {
            (PathSegment::Field(left), PathSegment::Field(right)) if left != right => {
                return false;
            }
            (PathSegment::FixedIndex(left), PathSegment::FixedIndex(right)) if left != right => {
                return false;
            }
            (PathSegment::Field(_), PathSegment::Field(_))
            | (PathSegment::FixedIndex(_), PathSegment::FixedIndex(_))
            | (PathSegment::FixedIndex(_), PathSegment::RuntimeIndex { .. })
            | (PathSegment::RuntimeIndex { .. }, PathSegment::FixedIndex(_)) => {}
            _ => return true,
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn residency_edge_violation(
    event: &AbstractAtomicEvent,
    edge: &AtomicResidencyEdge,
    block: BlockId,
    index: usize,
    events: &BTreeMap<OperationId, Vec<(BlockId, usize, &AbstractAtomicEvent)>>,
    reached: &BTreeSet<ReachingWrite>,
    block_dominates: &impl Fn(BlockId, BlockId) -> bool,
) -> Option<AtomicEdgeViolation> {
    let Some(location) = event.location() else {
        return Some(AtomicEdgeViolation::MissingEdge);
    };
    match edge {
        AtomicResidencyEdge::PathAlternatives { sources } => {
            if sources.len() < 2 || sources.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Some(AtomicEdgeViolation::InvalidPathAlternatives);
            }
            let mut claimed_frontier = BTreeSet::new();
            for source in sources {
                let source = match *source {
                    AtomicResidencySource::Initial => ReachingWrite::Initial,
                    AtomicResidencySource::AfterEffect { operation } => {
                        ReachingWrite::Effect(operation)
                    }
                    AtomicResidencySource::Write { operation: claimed } => {
                        let Some([(writer, position, resolved)]) =
                            events.get(&claimed).map(Vec::as_slice)
                        else {
                            return Some(AtomicEdgeViolation::Unresolved { claimed });
                        };
                        if !resolved.joins_modification_order()
                            || (resolved.location() != Some(location)
                                && !reached.contains(&ReachingWrite::Atomic(*writer, *position)))
                        {
                            return Some(AtomicEdgeViolation::OutsideModificationOrder { claimed });
                        }
                        ReachingWrite::Atomic(*writer, *position)
                    }
                };
                claimed_frontier.insert(source);
            }
            (claimed_frontier != *reached).then_some(AtomicEdgeViolation::InvalidPathAlternatives)
        }
        AtomicResidencyEdge::AfterEffect { operation } => (reached
            != &BTreeSet::from([ReachingWrite::Effect(*operation)]))
            .then_some(AtomicEdgeViolation::Unresolved {
                claimed: *operation,
            }),
        AtomicResidencyEdge::InitialResidency => (reached
            != &BTreeSet::from([ReachingWrite::Initial]))
            .then_some(AtomicEdgeViolation::InitialResidencyAfterWrite),
        AtomicResidencyEdge::Write { operation: claimed } => {
            let claimed = *claimed;
            let Some([(writer, position, resolved)]) = events.get(&claimed).map(Vec::as_slice)
            else {
                return Some(AtomicEdgeViolation::Unresolved { claimed });
            };
            if !resolved.joins_modification_order()
                || (resolved.location() != Some(location)
                    && !reached.contains(&ReachingWrite::Atomic(*writer, *position)))
            {
                return Some(AtomicEdgeViolation::OutsideModificationOrder { claimed });
            }
            let happens_before = if *writer == block {
                *position < index
            } else {
                block_dominates(*writer, block)
            };
            if !happens_before {
                return Some(AtomicEdgeViolation::NotHappensBefore { claimed });
            }
            (reached != &BTreeSet::from([ReachingWrite::Atomic(*writer, *position)]))
                .then_some(AtomicEdgeViolation::Overwritten { claimed })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralCaseId,
        StructuralFieldId, StructuralPlaceKind,
    };
    use terminal_psi::{
        BindingRelevance, StructuralAccess, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralPlaceDeclaration, TerminalIdentityIndex,
    };

    fn declaration() -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            requires: Vec::new(),
            id: StructuralTypeId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Sum {
                cases: vec![StructuralCaseDeclaration {
                    id: StructuralCaseId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(2),
                    fields: vec![StructuralFieldDeclaration {
                        inline_carrier: None,
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: TerminalIdentityIndex(3),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        storage: None,
                    }],
                }],
            },
        }
    }

    fn may_alias(
        declaration: &StructuralTypeDeclaration,
        fresh: bool,
        path: &[StructuralPathSegment],
    ) -> bool {
        let query = PlaceId::new(1).unwrap();
        let value = PlaceId::new(2).unwrap();
        let places = vec![StructuralPlaceDeclaration {
            id: value,
            kind: if fresh {
                StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(1).unwrap(),
                    structural_type: declaration.id,
                }
            } else {
                StructuralPlaceKind::Parameter { position: 1 }
            },
        }];
        let arrivals = vec![StructuralParameterDeclaration {
            place: query,
            position: 0,
            structural_type: declaration.id,
            access: StructuralAccess::SharedBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
            additional_input_qualifications: Vec::new(),
        }];
        atomic_projections_may_alias(
            &places,
            std::iter::once(arrivals.as_slice()),
            std::iter::once(declaration),
            query,
            &[],
            value,
            path,
        )
    }

    #[test]
    fn fresh_scalar_values_cannot_lend_separate_atomic_backing() {
        let mut declaration = declaration();
        declaration.descriptor_cleanup = true;
        assert!(contains_only_scalars(
            std::iter::once(&declaration),
            declaration.id
        ));
        assert!(!may_alias(&declaration, true, &[]));
        assert!(
            may_alias(&declaration, false, &[]),
            "a mere parameter name proves no fresh storage"
        );
        assert!(
            may_alias(&declaration, true, &[StructuralPathSegment::Referent]),
            "crossing a descriptor is never the scalar home"
        );
    }

    #[test]
    fn atomic_reference_and_unknown_payloads_retain_alias_uncertainty() {
        for mutation in 0..4 {
            let mut declaration = declaration();
            match mutation {
                0 => {
                    let StructuralTypeShape::Sum { cases } = &mut declaration.shape else {
                        unreachable!()
                    };
                    cases[0].fields[0].field_type =
                        StructuralFieldType::Atomic(ScalarType::Boolean);
                }
                1 => {
                    declaration.shape = StructuralTypeShape::Reference {
                        referent: declaration.id,
                        access: StructuralAccess::SharedBorrow,
                    }
                }
                2 => {
                    let StructuralTypeShape::Sum { cases } = &mut declaration.shape else {
                        unreachable!()
                    };
                    cases[0].fields[0].field_type =
                        StructuralFieldType::Structural(StructuralTypeId::new(99).unwrap());
                }
                3 => declaration.owns_linear_claim = true,
                _ => unreachable!(),
            }
            assert!(may_alias(&declaration, true, &[]), "mutation {mutation}");
        }
    }

    #[test]
    fn all_cases_and_closed_children_constrain_storage_reach() {
        let mut outer = declaration();
        let mut child = declaration();
        child.id = StructuralTypeId::new(2).unwrap();
        child.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
        let StructuralTypeShape::Sum { cases } = &mut outer.shape else {
            unreachable!()
        };
        let mut second = cases[0].clone();
        second.id = StructuralCaseId::new(2).unwrap();
        second.fields[0].field_type = StructuralFieldType::Structural(child.id);
        cases.push(second);
        assert!(contains_only_scalars(
            [&outer, &child].into_iter(),
            outer.id
        ));
        assert!(
            !contains_only_scalars([&outer].into_iter(), outer.id),
            "missing child"
        );
        assert!(
            !contains_only_scalars([&outer, &child, &child].into_iter(), outer.id),
            "ambiguous child"
        );
        child.shape = StructuralTypeShape::Reference {
            referent: outer.id,
            access: StructuralAccess::SharedBorrow,
        };
        assert!(
            !contains_only_scalars([&outer, &child].into_iter(), outer.id),
            "an inactive case can carry a reference"
        );
        child.shape = StructuralTypeShape::FixedArray {
            element: outer.id,
            length: terminal_psi::ArrayExtent::Literal(1),
        };
        assert!(
            !contains_only_scalars([&outer, &child].into_iter(), outer.id),
            "recursive graph is unknown"
        );
    }

    fn operation(edge: AtomicResidencyEdge) -> AbstractOperation {
        AbstractOperation::AtomicEvent {
            psi_operation: OperationId::new(1).unwrap(),
            event: AbstractAtomicEvent::ReadModifyWrite {
                location: AbstractAtomicLocation {
                    root: PlaceId::new(1).unwrap(),
                    path: vec![PathSegment::Field(terminal_psi::TerminalIdentityIndex(1))],
                },
                operation: AbstractAtomicReadModifyWrite::FetchAnd,
                ordering: MemoryOrdering::Receive,
                operand: ValueId::new(1).unwrap(),
                prior: AbstractResult {
                    value: ValueId::new(2).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                    ),
                },
            },
            resident_obligation: None,
            reads_from: Some(edge.clone()),
            modification_after: Some(edge),
        }
    }

    fn alternatives() -> AtomicResidencyEdge {
        AtomicResidencyEdge::PathAlternatives {
            sources: vec![
                AtomicResidencySource::Initial,
                AtomicResidencySource::Write {
                    operation: OperationId::new(1).unwrap(),
                },
            ],
        }
    }

    fn replay(
        edge: AtomicResidencyEdge,
        backedge: impl Fn(PlaceId) -> (PlaceId, Vec<PathSegment>),
    ) -> Option<(BlockId, usize, AtomicCoherenceViolation)> {
        let entry = BlockId::new(1).unwrap();
        let repeated = BlockId::new(2).unwrap();
        let operation = operation(edge);
        let blocks = BTreeMap::from([(entry, Vec::new()), (repeated, vec![&operation])]);
        let predecessors = BTreeMap::from([(repeated, BTreeSet::from([entry, repeated]))]);
        let dominators = BTreeMap::from([
            (entry, BTreeSet::from([entry])),
            (repeated, BTreeSet::from([entry, repeated])),
        ]);
        happens_before_atomic_coherence_violation(
            &blocks,
            &predecessors,
            |producer, consumer| {
                dominators
                    .get(&consumer)
                    .is_some_and(|set| set.contains(&producer))
            },
            entry,
            |source, _, root| {
                Some(if source == repeated {
                    backedge(root)
                } else {
                    (root, Vec::new())
                })
            },
            |_, _, _, _| false,
            |_, _, _| None,
        )
    }

    #[test]
    fn loop_retains_initial_and_previous_iteration_residency() {
        assert_eq!(replay(alternatives(), |root| (root, Vec::new())), None);
    }

    #[test]
    fn loop_cannot_claim_only_its_previous_iteration_write() {
        let edge = AtomicResidencyEdge::Write {
            operation: OperationId::new(1).unwrap(),
        };
        assert!(replay(edge, |root| (root, Vec::new())).is_some());
    }

    #[test]
    fn alternatives_cannot_add_a_non_reaching_writer() {
        let edge = AtomicResidencyEdge::PathAlternatives {
            sources: vec![
                AtomicResidencySource::Initial,
                AtomicResidencySource::Write {
                    operation: OperationId::new(99).unwrap(),
                },
            ],
        };
        assert!(replay(edge, |root| (root, Vec::new())).is_some());
    }

    #[test]
    fn alternatives_reject_changed_backedge_storage() {
        assert!(replay(alternatives(), |_| (PlaceId::new(2).unwrap(), Vec::new())).is_some());
        assert!(replay(alternatives(), |root| (root, vec![PathSegment::Referent])).is_some());
    }

    #[test]
    fn alternatives_must_be_canonical_and_nontrivial() {
        for sources in [
            vec![AtomicResidencySource::Initial],
            vec![
                AtomicResidencySource::Initial,
                AtomicResidencySource::Initial,
            ],
            vec![
                AtomicResidencySource::Write {
                    operation: OperationId::new(1).unwrap(),
                },
                AtomicResidencySource::Initial,
            ],
        ] {
            assert!(
                replay(AtomicResidencyEdge::PathAlternatives { sources }, |root| (
                    root,
                    Vec::new()
                ))
                .is_some()
            );
        }
    }

    #[test]
    fn alternatives_reject_a_defined_writer_that_cannot_reach_the_loop() {
        let entry = BlockId::new(1).unwrap();
        let repeated = BlockId::new(2).unwrap();
        let exit = BlockId::new(3).unwrap();
        let writer = OperationId::new(99).unwrap();
        let after_loop = AbstractOperation::AtomicEvent {
            psi_operation: writer,
            event: AbstractAtomicEvent::Store {
                location: AbstractAtomicLocation {
                    root: PlaceId::new(1).unwrap(),
                    path: vec![PathSegment::Field(terminal_psi::TerminalIdentityIndex(1))],
                },
                ordering: MemoryOrdering::Publish,
                value: ValueId::new(1).unwrap(),
            },
            resident_obligation: None,
            reads_from: None,
            modification_after: Some(AtomicResidencyEdge::Write {
                operation: OperationId::new(1).unwrap(),
            }),
        };
        let predecessors = BTreeMap::from([
            (repeated, BTreeSet::from([entry, repeated])),
            (exit, BTreeSet::from([repeated])),
        ]);
        let dominators = BTreeMap::from([
            (entry, BTreeSet::from([entry])),
            (repeated, BTreeSet::from([entry, repeated])),
            (exit, BTreeSet::from([entry, repeated, exit])),
        ]);
        let check = |edge| {
            let repeated_operation = operation(edge);
            let blocks = BTreeMap::from([
                (entry, Vec::new()),
                (repeated, vec![&repeated_operation]),
                (exit, vec![&after_loop]),
            ]);
            happens_before_atomic_coherence_violation(
                &blocks,
                &predecessors,
                |producer, consumer| {
                    dominators
                        .get(&consumer)
                        .is_some_and(|set| set.contains(&producer))
                },
                entry,
                |_, _, root| Some((root, Vec::new())),
                |_, _, _, _| false,
                |_, _, _| None,
            )
        };
        assert_eq!(check(alternatives()), None);
        let claimed = AtomicResidencyEdge::PathAlternatives {
            sources: vec![
                AtomicResidencySource::Initial,
                AtomicResidencySource::Write { operation: writer },
            ],
        };
        assert_eq!(
            check(claimed),
            Some((
                repeated,
                0,
                AtomicCoherenceViolation::ReadsFrom(AtomicEdgeViolation::InvalidPathAlternatives)
            ))
        );
    }
}
