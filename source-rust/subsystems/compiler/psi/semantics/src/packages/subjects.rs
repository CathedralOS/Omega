//! Compare borrowed package addresses and the historical values read by actual copies.
use super::*;
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorMachine, DescriptorPayloadTarget, ResidentValueCapture,
    ResidentValueCaptures, projected_stored_sources_in, require_unchanged_between_inputs,
    require_unchanged_entry_storage, require_unchanged_observed_storage,
    terminal_borrowed_storage_graph_from, terminal_resident_value_transfers,
};
use crate::descriptors::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, descriptor_location, descriptor_location_type,
};
use semantic_vocabulary::PlaceId;
use std::cell::OnceCell;
use terminal_psi::StructuralMultiplicity;

pub(super) fn same_storage_at(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    opened: &TerminalPackageOpening,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    let anchor = &opened.subject;
    if anchor.place == place && crate::structural_paths_select_same_location(&anchor.path, path) {
        return true;
    }
    with_history(module, machine, operation, opened.record, |history| {
        history
            .observe(anchor.place, &anchor.path, history.now)
            .is_some_and(|recorded| history.observe(place, path, history.now) == Some(recorded))
    })
}

/// A witness belongs to the value observed at an opening, even after its
/// storage is reassigned. Each anchored use names its own instruction.
pub(super) fn observed_witness(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    current: u32,
    anchor: (u32, OperationId),
) -> bool {
    let rows = &module.execution.packages.openings;
    let row = &rows[current as usize];
    let original = &rows[anchor.0 as usize];
    if !module
        .structural_type(row.record)
        .is_some_and(|ty| ty.multiplicity == StructuralMultiplicity::Unrestricted)
    {
        return false;
    }
    let Some(at) = position(machine, anchor.1) else {
        return false;
    };
    with_history(module, machine, operation, row.record, |history| {
        let earlier = history.ancestry(original.subject.place, &original.subject.path, at);
        let current_value = history.ancestry(row.subject.place, &row.subject.path, history.now);
        history.related(&earlier, &current_value, current != anchor.0)
    })
}

/// Compare a borrowed operand's package with the selected callback's package.
/// A real whole-value copy can retain the witness at another address.
pub(super) fn copies_value_at(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    opened: &TerminalPackageOpening,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    if !module
        .structural_type(opened.record)
        .is_some_and(|ty| ty.multiplicity == StructuralMultiplicity::Unrestricted)
    {
        return false;
    }
    with_history(module, machine, operation, opened.record, |history| {
        let left = history.ancestry(opened.subject.place, &opened.subject.path, history.now);
        let right = history.ancestry(place, path, history.now);
        history.related(&left, &right, true)
    })
}

fn position(machine: &TerminalMachine, id: OperationId) -> Option<DescriptorPosition> {
    machine.blocks.iter().find_map(|block| {
        let offset = block.operations.iter().position(|op| op.id == id)?;
        Some((block.id, u32::try_from(offset).ok()?))
    })
}

fn with_history(
    module: &TerminalModule,
    machine: &TerminalMachine,
    op: &Operation,
    record: StructuralTypeId,
    check: impl FnOnce(&mut StorageHistory<'_>) -> bool,
) -> bool {
    let Some(now) = position(machine, op.id) else {
        return false;
    };
    let Ok(graph) = terminal_borrowed_storage_graph_from(module, [machine.id], &[]) else {
        return false;
    };
    let types = &module.types.structural_types;
    let replay = DescriptorAnalysis::with_borrowed_storage_slots(types, &graph);
    check(&mut StorageHistory {
        machine,
        types,
        graph: &graph,
        replay,
        transfers: OnceCell::new(),
        now,
        record,
    })
}

/// A copy observes its source immediately before the copy instruction.
#[derive(PartialEq, Eq)]
struct Observation {
    home: DescriptorLocation,
    at: DescriptorPosition,
}

struct StorageHistory<'a> {
    machine: &'a TerminalMachine,
    types: &'a [terminal_psi::StructuralTypeDeclaration],
    graph: &'a [DescriptorMachine],
    replay: DescriptorAnalysis<'a>,
    transfers: OnceCell<Option<ResidentValueCaptures>>,
    now: DescriptorPosition,
    record: StructuralTypeId,
}

