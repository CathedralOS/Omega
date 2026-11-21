//! Trace a loaded integer to the version the read actually observed. Whole
//! aggregate transfers take their source at the transfer's own instruction.

use std::collections::BTreeMap;

use semantic_vocabulary::{MachineId, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{TerminalMachine, TerminalModule};

use crate::descriptor_lifetimes::{DescriptorLocation, DescriptorPosition};
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorMachine, ResidentValueCapture, TerminalScalarCaptures,
    descriptor_machine, projected_stored_sources_in, terminal_borrowed_storage_graph_from,
    terminal_resident_value_transfers,
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Stored {
    Scalar(DescriptorPosition, ScalarTerm),
    Copy(DescriptorPosition, DescriptorLocation),
}

pub(super) struct StorageValues {
    graph: Vec<DescriptorMachine>,
    writes: BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, Stored)>>,
}

impl StorageValues {
    pub(super) fn new(module: &TerminalModule, machine: &TerminalMachine) -> Option<Self> {
        let graph = terminal_borrowed_storage_graph_from(module, [machine.id], &[]).ok()?;
        let row = descriptor_machine(&graph, machine.id)?;
        let mut writes: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (at, captured) in TerminalScalarCaptures::new(module, machine).ok()?.writes {
            if let Ok(values) = captured {
                writes.entry(at).or_default().extend(
                    values
                        .into_iter()
                        .map(|(place, term)| (place, Stored::Scalar(at, term))),
                );
            }
        }
        for (at, captured) in terminal_resident_value_transfers(machine, row).ok()? {
            if let ResidentValueCapture::Complete(values) = captured {
                writes.entry(at).or_default().extend(
                    values
                        .into_iter()
                        .map(|value| (value.destination, Stored::Copy(at, value.source))),
                );
            }
        }
        Some(Self { graph, writes })
    }

    pub(super) fn sources(
        &self,
        module: &TerminalModule,
        machine: MachineId,
        at: DescriptorPosition,
        value: ValueId,
        ty: ScalarType,
    ) -> Option<Vec<(DescriptorPosition, ScalarTerm)>> {
        let row = descriptor_machine(&self.graph, machine)?;
        let mut reads = row
            .scalar_payload_reads
            .get(&at)?
            .iter()
            .filter(|(_, term)| *term == ScalarTerm::value(value, ty));
        let (location, _) = reads.next()?;
        if reads.next().is_some() {
            return None;
        }
        let types = &module.types.structural_types;
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(types, &self.graph);
        let mut pending = vec![(at, location.clone())];
        let mut visited = std::collections::BTreeSet::new();
        let mut values = Vec::new();
        while let Some((at, location)) = pending.pop() {
            if !visited.insert((at, location.clone())) {
                // A loop-carried storage value needs an inductive proof.
                return None;
            }
            if visited.len() > 64 {
                return None;
            }
            let sources = projected_stored_sources_in(
                types,
                &self.graph,
                &mut analysis,
                machine,
                at,
                &location,
                |at| self.writes.get(&at).cloned().unwrap_or_default(),
                |source, suffix| match source {
                    Stored::Copy(at, mut from) => {
                        from.path.extend_from_slice(suffix);
                        Some(Stored::Copy(at, from))
                    }
                    Stored::Scalar(..) if suffix.is_empty() => Some(source),
                    _ => None,
                },
            )?;
            for source in sources {
                match source {
                    Stored::Scalar(at, term) => values.push((at, term)),
                    Stored::Copy(at, from) => pending.push((at, from)),
                }
            }
        }
        values.sort_unstable();
        values.dedup();
        (!values.is_empty()).then_some(values)
    }
}
