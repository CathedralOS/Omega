//! Correlate every chosen physical instruction with its selected fault source.
use super::correlate_native_instruction_sources;
use selected_instructions::SelectedInstructionKind as Kind;

pub fn capture(
    selected: &post_allocation_machine::PostAllocationMachineProgram,
) -> Result<machine_code::fault_origins::PhysicalFaultOrigins, String> {
    use machine_code::fault_origins::*;
    let physical = &selected.machine;
    let correlated = correlate_native_instruction_sources(
        selected,
        physical.functions.iter().flat_map(|function| {
            function
                .instructions()
                .map(move |row| (function.machine, row.instruction))
        }),
    )
    .map_err(|error| error.to_string())?;
    let mut machines: Vec<_> = selected
        .functions
        .iter()
        .map(|function| function.machine)
        .collect();
    let mut physical_machines: Vec<_> = physical
        .functions
        .iter()
        .map(|function| function.machine)
        .collect();
    machines.sort_unstable();
    physical_machines.sort_unstable();
    if machines != physical_machines || machines.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("installed fault analysis changed its exact function roster".into());
    }
    let mut correlated = correlated.into_iter();
    let mut functions = Vec::with_capacity(physical.functions.len());
    for function in &physical.functions {
        let mut instructions = Vec::new();
        for row in function.instructions() {
            let (machine, instruction) = correlated
                .next()
                .ok_or("installed physical instruction has no exact selected source")?;
            if machine != function.machine {
                return Err("physical instruction changed its owning machine".into());
            }
            let operation = classify(&instruction.kind);
            instructions.push(PhysicalInstructionFaultOrigin {
                instruction: row.instruction,
                operation,
                description: format!("{:?}", instruction.kind),
            });
        }
        functions.push(PhysicalFunctionFaultOrigins {
            machine: function.machine,
            instructions,
        });
    }
    if correlated.next().is_some() {
        return Err("installed physical closure omits a selected instruction".into());
    }
    Ok(PhysicalFaultOrigins {
        psi: selected.psi,
        entry: selected.entry,
        target: physical.target,
        physical_identity: selected.identity,
        functions,
    })
}

pub fn classify(kind: &Kind) -> machine_code::fault_origins::FaultOperation {
    use machine_code::fault_origins::FaultOperation;
    let memory = matches!(
        *kind,
        Kind::CopyBytes
            | Kind::ZeroBytes
            | Kind::LoadEncodedScalar
            | Kind::StoreEncodedScalar
            | Kind::ScanByteSequence { .. }
            | Kind::CompareByteSequences
            | Kind::LoadPacked { .. }
            | Kind::StorePacked { .. }
            | Kind::Store { .. }
            | Kind::Load8Indexed
            | Kind::Load16Indexed
            | Kind::Load32Indexed
            | Kind::Load64Indexed
            | Kind::Load8 { .. }
            | Kind::Load16 { .. }
            | Kind::Load32 { .. }
            | Kind::Load64 { .. }
            | Kind::Store64 { .. }
            | Kind::AtomicAccess { .. }
            | Kind::ReturnUnit
            | Kind::ReturnScalar
            | Kind::ReturnAggregate { .. }
    );
    let privileged = match *kind {
        Kind::PrivilegedCrash | Kind::PortWrite { .. } => true,
        Kind::RegisterAccess { operation } => matches!(
            operation,
            terminal_psi::RegisterOperation::ProcessorHalt
                | terminal_psi::RegisterOperation::InterruptControl(
                    language_core::inline_assembly::AsmInterruptControlKind::Disable
                )
                | terminal_psi::RegisterOperation::MemoryFence(_)
                | terminal_psi::RegisterOperation::PortWrite
        ),
        _ => false,
    };

    if let Some(target) = kind.call_target() {
        FaultOperation::Call(target)
    } else if memory {
        FaultOperation::MemoryOrStack
    } else if privileged {
        FaultOperation::Privileged
    } else {
        FaultOperation::Other
    }
}