impl StorageHistory<'_> {
    fn observe(
        &mut self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        at: DescriptorPosition,
    ) -> Option<Observation> {
        // The spelling of a runtime selector does not prove what an earlier
        // reference captured. Only one resolved static address can match.
        if path
            .iter()
            .any(|step| matches!(step, StructuralPathSegment::RuntimeIndex { .. }))
        {
            return None;
        }
        let root = self.machine.structural_place_type(place)?;
        if crate::runtime_structural_path_tip(self.types.iter(), root, path)?.id != self.record {
            return None;
        }
        let slot = descriptor_location(self.types, place, root, path, None)?;
        self.observe_slot(&slot, at)
    }

    fn observe_slot(
        &mut self,
        slot: &DescriptorLocation,
        at: DescriptorPosition,
    ) -> Option<Observation> {
        let root = self.machine.structural_place_type(slot.root)?;
        if descriptor_location_type(self.types, root, &slot.path)? != self.record {
            return None;
        }
        let location = self.replay.location(self.machine.id, at, slot)?;
        let home = DescriptorLocation::captured(location.root, &location.path)?;
        Some(Observation { home, at })
    }

    fn related(
        &mut self,
        left: &[Observation],
        right: &[Observation],
        requires_copy: bool,
    ) -> bool {
        let (Some(a), Some(b)) = (left.first(), right.first()) else {
            return false;
        };
        if requires_copy && a.home == b.home {
            return false;
        }
        left.iter()
            .any(|a| right.iter().any(|b| self.same_value(a, b)))
    }

    fn same_value(&mut self, a: &Observation, b: &Observation) -> bool {
        if a.home != b.home {
            return false;
        }
        let target = DescriptorPayloadTarget::current(a.home.clone());
        let mut unchanged = |from, to| {
            require_unchanged_between_inputs(
                self.types,
                self.graph,
                &mut self.replay,
                self.machine.id,
                from,
                to,
                &target,
            )
            .is_ok()
        };
        if a.at == b.at || unchanged(a.at, b.at) || unchanged(b.at, a.at) {
            return true;
        }
        // Neither branch-local read need dominate the other. Both must still
        // see the value supplied by the same parameter or producing instruction.
        let Some(place) = self.machine.structural_place(a.home.root) else {
            return false;
        };
        match place.kind {
            semantic_vocabulary::StructuralPlaceKind::Parameter { .. } => {
                [a.at, b.at].into_iter().all(|at| {
                    require_unchanged_entry_storage(
                        self.types,
                        self.graph,
                        &mut self.replay,
                        self.machine.id,
                        at,
                        &target,
                    )
                    .is_ok()
                })
            }
            semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } => {
                let Some(definition) = position(self.machine, producer) else {
                    return false;
                };
                [a.at, b.at].into_iter().all(|at| {
                    require_unchanged_observed_storage(
                        self.types,
                        self.graph,
                        self.machine.id,
                        definition,
                        at,
                        &target,
                    )
                    .is_ok()
                })
            }
            _ => false,
        }
    }

    fn ancestry(
        &mut self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        at: DescriptorPosition,
    ) -> Vec<Observation> {
        let mut chain = Vec::new();
        let mut next = self.observe(place, path, at);
        while let Some(observed) = next {
            if chain.contains(&observed) {
                break;
            }
            next = self.copied_from(&observed);
            chain.push(observed);
        }
        chain
    }

    fn copied_from(&mut self, destination: &Observation) -> Option<Observation> {
        let transfers = self
            .transfers
            .get_or_init(|| {
                let row = self.graph.iter().find(|row| row.id == self.machine.id)?;
                terminal_resident_value_transfers(self.machine, row).ok()
            })
            .as_ref()?;
        // The same storage query used by scalar observations follows a whole
        // copy, aggregate field or replacement only if it reaches this read.
        let sources = projected_stored_sources_in(
            self.types,
            self.graph,
            &mut self.replay,
            self.machine.id,
            destination.at,
            &destination.home,
            |at| match transfers.get(&at) {
                Some(ResidentValueCapture::Complete(values)) => values
                    .iter()
                    .map(|value| (value.destination.clone(), (at, value.source.clone())))
                    .collect(),
                _ => Vec::new(),
            },
            |(at, mut source), suffix| {
                source.path.extend_from_slice(suffix);
                Some((at, source))
            },
        )?;
        let [(at, source)] = sources.as_slice() else {
            return None;
        };
        self.observe_slot(source, *at)
    }
}
