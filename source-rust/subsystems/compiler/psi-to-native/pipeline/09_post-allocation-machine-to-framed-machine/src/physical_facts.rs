//! Resource commitments derived from the selected physical alternatives before
//! encoding. Explicit audits compare them with independently decoded effects.

pub fn for_emission(
    frame: &crate::PhysicalFrameProgram,
    installations: &installation_admission::PlannedInstallationResources,
    semantic: &target_vocabulary::publication::EmissionProgramFacts,
) -> Result<physical_program::PhysicalEmissionFacts, &'static str> {
    let program = &frame.instructions;
    let layout = frame.frame.plan();
    let stack = crate::stack_demand::plan_physical_stack_program(frame)?;
    Ok(physical_program::PhysicalEmissionFacts {
        termination: installation_admission::selected::verify_privileged_termination(
            installations.installed_entry_execution(),
            program,
        )
        .map_err(|error| error.to_string()),
        faults: installation_admission::selected::fault_origins::capture(program),
        cpu: installation_admission::selected::cpu_origins::capture(program),
        internal_calls: crate::internal_call_plans::capture(program, layout, semantic),
        boundary_settlements: crate::boundary_settlements::capture(program),
        contracts: std::sync::Arc::clone(&program.contracts),
        port_writes: port_writes(program),
        temporary_stack: temporary_stack_bounds(program),
        stack,
        calls: std::sync::Arc::new(crate::call_origins::capture(program)),
        image_origins: crate::image_origins::capture(program.functions.iter()),
        foreign_calls: crate::foreign_call_plans::capture(program, layout).map(std::sync::Arc::new),
    })
}

pub fn temporary_stack_bounds(
    program: &post_allocation_machine::PostAllocationMachineProgram,
) -> Vec<machine_code::function_metadata::FunctionTemporaryStack> {
    program
        .machine
        .functions
        .iter()
        .map(|function| {
            let byte_count = function
                .instructions()
                .filter_map(|instruction| match instruction.alternative.encoded.stack {
                    selected_instructions::MachineEncodedStackEffect::TemporaryBytesV1 {
                        byte_count,
                        ..
                    } => Some(u32::from(byte_count)),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            machine_code::function_metadata::FunctionTemporaryStack {
                machine: function.machine,
                byte_count,
            }
        })
        .collect()
}

pub fn port_writes(
    program: &post_allocation_machine::PostAllocationMachineProgram,
) -> Result<Vec<machine_code::function_metadata::PhysicalPortWrite>, &'static str> {
    let mut rows = Vec::new();
    for function in program.functions.iter() {
        rows.extend(port_writes_for_function(
            function.machine,
            function.instructions(),
        )?);
    }
    Ok(rows)
}

pub fn port_writes_for_function<'a>(
    machine: semantic_vocabulary::MachineId,
    instructions: impl IntoIterator<Item = &'a selected_instructions::SelectedInstruction>,
) -> Result<Vec<machine_code::function_metadata::PhysicalPortWrite>, &'static str> {
    let mut rows = Vec::new();
    for instruction in instructions {
        let selected_instructions::SelectedInstructionKind::PortWrite {
            service,
            port,
            value,
        } = instruction.kind
        else {
            continue;
        };
        let [operation] = instruction.provenance.operations.as_slice() else {
            return Err("port event has no exact source occurrence");
        };
        rows.push(machine_code::function_metadata::PhysicalPortWrite {
            machine,
            instruction: instruction.id,
            operation: *operation,
            service,
            port,
            value,
        });
    }
    Ok(rows)
}
