//! Measure an installed handler against its admitted entry and physical call graph.
//! Image publication independently checks the completed body and arrival bridge.
use installation_admission::selected::correlate_native_instruction_sources;
use installation_evidence::{InstalledBodyFaultPremises, InstalledInterruptStackRequirement};
use selected_instructions::{MachineEncodedTrapBehavior as Trap, SelectedInstructionKind as Kind};

#[derive(Debug)]
pub enum Error {
    Mismatch(&'static str),
    Overflow,
    InstalledFaultSurface {
        machine: semantic_vocabulary::MachineId,
        instruction: selected_instructions::SelectedInstructionId,
        detail: String,
    },
    Memory(installation_admission::ram::Error),
    Selected(installation_admission::selected::Error),
    Cpu(installation_admission::cpu::Error),
    Demand(frame_planning::stack_demand::StackDemandError),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch(reason) => f.write_str(reason),
            Self::Overflow => f.write_str("shared object coordinate exceeds supported size"),
            Self::InstalledFaultSurface {
                machine,
                instruction,
                detail,
            } => write!(
                f,
                "installed handler {machine:?} instruction {instruction:?} has no software-fault exclusion: {detail}"
            ),
            Self::Memory(error) => error.fmt(f),
            Self::Selected(error) => error.fmt(f),
            Self::Cpu(error) => error.fmt(f),
            Self::Demand(error) => write!(f, "compiler-private function custody: {error:?}"),
        }
    }
}
impl std::error::Error for Error {}

/// A physical measurement of this admitted private handler before arrival composition.
#[derive(Debug, Clone)]
pub struct MeasuredInstalledHandler(InstalledInterruptStackRequirement);
impl MeasuredInstalledHandler {
    pub fn requirement(&self) -> &InstalledInterruptStackRequirement {
        &self.0
    }
    pub fn into_requirement(self) -> InstalledInterruptStackRequirement {
        self.0
    }
}

pub fn measure_installed_handler(
    identity: function_identity::MachineFunctionIdentity,
    frame: &frame_planning::PhysicalFrameProgram,
    facts: &physical_program::EmissionFacts,
    authority: &installation_admission::PlannedInstallationResources,
    bridge: &entry_planning::native_entry_bridge::NativeEntryBridgePlan,
    cpu: &entry_planning::body_cpu::PlannedBodyCpuState,
) -> Result<MeasuredInstalledHandler, Error> {
    let execution = authority
        .installed_entry_execution()
        .ok_or(Error::Mismatch(
            "installed body stack demand has no exact execution domain",
        ))?;
    let placement_index = identity
        .callback_thunk_placement_index()
        .ok_or(Error::Mismatch(
            "installed stack demand has no private placement",
        ))?;
    let program = &frame.instructions;
    if function_identity::MachineFunctionIdentity::callback_thunk(program.psi, placement_index)
        != Some(identity)
        || execution.entry().callback_function != identity
        || execution.entry().placement_index != placement_index
        || execution.entry().entry().machine() != program.entry
        || execution.entry().entry().inbound() != &bridge.request().inbound
        || facts.physical.calls.physical_identity != program.identity
        || facts.physical.stack.psi != program.psi
        || facts.physical.stack.target != program.machine.target
        || bridge.request().target != program.machine.target
    {
        return Err(Error::Mismatch(
            "installed stack demand changed its original private body",
        ));
    }
    cpu.require_body(
        frame,
        program.entry,
        bridge.request().inbound.state.preemption,
    )
    .map_err(Error::Cpu)?;
    let entry_plan = bridge.installed_interrupt().ok_or(Error::Mismatch(
        "installed stack demand has no admitted arrival bridge",
    ))?;
    let body = frame_planning::stack_demand::derive_physical_stack_demand(
        &facts.physical.stack,
        program.entry,
        Vec::new(),
    )
    .map_err(Error::Demand)?;
    let requirement = InstalledInterruptStackRequirement {
        installation_machine: execution.scope().machine(),
        arrival: None,
        fault_exclusion: plan_fault_exclusion(frame, facts, authority, cpu)?,
        psi: program.psi,
        target: program.machine.target,
        operation: execution.scope().operation(),
        placement_index,
        entry: program.entry,
        entry_plan,
        body_bytes: body.ceiling_bytes().ok_or(Error::Mismatch(
            "installed stack requires a finite demand without hosted foreign calls",
        ))?,
        bridge_bytes: bridge.frame_bytes(),
        call_stack_bias: bridge.call_stack_bias(),
        alignment: body
            .stack_alignment()
            .max(u32::from(bridge.request().internal.stack_alignment)),
    };
    requirement.required_bytes().ok_or(Error::Overflow)?;
    Ok(MeasuredInstalledHandler(requirement))
}

