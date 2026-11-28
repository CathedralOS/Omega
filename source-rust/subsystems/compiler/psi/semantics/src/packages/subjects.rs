//! Compare borrowed package addresses and the historical values read by actual copies.
use super::*;
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorMachine, DescriptorPayloadTarget,
    require_unchanged_observed_storage, terminal_borrowed_storage_graph_from,
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

pub(super) fn copied_witness_at(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    first: &TerminalPackageOpening,
    second: &TerminalPackageOpening,
) -> bool {
    first.machine == machine.id
        && second.machine == machine.id
        && first.record == second.record
        && first.witnesses == second.witnesses
        && copies_value_at(
            module,
            machine,
            operation,
            first,
            second.subject.place,
            &second.subject.path,
        )
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
        let left = history.ancestry(opened.subject.place, &opened.subject.path);
        let right = history.ancestry(place, path);
        // Two labels for one slot do not establish a copy.
        left.first()
            .zip(right.first())
            .is_some_and(|(a, b)| a.home != b.home)
            && left
                .iter()
                .any(|a| right.iter().any(|b| history.same_value(a, b)))
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

    fn same_value(&self, a: &Observation, b: &Observation) -> bool {
        // Both operands begin at the current prestate. Each older prestate is
        // a pure StructuralLeafCopy read, so completing it preserves its input.
        a.home == b.home
            && (a.at == b.at
                || self.unchanged(&a.home, a.at, b.at)
                || self.unchanged(&a.home, b.at, a.at))
    }

    fn ancestry(&mut self, place: PlaceId, path: &[StructuralPathSegment]) -> Vec<Observation> {
        let mut chain = Vec::new();
        let mut next = self.observe(place, path, self.now);
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
