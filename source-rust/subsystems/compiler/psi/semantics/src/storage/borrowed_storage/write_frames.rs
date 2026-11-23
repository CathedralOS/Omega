//! Infer which parameter fields a machine can write and check any frame it publishes.

use crate::descriptor_results::{DescriptorAnalysis, DescriptorGraphError, DescriptorSlotKind};
use arena::IdTable;
use semantic_vocabulary::MachineId;

/// A bodyless contract belongs to the program. Every error in a body names its machine.
#[derive(Debug)]
pub struct WriteFrameError {
    pub machine: Option<MachineId>,
    pub detail: &'static str,
}

impl WriteFrameError {
    fn at(machine: MachineId, detail: &'static str) -> Self {
        Self {
            machine: Some(machine),
            detail,
        }
    }
}

impl std::fmt::Display for WriteFrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.detail)?;
        if let Some(machine) = self.machine {
            write!(f, " in {machine:?}")?;
        }
        Ok(())
    }
}

/// An unreadable body cannot publish a precise write frame. Independent bodies
/// can still publish the paths they write through their parameters.
pub fn inferred_write_frames(
    module: &terminal_psi::TerminalModule,
) -> Result<IdTable<MachineId, terminal_psi::PublishedWriteFrame>, WriteFrameError> {
    derive_write_frames(module, &|_| Ok(()))
}

fn derive_write_frames(
    module: &terminal_psi::TerminalModule,
    unreadable: &dyn Fn(DescriptorGraphError) -> Result<(), WriteFrameError>,
) -> Result<IdTable<MachineId, terminal_psi::PublishedWriteFrame>, WriteFrameError> {
    let mut frames = IdTable::default();
    for machine in &module.machines {
        if frames.push(None) != machine.id {
            return Err(WriteFrameError::at(
                machine.id,
                "write-frame machines are not at their declared positions",
            ));
        }
    }
    let mut graph = Vec::new();
    for row in crate::descriptor_results::terminal_graph_rows(
        module,
        module.machines.iter().map(|machine| machine.id),
        DescriptorSlotKind::BorrowedStorage,
        &[],
    ) {
        match row {
            Ok(body) => graph.push(body),
            Err(error @ DescriptorGraphError::IncompleteMachine { .. }) => unreadable(error)?,
            Err(DescriptorGraphError::MissingOrAmbiguousMachine(machine)) => {
                return Err(WriteFrameError::at(
                    machine,
                    "write frame has no unique machine",
                ));
            }
        }
    }
    // Later local summaries may strengthen scalar entry and result facts.
    // They must not silently narrow a published storage-effect contract.
    // Retain the actual alias/custody flow and infer a conservative frame
    // without those scalar premises on either producer or receiver.
    for machine in &mut graph {
        machine.payload_entry_predicates = None;
        machine.payload_scalar_postconditions.clear();
        machine.scalar_payload_writes.clear();
    }
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.types.structural_types, &graph);
    for body in &graph {
        let machine = frame_owner(module, body.id)?;
        let frame = machine_write_frame(machine, &mut analysis)
            .map_err(|detail| WriteFrameError::at(machine.id, detail))?;
        *frames
            .get_mut(machine.id)
            .ok_or_else(|| WriteFrameError::at(machine.id, "write-frame owner disappeared"))? =
            frame;
        analysis.release_storage_snapshots();
    }
    Ok(frames)
}

// Frame inference checked every machine ID against its table position before reading bodies.
fn frame_owner(
    module: &terminal_psi::TerminalModule,
    id: MachineId,
) -> Result<&terminal_psi::TerminalMachine, WriteFrameError> {
    usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| module.machines.get(position))
        .ok_or_else(|| WriteFrameError::at(id, "write frame has no source machine"))
}

fn machine_write_frame(
    machine: &terminal_psi::TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
) -> Result<terminal_psi::PublishedWriteFrame, &'static str> {
    use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;

    let Some(written) = analysis.write_frame(machine.id)? else {
        return Ok(None);
    };
    let mut paths = Vec::with_capacity(written.len());
    for region in written {
        let mut parameters = machine
            .structural_parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.place == region.root);
        let (position, _) = parameters
            .next()
            .ok_or("write frame names storage outside its structural formals")?;
        if parameters.next().is_some() {
            return Err("write frame has an ambiguous structural formal");
        }
        let position = u32::try_from(position)
            .map_err(|_| "write-frame formal ordinal cannot be represented")?;
        let mut fields = Vec::with_capacity(region.path.len());
        for segment in &region.path {
            let DescriptorPathSegment::Field(field) = segment else {
                return Err("published write frame contains a non-field step");
            };
            fields.push(*field);
        }
        paths.push((position, fields));
    }
    paths.sort_unstable();
    paths.dedup();
    Ok(Some(paths))
}

/// A supplied frame is usable only when the canonical body derives it.
/// Omitting a summary keeps the ceiling and grants no precision. A
/// bodyless header cannot invent an empty or narrowed write promise.
pub fn validate_write_frames(module: &terminal_psi::TerminalModule) -> Result<(), WriteFrameError> {
    if module
        .execution
        .declaration_contracts
        .iter()
        .any(|contract| contract.published_writes.is_some())
    {
        return Err(WriteFrameError {
            machine: None,
            detail: "a bodyless contract claims an inferred write frame",
        });
    }
    if module
        .machines
        .iter()
        .all(|machine| machine.contract.published_writes.is_none())
    {
        return Ok(());
    }
    let inferred = derive_write_frames(module, &|error| {
        let machine = error.machine();
        if frame_owner(module, machine)?
            .contract
            .published_writes
            .is_some()
        {
            return Err(WriteFrameError::at(
                machine,
                error.detail().unwrap_or(
                    "cannot read the storage used by a body with a published write frame",
                ),
            ));
        }
        Ok(())
    })?;
    for machine in &module.machines {
        let Some(published) = &machine.contract.published_writes else {
            continue;
        };
        let Some(writes) = inferred.get(machine.id).and_then(Option::as_ref) else {
            return Err(WriteFrameError::at(
                machine.id,
                "cannot infer the writes of a body with a published write frame",
            ));
        };
        if writes != published {
            return Err(WriteFrameError::at(
                machine.id,
                "published write frame differs from its body",
            ));
        }
    }
    Ok(())
}
