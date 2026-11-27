//! Project stack use from allocated instructions, frame geometry and closed
//! call contracts, before any body or frame bytes are encoded.

use super::{
    ForeignStackExtent, LocalStackExtent, PhysicalForeignStackCall, PhysicalStackCall,
    PhysicalStackFunction, PhysicalStackProgram,
};
use crate::PhysicalFrameProgram;
use crate::instruction_roster::SelectedInstructionRoster;
use selected_instructions::{MachineEncodedStackEffect, SelectedCallTarget};
use semantic_vocabulary::InvocationSite;
use target::Architecture;
use target_vocabulary::{CallSiteOwner, NativeCallTarget};
use terminal_semantics::callable_values::{CallableInvocationSite, CallableInvocationTarget};

pub fn plan_physical_stack_program(
    frame: &PhysicalFrameProgram,
) -> Result<PhysicalStackProgram, &'static str> {
    let program = &frame.instructions;
    let invocations = program
        .contracts
        .execution()
        .ok_or("current program has no prepared callable invocation context")?
        .callable_invocations();
    let catalog = program
        .contracts
        .publication()
        .code_catalogs()
        .dynamic_dispatch();
    let linkage = if program.target.architecture == Architecture::X86_64 {
        8
    } else {
        0
    };
    // Post-allocation validation keeps the selected and allocated function
    // rosters in one order, and frame layout derives its rows from the
    // allocated roster in that order, so row `index` of each names one machine.
    let layouts = &frame.frame.plan().functions;
    let mut functions = Vec::with_capacity(program.functions.len());
    for (index, function) in program.functions.iter().enumerate() {
        let layout = layouts
            .get(index)
            .filter(|row| row.machine == function.machine)
            .ok_or("physical stack function has no frame geometry")?;
        let machine = program
            .machine
            .functions
            .get(index)
            .filter(|row| row.machine == function.machine)
            .ok_or("physical stack function has no allocated instructions")?;
        let frame_bytes =
            u32::try_from(layout.frame_size_bytes).map_err(|_| "physical frame exceeds u32")?;
        let temporary = machine
            .instructions()
            .filter_map(|instruction| match instruction.alternative.encoded.stack {
                MachineEncodedStackEffect::TemporaryBytesV1 { byte_count, .. } => {
                    Some(u32::from(byte_count))
                }
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let mut local = frame_bytes
            .checked_add(temporary)
            .ok_or("physical stack peak overflows")?;
        let caller_live_bytes = frame_bytes
            .checked_add(linkage)
            .ok_or("physical call stack overflows")?;
        // Call records sorted by instruction (stably, so the first record for
        // an instruction stays first) replace a scan of every record per call.
        let mut calls_by_instruction = function.calls.iter().collect::<Vec<_>>();
        calls_by_instruction.sort_by_key(|call| call.instruction);
        let mut calls = Vec::new();
        for instruction in function.instructions_and_terminators() {
            let Some(selected_target) = instruction.kind.call_target() else {
                continue;
            };
            let first =
                calls_by_instruction.partition_point(|call| call.instruction < instruction.id);
            let call = calls_by_instruction
                .get(first)
                .copied()
                .filter(|call| call.instruction == instruction.id)
                .ok_or("physical call has no retained call contract")?;
            let targets = match (&call.call.target, selected_target) {
                (NativeCallTarget::Machine(target), SelectedCallTarget::Machine(selected))
                    if *target == selected =>
                {
                    vec![*target]
                }
                (
                    NativeCallTarget::Callable { callee, contract },
                    SelectedCallTarget::Callable { contract: selected },
                ) if *contract == selected => {
                    let site = call_site(call.owner)?;
                    let Some(invocation) = invocations.site(function.machine, site) else {
                        if invocations
                            .reached_machines
                            .binary_search(&function.machine)
                            .is_ok()
                        {
                            return Err("callable stack demand lost its reached occurrence");
                        }
                        continue;
                    };
                    if invocation.invocation.target
                        != CallableInvocationTarget::Value(*callee, *contract)
                    {
                        return Err("callable stack demand changed its evaluated code or contract");
                    }
                    if !invocation.targets_are_resolved() {
                        return Err(
                            "callable stack demand has unresolved code and no physical ceiling",
                        );
                    }
                    invocation.selected_machines.clone()
                }
                (
                    NativeCallTarget::Dynamic {
                        interface, slot, ..
                    },
                    SelectedCallTarget::Dynamic {
                        interface: selected,
                        slot: selected_slot,
                    },
                ) if *interface == selected && *slot == selected_slot => {
                    let position =
                        usize::try_from(*slot).map_err(|_| "dynamic slot exceeds usize")?;
                    if catalog
                        .interface(*interface)
                        .and_then(|row| row.slots.get(position))
                        .is_none()
                    {
                        return Err("dynamic stack call has no declared slot");
                    }
                    let mut targets = Vec::new();
                    for table in catalog
                        .tables
                        .iter()
                        .filter(|table| table.interface == *interface)
                    {
                        if let Some(target) = table
                            .rows
                            .get(position)
                            .ok_or("dynamic stack call table is missing its slot")?
                        {
                            targets.push(*target);
                        }
                    }
                    targets.sort_unstable();
                    targets.dedup();
                    targets
                }
                _ => return Err("physical call changed its retained target"),
            };
            for target in targets {
                if program.function(target).is_none() {
                    return Err("physical stack call target has no retained function");
                }
                calls.push(PhysicalStackCall {
                    target,
                    owner: call.owner,
                    caller_live_bytes,
                });
                local = local.max(caller_live_bytes);
            }
        }
        let roster = if function.normalized_foreign_calls.is_empty() {
            SelectedInstructionRoster::default()
        } else {
            SelectedInstructionRoster::new(function.blocks.iter())
        };
        let mut foreign_calls = Vec::new();
        for call in &function.normalized_foreign_calls {
            if !roster.contains(call.instruction) {
                return Err("physical foreign call has no retained instruction");
            }
            let binding = &call.call.binding;
            let extent = ForeignStackExtent::from_binding(binding);
            let committed = layout
                .frame_size_bytes
                .checked_sub(layout.red_zone_resident_bytes)
                .and_then(|bytes| {
                    bytes.checked_add(if binding.system_call().is_none() {
                        u64::from(linkage)
                    } else {
                        0
                    })
                })
                .ok_or("physical foreign stack extent overflows")?;
            foreign_calls.push(PhysicalForeignStackCall {
                owner: call.invocation_site.into(),
                caller_live_bytes: u32::try_from(committed)
                    .map_err(|_| "physical foreign stack exceeds u32")?,
                extent,
                invoked_callbacks: call
                    .call
                    .callbacks
                    .iter()
                    .filter(|callback| callback.registrar_invokes_callback)
                    .map(|callback| callback.callback_function)
                    .collect(),
            });
        }
        functions.push(PhysicalStackFunction {
            machine: function.machine,
            local: LocalStackExtent::Finite(u64::from(local)),
            calls,
            foreign_calls,
        });
    }
    Ok(PhysicalStackProgram {
        psi: program.psi,
        target: program.target,
        functions,
    })
}

fn call_site(owner: CallSiteOwner) -> Result<CallableInvocationSite, &'static str> {
    Ok(match owner {
        CallSiteOwner::Operation(operation) => {
            CallableInvocationSite::Invocation(InvocationSite::Operation(operation))
        }
        CallSiteOwner::InvokeNoNormal(edge) => {
            CallableInvocationSite::Invocation(InvocationSite::InvokeNoNormal(edge))
        }
        CallSiteOwner::ExternalCompletion(edge) => {
            CallableInvocationSite::Invocation(InvocationSite::ExternalCompletion(edge))
        }
        CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => CallableInvocationSite::CleanupAction {
            edge,
            action_ordinal,
        },
        CallSiteOwner::Installation { .. } => {
            return Err("installed call cannot replace a callable source occurrence");
        }
    })
}
