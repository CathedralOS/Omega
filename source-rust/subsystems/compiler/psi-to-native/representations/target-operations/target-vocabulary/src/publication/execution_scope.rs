use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};

/// Exact native reach used to check an emitted object's boundary calls and
/// callback registrations after the checked target selection has been released.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedExecutionScope {
    machines: Vec<MachineId>,
    boundaries: Vec<BoundaryMachineId>,
    callbacks: Vec<EmittedCallbackOccurrence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmittedCallbackOccurrence {
    pub application_index: u32,
    pub terminal_operation: OperationId,
}

impl EmittedExecutionScope {
    pub fn new(
        mut machines: Vec<MachineId>,
        mut boundaries: Vec<BoundaryMachineId>,
        callbacks: Vec<EmittedCallbackOccurrence>,
    ) -> Self {
        machines.sort_unstable();
        boundaries.sort_unstable();
        Self {
            machines,
            boundaries,
            callbacks,
        }
    }

    pub fn machines(&self) -> &[MachineId] {
        &self.machines
    }

    pub fn contains_machine(&self, machine: MachineId) -> bool {
        self.machines.binary_search(&machine).is_ok()
    }

    pub fn contains_boundary(&self, boundary: BoundaryMachineId) -> bool {
        self.boundaries.binary_search(&boundary).is_ok()
    }

    pub fn callbacks(&self) -> &[EmittedCallbackOccurrence] {
        &self.callbacks
    }
}
