//! Checked hosted-entry storage geometry and the receiver custody read by physical planning.
use diagnostics::Diagnostic;
use machine_code::physical_stack::StackDemand;
use terminal_installation::CheckedPrivateEntryRamRecipe;

/// The closed source receipt, physical contract and private-RAM recipes are
/// retained together through encoding. A receipt issuer owns their association.
pub trait HostedReceiverCustody {
    fn source(&self) -> &crate::SelectedProgramEntrySignature;
    fn checked_entry(&self) -> &terminal_psi::CheckedProgramEntryTerminalReceipt;
    fn physical_contract(&self) -> &crate::ProgramEntryPhysicalContractPlan;
    fn private_entry_ram(&self) -> &[CheckedPrivateEntryRamRecipe];
}

pub fn invalid(target: target::NativeTarget) -> Diagnostic {
    Diagnostic::error(format!(
        "hosted entry lost exact stack or auxiliary backing for {target:?}"
    ))
}

pub fn align(
    value: u64,
    alignment: u64,
    invalid: &dyn Fn() -> Diagnostic,
) -> Result<u64, Diagnostic> {
    if !alignment.is_power_of_two() {
        return Err(invalid());
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(invalid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedEntryPartitions {
    pub saved_continuation_offset: u64,
    pub stack_offset: u64,
    pub stack_byte_count: u64,
    pub allocation_offset: u64,
    pub end_offset: u64,
}

pub fn partitions(
    target: target::NativeTarget,
    finite_stack_bytes: Option<u64>,
    recipes: &[CheckedPrivateEntryRamRecipe],
    call_overhead: u64,
    existing_bss_bytes: u64,
) -> Result<HostedEntryPartitions, Diagnostic> {
    let invalid = || invalid(target);
    let saved = align(existing_bss_bytes, 16, &invalid)?;
    let stack = saved.checked_add(16).ok_or_else(invalid)?;
    let stack_bytes = align(
        finite_stack_bytes
            .ok_or_else(|| {
                Diagnostic::error("fixed hosted stack needs an admitted finite foreign-stack bound")
            })?
            .max(16)
            .checked_add(call_overhead)
            .ok_or_else(invalid)?,
        16,
        &invalid,
    )?;
    let stack_end = stack.checked_add(stack_bytes).ok_or_else(invalid)?;
    let allocation_offset = stack_end;
    let end = recipes.iter().try_fold(allocation_offset, |end, recipe| {
        let bytes = (recipe.contents().len() as u64)
            .checked_add(recipe.observation_header().len() as u64)
            .ok_or_else(invalid)?;
        align(end, recipe.alignment(), &invalid)?
            .checked_add(bytes)
            .ok_or_else(invalid)
    })?;
    Ok(HostedEntryPartitions {
        saved_continuation_offset: saved,
        stack_offset: stack,
        stack_byte_count: stack_bytes,
        allocation_offset,
        end_offset: end,
    })
}

/// A receiver stays coupled to the stack measured for the same physical root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedReceiverBinding<Receiver> {
    receipt: Receiver,
    demand: StackDemand,
    call_overhead: u64,
}

impl<R: HostedReceiverCustody> HostedReceiverBinding<R> {
    pub fn source(&self) -> &crate::SelectedProgramEntrySignature {
        self.receipt.source()
    }
    pub fn checked_entry(&self) -> &terminal_psi::CheckedProgramEntryTerminalReceipt {
        self.receipt.checked_entry()
    }
    pub fn physical_contract(&self) -> &crate::ProgramEntryPhysicalContractPlan {
        self.receipt.physical_contract()
    }
    pub fn private_entry_ram(&self) -> &[terminal_installation::CheckedPrivateEntryRamRecipe] {
        self.receipt.private_entry_ram()
    }
    pub const fn call_overhead(&self) -> u64 {
        self.call_overhead
    }
    pub const fn stack_demand(&self) -> &StackDemand {
        &self.demand
    }
    pub fn partitions(
        &self,
        existing_bss_bytes: u64,
    ) -> Result<crate::hosted_receiver::HostedEntryPartitions, Diagnostic> {
        crate::hosted_receiver::partitions(
            self.demand.target(),
            self.demand.ceiling_bytes(),
            self.private_entry_ram(),
            self.call_overhead,
            existing_bss_bytes,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedProgramContext<Receiver> {
    profile: Option<target::TargetProfile>,
    demand: StackDemand,
    receiver: Option<HostedReceiverBinding<Receiver>>,
}

impl<R> HostedProgramContext<R> {
    /// Bind the receiver's stack measurement and the physical entry's call overhead.
    pub fn from_demand(
        profile: Option<target::TargetProfile>,
        demand: StackDemand,
        receipt: Option<R>,
        call_overhead: u64,
    ) -> Self {
        let receiver = receipt.map(|receipt| HostedReceiverBinding {
            receipt,
            demand: demand.clone(),
            call_overhead,
        });
        Self {
            profile,
            demand,
            receiver,
        }
    }

    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.demand.psi()
    }
    pub const fn entry(&self) -> semantic_vocabulary::MachineId {
        self.demand.entry()
    }
    pub const fn profile(&self) -> Option<target::TargetProfile> {
        self.profile
    }
    pub const fn demand(&self) -> &StackDemand {
        &self.demand
    }
    pub const fn receiver(&self) -> Option<&HostedReceiverBinding<R>> {
        self.receiver.as_ref()
    }
}
