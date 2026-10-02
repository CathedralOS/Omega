//! Shared state arrivals pin their possible storage regions, not sibling fields.
//!
//! The ordinary borrowed-origin traversal composes every incoming projection.
//! Any overlapping possible origin rejects interference; that union never
//! grants access or restoration authority. Unknown origins and projections
//! remain conservative, including after another state forwards the loan.

use std::collections::{BTreeMap, BTreeSet};

use super::super::borrowed_windows::{
    BorrowedStorageOrigins, BorrowedStorageRegion, resolve_origin,
};
use super::super::structural::operations::{
    canonical_field_path, structural_field_store_write_path,
};
use super::super::{
    BlockId, CanonicalStructuralPathSegment, OperationKind, PlaceId, StructuralAccess,
    TerminalMachine, TerminalModule,
};

pub(super) struct SharedLoans<'a> {
    origins: &'a BorrowedStorageOrigins,
    regions: BTreeMap<BlockId, BTreeSet<BorrowedStorageRegion>>,
    unresolved: BTreeSet<BlockId>,
}

impl<'a> SharedLoans<'a> {
    pub(super) fn new(machine: &TerminalMachine, origins: &'a BorrowedStorageOrigins) -> Self {
        let mut regions = BTreeMap::new();
        let mut unresolved = BTreeSet::new();
        for block in &machine.blocks {
            let mut pinned = BTreeSet::new();
            for parameter in &block.structural_parameters {
                if parameter.access != StructuralAccess::SharedBorrow {
                    continue;
                }
                match origins.get(&parameter.place) {
                    Some(sources) if !sources.is_empty() => pinned.extend(sources.iter().cloned()),
                    _ => {
                        unresolved.insert(block.id);
                    }
                }
            }
            if !pinned.is_empty() {
                regions.insert(block.id, pinned);
            }
        }
        Self {
            origins,
            regions,
            unresolved,
        }
    }

    pub(super) fn disturbed(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        block: BlockId,
        operation: &terminal_psi::Operation,
        consumed: &[PlaceId],
    ) -> Option<PlaceId> {
        let pinned = self.regions.get(&block);
        if pinned.is_none() && !self.unresolved.contains(&block) {
            return None;
        }
        let conflicts = |place: PlaceId, sources: BTreeSet<BorrowedStorageRegion>| {
            (self.unresolved.contains(&block)
                || sources.is_empty()
                || sources.iter().any(|source| {
                    pinned.into_iter().flatten().any(|loan| {
                        loan.root == source.root
                            && (loan.path.starts_with(&source.path)
                                || source.path.starts_with(&loan.path))
                    })
                }))
            .then_some(place)
        };
        // Publication replaces the whole result storage on cyclic re-entry,
        // even for unrestricted producers with no disposal obligation.
        for place in consumed
            .iter()
            .copied()
            .chain(operation.result.structural().map(|result| result.place))
        {
            if let Some(place) = conflicts(place, self.regions_at(place, &[])) {
                return Some(place);
            }
        }
        for argument in super::operations::projected_arguments(operation) {
            if argument.access != StructuralAccess::SharedBorrow
                && let Some(place) = conflicts(
                    argument.place,
                    resolve_origin(module, machine, self.origins, argument),
                )
            {
                return Some(place);
            }
        }
        if let OperationKind::EstablishReference { source } = &operation.kind
            && source.access != StructuralAccess::SharedBorrow
            && let Some(place) = conflicts(
                source.place,
                resolve_origin(module, machine, self.origins, source),
            )
        {
            return Some(place);
        }
        let (place, region) = mutation_region(module, machine, operation)?;
        conflicts(place, self.regions_at(place, &region))
    }

    fn regions_at(
        &self,
        place: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> BTreeSet<BorrowedStorageRegion> {
        match self.origins.get(&place) {
            Some(origins) => origins
                .iter()
                .map(|origin| {
                    let mut region = origin.clone();
                    if region.exact {
                        region.path.extend_from_slice(path);
                    }
                    region
                })
                .collect(),
            None => BTreeSet::from([BorrowedStorageRegion {
                root: place,
                path: path.to_vec(),
                exact: true,
            }]),
        }
    }
}

fn mutation_region(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
    // Field writes and extraction already have one canonical region owner.
    // A failed projection must still interfere with the whole spelled root.
    let root = match &operation.kind {
        OperationKind::StructuralScalarFieldStore { destination, .. }
        | OperationKind::StructuralByteSequenceFieldStore { destination, .. }
        | OperationKind::StructuralByteViewFieldStore { destination, .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { destination, .. }
        | OperationKind::StoreStructuralField { destination, .. } => *destination,
        OperationKind::MoveStructuralField { source, .. } => *source,
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => {
            let prefix = path
                .iter()
                .position(|segment| segment.runtime_index().is_some())
                .unwrap_or(path.len());
            return Some((
                *destination,
                canonical_field_path(module, machine, *destination, &path[..prefix])
                    .unwrap_or_default(),
            ));
        }
        OperationKind::AtomicAccess {
            place,
            path,
            field,
            event,
            ..
        } if event.modifies_resident() => {
            // Atomic fields are declaration identities; canonicalization of
            // the carrier precedes attaching that exact leaf.
            let region = canonical_field_path(module, machine, *place, path)
                .map(|mut region| {
                    region.push(CanonicalStructuralPathSegment::Field(*field));
                    region
                })
                .unwrap_or_default();
            return Some((*place, region));
        }
        OperationKind::ByteSequenceStore { destination, .. }
        | OperationKind::ByteSequenceWrite { destination, .. }
        | OperationKind::ElementViewWrite { destination, .. } => {
            return Some((*destination, Vec::new()));
        }
        _ => return None,
    };
    Some(
        structural_field_store_write_path(module, machine, operation).unwrap_or((root, Vec::new())),
    )
}
