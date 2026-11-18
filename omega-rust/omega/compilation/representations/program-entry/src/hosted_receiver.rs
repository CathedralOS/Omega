//! Checked hosted-entry storage geometry and the receiver custody read by physical planning.
use diagnostics::Diagnostic;
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
