//! Same-stack call alternatives from admitted instructions and selector tables.
//!
//! Relocations describe direct calls only. An encoded table call still keeps
//! its caller's frame live while any implementation of its exact slot runs.
//! Expand that finite target set for the ordinary maximum-depth calculation;
//! counting only fixups underallocates the task stack and can overwrite the
//! adjacent implementation tables. Explicit trap rows have no callee frame.

use super::{Error, MachineId};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::SelectedCallTarget;
use target_operations::{CallSiteOwner, NativeCallTarget};

#[derive(Clone, Copy)]
pub(in crate::function_fragments) struct CallStackEdge {
    pub owner: CallSiteOwner,
    pub callee: MachineId,
    pub section_offset: u64,
}

impl From<&machine_code::PlacedInternalMachineCallResolution> for CallStackEdge {
    fn from(call: &machine_code::PlacedInternalMachineCallResolution) -> Self {
        Self {
            owner: call.owner,
            callee: call.callee,
            section_offset: call.field_section_offset,
        }
    }
}

pub(in crate::function_fragments) fn stack_calls(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<Vec<CallStackEdge>, Error> {
    let text = source.source().text_section();
    let mut edges = text
        .resolved_internal_machine_calls
        .iter()
        .filter(|call| call.caller == machine)
        .map(CallStackEdge::from)
        .collect::<Vec<_>>();
    let emissions = source.source().source().source().source();
    let selected = emissions
        .selected_plan()
        .function(machine)
        .ok_or(Error::Mismatch("stack caller has no selected function"))?;
    let placed = text
        .function(machine)
        .ok_or(Error::Mismatch("stack caller has no placed function"))?;
    let catalog = &emissions
        .optimized_target()
        .optimized()
        .plan()
        .dynamic_dispatch;
    for call in &selected.calls {
        let NativeCallTarget::Dynamic {
            interface, slot, ..
        } = call.call.target
        else {
            continue;
        };
        let instruction = selected
            .instructions()
            .find(|instruction| instruction.id == call.instruction)
            .ok_or(Error::Mismatch("dynamic stack call has no instruction"))?;
        if instruction.kind.call_target() != Some(SelectedCallTarget::Dynamic { interface, slot }) {
            return Err(Error::Mismatch("dynamic stack call target changed"));
        }
        let span = placed
            .instruction_span(call.instruction)
            .ok_or(Error::Mismatch("dynamic stack call has no emitted span"))?;
        let position = usize::try_from(slot).map_err(|_| Error::Overflow)?;
        if catalog
            .interface(interface)
            .and_then(|interface| interface.slots.get(position))
            .is_none()
        {
            return Err(Error::Mismatch("dynamic stack call has no declared slot"));
        }
        let mut targets = Vec::new();
        for table in catalog
            .tables
            .iter()
            .filter(|table| table.interface == interface)
        {
            let target = table.rows.get(position).ok_or(Error::Mismatch(
                "dynamic stack call table is missing its slot",
            ))?;
            if let Some(target) = target {
                if text.function(*target).is_none() {
                    return Err(Error::Mismatch(
                        "dynamic stack call target has no emitted function",
                    ));
                }
                targets.push(*target);
            }
        }
        targets.sort();
        targets.dedup();
        edges.extend(targets.into_iter().map(|callee| CallStackEdge {
            owner: call.owner,
            callee,
            section_offset: span.section_offset,
        }));
    }
    Ok(edges)
}