fn plan_fault_exclusion(
    frame: &frame_planning::PhysicalFrameProgram,
    facts: &physical_program::EmissionFacts,
    authority: &installation_admission::PlannedInstallationResources,
    cpu: &entry_planning::body_cpu::PlannedBodyCpuState,
) -> Result<Option<InstalledBodyFaultPremises>, Error> {
    let execution = authority
        .installed_entry_execution()
        .ok_or(Error::Mismatch(
            "installed body stack demand has no exact execution domain",
        ))?;
    if execution.scope().fatal_entries().is_empty() {
        return Ok(None);
    }
    if execution.scope().interrupt_delivery_assumption().is_none() {
        return Err(Error::Mismatch(
            "installed fatal closure lacks its delivery condition",
        ));
    }
    if frame
        .instructions
        .functions
        .iter()
        .any(|function| !function.normalized_foreign_calls.is_empty())
    {
        return Err(Error::Mismatch(
            "installed quiet closure has an unresolved foreign or private execution dependency",
        ));
    }
    let selected = &frame.instructions;
    let physical = &selected.machine;
    if selected.psi != facts.semantic.program.psi
        || selected.entry != facts.semantic.program.entry
        || physical.target != authority.program().target
    {
        return Err(Error::Mismatch(
            "installed fault analysis changed its body or target",
        ));
    }
    let correlated = correlate_native_instruction_sources(
        selected,
        physical.functions.iter().flat_map(|function| {
            function
                .instructions()
                .map(move |instruction| (function.machine, instruction.instruction))
        }),
    )
    .map_err(Error::Selected)?;
    let mut machines: Vec<_> = selected
        .functions
        .iter()
        .map(|function| function.machine)
        .collect();
    machines.sort_unstable();
    let mut physical_machines: Vec<_> = physical
        .functions
        .iter()
        .map(|function| function.machine)
        .collect();
    physical_machines.sort_unstable();
    if machines != physical_machines || machines.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::Mismatch(
            "installed fault analysis changed its exact function roster",
        ));
    }
    installation_admission::ram::validate(
        &facts.semantic.program,
        &facts.physical.calls,
        authority.installed_interrupts(),
        execution,
        &machines,
    )
    .map_err(Error::Memory)?;
    cpu.require_without_floating_state().map_err(Error::Cpu)?;
    let mut correlated = correlated.into_iter();
    for function in &physical.functions {
        for instruction in function.instructions() {
            let (machine, source) = correlated.next().ok_or(Error::Mismatch(
                "installed physical instruction has no exact selected source",
            ))?;
            let fail = |reason: &str| Error::InstalledFaultSurface {
                machine,
                instruction: instruction.instruction,
                detail: format!(
                    "{reason}; selected={:?}, encoded={:?}",
                    source.kind, instruction.alternative.encoded
                ),
            };
            if machine != function.machine {
                return Err(fail("physical instruction changed its owning machine"));
            }
            if let Some(target) = source.kind.call_target() {
                let Some(target) = target.machine() else {
                    return Err(fail("dynamic call has no closed fault analysis"));
                };
                if machines.binary_search(&target).is_err() {
                    return Err(fail("callee is outside the exact retained body closure"));
                }
            }
            let effects = &instruction.alternative.encoded;
            match effects.trap {
                Trap::NeverV1 => {}
                Trap::MayArchitecturalFaultV1 => {
                    let relative = matches!(
                        source.kind,
                        Kind::Jump
                            | Kind::ConditionalBranchNonZero
                            | Kind::ConditionalBranchU64LessThan
                            | Kind::ConditionalBranchI64LessThan
                    );
                    if !relative
                        && installation_admission::selected::fault_origins::classify(&source.kind)
                            == machine_code::fault_origins::FaultOperation::Other
                    {
                        return Err(fail(
                            "architectural fault source is not discharged by checked RAM/stack or CPL0",
                        ));
                    }
                }
                _ => {
                    return Err(fail(
                        "instruction retains an architectural trap or hosted failure route",
                    ));
                }
            }
        }
    }
    if correlated.next().is_some() {
        return Err(Error::Mismatch(
            "installed physical closure omits a selected instruction",
        ));
    }
    Ok(Some(InstalledBodyFaultPremises {
        physical_program_identity: physical.identity.bytes(),
    }))
}
