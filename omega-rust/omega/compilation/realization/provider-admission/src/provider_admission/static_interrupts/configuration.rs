//! Keep the CPU configuration used to recover installed context stable.
//! Borrowed table and TSS storage cannot prevent a separate instruction from
//! replacing a CPU register, so inspect the complete selected execution closure.

use language_core::inline_assembly::AsmGlobalDescriptorTableOperation;
use language_core::static_interrupts::StaticInterruptOperation;
use semantic_vocabulary::{BoundaryMachineId, IdSet, MachineId, ServiceId};
use terminal_psi::{OperationKind, RegisterOperation, TerminalModule};

pub(super) struct ConfigurationLease {
    machine_control: ServiceId,
    bodies: IdSet<MachineId>,
    boundaries: IdSet<BoundaryMachineId>,
    masked_bodies: IdSet<MachineId>,
}

impl ConfigurationLease {
    pub(super) fn new(machine_control: ServiceId) -> Self {
        Self {
            machine_control,
            bodies: IdSet::default(),
            boundaries: IdSet::default(),
            masked_bodies: IdSet::default(),
        }
    }

    pub(super) fn require(
        &mut self,
        module: &TerminalModule,
        closure: &terminal_installation::SynchronousCallClosure<'_>,
    ) -> Result<(), String> {
        for body in &closure.machines {
            if !self.bodies.insert(body.id) {
                continue;
            }
            for operation in body.blocks.iter().flat_map(|block| &block.operations) {
                let replaces_configuration = match &operation.kind {
                    OperationKind::ActivateTaskRegister { .. }
                    | OperationKind::ReloadSegmentDescriptors { .. } => true,
                    OperationKind::GlobalDescriptorTable { operation, .. } => {
                        *operation == AsmGlobalDescriptorTableOperation::Load
                    }
                    OperationKind::RegisterAccess { event, .. } => {
                        changes_configuration(event.operation())
                    }
                    _ => false,
                };
                if replaces_configuration {
                    return Err(format!(
                        "installed context requires stable CPU configuration; machine {:?} operation {:?} can replace it",
                        body.id, operation.id,
                    ));
                }
            }
        }
        for &boundary in &closure.boundaries {
            if !self.boundaries.insert(boundary) {
                continue;
            }
            let declaration = module
                .boundary_machine(boundary)
                .ok_or("installed execution reaches a missing boundary")?;
            let intrinsic = declaration
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.execution.static_interrupt_operation());
            match intrinsic {
                Some(StaticInterruptOperation::Install) => {
                    return Err("installed execution can publish another table before retiring the live root".into());
                }
                Some(
                    StaticInterruptOperation::Context
                    | StaticInterruptOperation::BeginCompletion
                    | StaticInterruptOperation::FinishCompletion,
                ) => {}
                None if declaration
                    .published_service_ceiling
                    .contains(&self.machine_control) =>
                {
                    return Err(format!(
                        "installed context requires a checked configuration-preserving body for machine-control boundary {boundary:?}",
                    ));
                }
                None => {}
            }
        }
        Ok(())
    }
    /// Interrupt-gate entry clears IF. Retaining that mask prevents ordinary
    /// IRQs from recursively reusing the handler's stack before it finishes.
    pub(super) fn require_masked(
        &mut self,
        closure: &terminal_installation::SynchronousCallClosure<'_>,
    ) -> Result<(), String> {
        for body in &closure.machines {
            if !self.masked_bodies.insert(body.id) {
                continue;
            }
            for operation in body.blocks.iter().flat_map(|block| &block.operations) {
                if matches!(&operation.kind,
                    OperationKind::RegisterAccess { event, .. }
                    if event.operation() == RegisterOperation::InterruptControl(
                        language_core::inline_assembly::AsmInterruptControlKind::Enable))
                {
                    return Err(format!(
                        "masked installed handler machine {:?} operation {:?} enables nested maskable arrivals",
                        body.id, operation.id,
                    ));
                }
            }
        }
        Ok(())
    }
}

fn changes_configuration(operation: RegisterOperation) -> bool {
    matches!(
        operation,
        RegisterOperation::GlobalDescriptorTable(AsmGlobalDescriptorTableOperation::Load)
            | RegisterOperation::TaskRegisterActivate
            | RegisterOperation::SegmentReload(_)
            | RegisterOperation::SystemWrite(_)
            | RegisterOperation::ModelSpecificWrite
    )
}
