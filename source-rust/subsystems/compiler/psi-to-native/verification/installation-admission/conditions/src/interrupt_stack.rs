//! Physical stack requirements and conditional arrival demands for installed handlers.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInterruptStackRequirement {
    pub installation_machine: semantic_vocabulary::MachineId,
    pub arrival: Option<InstalledArrivalStackDemand>, // Conditional composition for this installation; runtime guards check dedicated backing.
    pub fault_exclusion: Option<InstalledBodyFaultPremises>, // Conditional body result re-derived at final composition from the private body.
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub operation: semantic_vocabulary::OperationId,
    pub placement_index: usize,
    pub entry: semantic_vocabulary::MachineId,
    pub entry_plan: runtime_abi::static_interrupts::InstalledInterruptEntryPlan,
    pub body_bytes: u64,
    pub bridge_bytes: u32,
    pub call_stack_bias: u32,
    pub alignment: u32,
}

impl InstalledInterruptStackRequirement {
    pub fn required_bytes(&self) -> Option<u64> {
        let own = self.own_bytes()?;
        Some(
            self.arrival
                .as_ref()
                .map_or(own, |arrival| own.max(arrival.domain_bytes)),
        )
    }

    pub fn own_bytes(&self) -> Option<u64> {
        self.body_bytes
            .checked_add(u64::from(self.bridge_bytes))?
            .checked_add(u64::from(self.call_stack_bias))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledBodyFaultPremises {
    pub physical_program_identity: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledArrivalStackDemand {
    pub physical_entry: InstalledPhysicalStackRoot,
    pub root_reserve_bytes: u64,
    pub domain_bytes: u64,
    pub simultaneous_placements: Vec<usize>, // Sorted, unique, reciprocal placement indices of simultaneously live dedicated entries.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledPhysicalStackRoot {
    pub semantic_entry: semantic_vocabulary::MachineId,
    pub adapter: semantic_vocabulary::MachineId,
}
