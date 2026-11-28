//! Compare borrowed package addresses and the historical values read by actual copies.
use super::*;
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorMachine, DescriptorPayloadTarget,
    require_unchanged_between_inputs, require_unchanged_observed_storage,
    terminal_borrowed_storage_graph_from,
};
use crate::descriptors::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, descriptor_location,
};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
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
        self.observe_field(place, path, None, at)
    }

    fn observe_field(
        &mut self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
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
        let tip = crate::runtime_structural_path_tip(self.types.iter(), root, path)?;
        let selected = match field {
            None => tip.id,
            Some(field) => {
                let mut matching = tip.shape.fields().filter(|row| row.id == field);
                let ty = matching.next()?.field_type.as_structural()?;
                if matching.next().is_some() {
                    return None;
                }
                ty
            }
        };
        if selected != self.record {
            return None;
        }
        let slot = descriptor_location(self.types, place, root, path, field)?;
        let location = self.replay.location(self.machine.id, at, &slot)?;
        let home = DescriptorLocation::captured(location.root, &location.path)?;
        Some(Observation { home, at })
    }

    fn unchanged(
        &self,
        home: &DescriptorLocation,
        from: DescriptorPosition,
        to: DescriptorPosition,
    ) -> bool {
        require_unchanged_observed_storage(
            self.types,
            self.graph,
            self.machine.id,
            from,
            to,
            &DescriptorPayloadTarget::current(home.clone()),
        )
        .is_ok()
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
        a.at == b.at || unchanged(a.at, b.at) || unchanged(b.at, a.at)
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
        if let Some(source) = self.initial_copy(destination) {
            return Some(source);
        }
        // A whole-value store transfers the value observed at its input. It
        // must reach this read unchanged along every incoming control path.
        let machine = self.machine;
        for block in &machine.blocks {
            for (offset, op) in block.operations.iter().enumerate() {
                let OperationKind::StoreStructuralField {
                    destination: root,
                    path,
                    field,
                    value,
                    ..
                } = &op.kind
                else {
                    continue;
                };
                let at = (block.id, u32::try_from(offset).ok()?);
                if value.access == StructuralAccess::Owned
                    && at != destination.at
                    && self
                        .observe_field(*root, path, *field, at)
                        .is_some_and(|target| target.home == destination.home)
                    && self.unchanged(&destination.home, at, destination.at)
                {
                    return self.observe(value.place, &value.path, at);
                }
            }
        }
        None
    }

    fn initial_copy(&mut self, destination: &Observation) -> Option<Observation> {
        if !destination.home.path.is_empty() {
            return None;
        }
        let place = self
            .machine
            .structural_places
            .iter()
            .find(|place| place.id == destination.home.root)?;
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = place.kind
        else {
            return None;
        };
        let op = self.machine.operation(producer)?;
        let result = op.result.structural()?;
        if structural_type != self.record
            || result.structural_type != self.record
            || result.place != place.id
        {
            return None;
        }
        let OperationKind::StructuralLeafCopy { source, path } = &op.kind else {
            return None;
        };
        let at = position(self.machine, producer)?;
        if !self.unchanged(&destination.home, at, destination.at) {
            return None;
        }
        self.observe(*source, path, at)
    }
}
