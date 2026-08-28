use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet};
use omega_target::{Architecture, NativeTarget, TargetProfile};

use super::fingerprint::fingerprint_transfer_plan;
use super::{NativeFuelContextLayout, NativeFuelTargetPlanProjection, SponsorContextTransport};

/// One exact machine value retained in the opaque activation save area.
/// Stack-pointer state is distinct from the AArch64 X31/ZR encoding and from
/// an ordinary general-purpose register slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeFuelSavedValue {
    Register(MachineRegister),
    Flags,
    StackPointer,
}

/// Exact naturally aligned context slot for one saved activation value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFuelActivationStateSlot {
    pub value: NativeFuelSavedValue,
    pub context_offset: u32,
    pub byte_count: u32,
}

/// Independently provisioned stack available after the transfer stub leaves
/// the suspended activation's stack. `byte_ceiling` bounds the complete
/// compiler/runtime transfer and resume path, excluding authored sponsor work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFuelSponsorStackPlan {
    pub alignment: u32,
    pub byte_ceiling: u64,
}

/// Stable object-independent identity for one compiler-owned runtime entry.
/// The final object/image layer rejoins these identities to concrete handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFuelRuntimeEntryIdentity {
    pub section_identity: u64,
    pub symbol_identity: u64,
}

/// Structural transfer/runtime plan consumed by target encoding and replay.
/// Its normalized identity is derived from every field and is never supplied
/// independently by a caller. Constructing this projection validates shape
/// only; it grants no execution or installation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFuelTransferRuntimePlanProjection {
    pub(super) profile: TargetProfile,
    pub(super) target: NativeTarget,
    pub(super) transport: SponsorContextTransport,
    pub(super) context: NativeFuelContextLayout,
    pub(super) activation_state_slots: Vec<NativeFuelActivationStateSlot>,
    pub(super) sponsor_stack: NativeFuelSponsorStackPlan,
    pub(super) interrupted_state: MachineStateSet,
    pub(super) saved_state: MachineStateSet,
    pub(super) restored_state: MachineStateSet,
    pub(super) transfer_entry: NativeFuelRuntimeEntryIdentity,
    pub(super) resume_entry: NativeFuelRuntimeEntryIdentity,
    pub(super) normalized_identity: u64,
}

impl NativeFuelTransferRuntimePlanProjection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        profile: TargetProfile,
        target: NativeTarget,
        transport: SponsorContextTransport,
        context: NativeFuelContextLayout,
        activation_state_slots: Vec<NativeFuelActivationStateSlot>,
        sponsor_stack: NativeFuelSponsorStackPlan,
        interrupted_state: MachineStateSet,
        saved_state: MachineStateSet,
        restored_state: MachineStateSet,
        transfer_entry: NativeFuelRuntimeEntryIdentity,
        resume_entry: NativeFuelRuntimeEntryIdentity,
    ) -> Result<Self, NativeFuelTransferPlanError> {
        validate_target_recipe(profile, target, transport, context)?;
        validate_activation_slots(target, context, &activation_state_slots)?;
        validate_state_sets(
            &activation_state_slots,
            interrupted_state,
            saved_state,
            restored_state,
        )?;
        validate_sponsor_stack(target, sponsor_stack)?;
        validate_entry_identities(transfer_entry, resume_entry)?;

        let mut plan = Self {
            profile,
            target,
            transport,
            context,
            activation_state_slots,
            sponsor_stack,
            interrupted_state,
            saved_state,
            restored_state,
            transfer_entry,
            resume_entry,
            normalized_identity: 0,
        };
        plan.normalized_identity = fingerprint_transfer_plan(&plan);
        Ok(plan)
    }

    /// Construct a structural projection and require its derived canonical
    /// identity to be the exact transfer identity named by target policy.
    #[allow(clippy::too_many_arguments)]
    pub fn from_target_policy(
        policy: NativeFuelTargetPlanProjection,
        activation_state_slots: Vec<NativeFuelActivationStateSlot>,
        sponsor_stack: NativeFuelSponsorStackPlan,
        interrupted_state: MachineStateSet,
        saved_state: MachineStateSet,
        restored_state: MachineStateSet,
        transfer_entry: NativeFuelRuntimeEntryIdentity,
        resume_entry: NativeFuelRuntimeEntryIdentity,
    ) -> Result<Self, NativeFuelTransferPlanError> {
        let plan = Self::new(
            policy.profile,
            policy.target,
            policy.transport,
            policy.context,
            activation_state_slots,
            sponsor_stack,
            interrupted_state,
            saved_state,
            restored_state,
            transfer_entry,
            resume_entry,
        )?;
        plan.validate_target_policy(policy)?;
        Ok(plan)
    }

    pub fn validate_target_policy(
        &self,
        policy: NativeFuelTargetPlanProjection,
    ) -> Result<(), NativeFuelTransferPlanError> {
        if self.profile != policy.profile
            || self.target != policy.target
            || self.transport != policy.transport
            || self.context != policy.context
        {
            return Err(NativeFuelTransferPlanError::TargetPolicyMismatch);
        }
        if self.normalized_identity != policy.transfer_plan_identity {
            return Err(NativeFuelTransferPlanError::TransferPlanIdentityMismatch {
                expected: self.normalized_identity,
                supplied: policy.transfer_plan_identity,
            });
        }
        Ok(())
    }

    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn transport(&self) -> SponsorContextTransport {
        self.transport
    }

    pub const fn context(&self) -> NativeFuelContextLayout {
        self.context
    }

    pub fn activation_state_slots(&self) -> &[NativeFuelActivationStateSlot] {
        &self.activation_state_slots
    }

    pub const fn sponsor_stack(&self) -> NativeFuelSponsorStackPlan {
        self.sponsor_stack
    }

    pub const fn interrupted_state(&self) -> MachineStateSet {
        self.interrupted_state
    }

    pub const fn saved_state(&self) -> MachineStateSet {
        self.saved_state
    }

    pub const fn restored_state(&self) -> MachineStateSet {
        self.restored_state
    }

    pub const fn transfer_entry(&self) -> NativeFuelRuntimeEntryIdentity {
        self.transfer_entry
    }

    pub const fn resume_entry(&self) -> NativeFuelRuntimeEntryIdentity {
        self.resume_entry
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFuelTransferPlanError {
    InvalidTargetRecipe,
    EmptyActivationState,
    InvalidActivationStateSlot,
    NonCanonicalActivationStateSlots,
    DuplicateSavedValue,
    IncompleteActivationStateCoverage,
    StateSetMismatch,
    InvalidSponsorStack,
    InvalidEntryIdentity,
    DuplicateEntryIdentity,
    TargetPolicyMismatch,
    TransferPlanIdentityMismatch { expected: u64, supplied: u64 },
}

impl std::fmt::Display for NativeFuelTransferPlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeFuelTransferPlanError {}

fn validate_target_recipe(
    profile: TargetProfile,
    target: NativeTarget,
    transport: SponsorContextTransport,
    context: NativeFuelContextLayout,
) -> Result<(), NativeFuelTransferPlanError> {
    let transport_matches = matches!(
        (target.architecture, transport),
        (
            Architecture::X86_64,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            },
        ) | (
            Architecture::Aarch64,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28),
            },
        )
    );
    if profile.native_target() != target
        || target.pointer_size != 8
        || target.pointer_alignment != 8
        || !transport_matches
        || context.byte_size == 0
        || context.alignment < 8
        || !context.alignment.is_power_of_two()
        || !context.byte_size.is_multiple_of(context.alignment)
    {
        return Err(NativeFuelTransferPlanError::InvalidTargetRecipe);
    }

    let scalar_offsets = [
        context.remaining_units_offset,
        context.unpaid_site_kind_offset,
        context.unpaid_site_identity_offset,
        context.required_units_offset,
        context.transfer_entry_offset,
        context.retry_code_offset_offset,
        context.sponsor_stack_top_offset,
    ];
    let mut ranges = Vec::with_capacity(scalar_offsets.len() + 1);
    for offset in scalar_offsets {
        let Some(end) = offset.checked_add(8) else {
            return Err(NativeFuelTransferPlanError::InvalidTargetRecipe);
        };
        if !offset.is_multiple_of(8) || end > context.byte_size {
            return Err(NativeFuelTransferPlanError::InvalidTargetRecipe);
        }
        ranges.push((offset, end));
    }
    let Some(activation_end) = context
        .activation_state_offset
        .checked_add(context.activation_state_byte_count)
    else {
        return Err(NativeFuelTransferPlanError::InvalidTargetRecipe);
    };
    if context.activation_state_byte_count == 0
        || !context.activation_state_offset.is_multiple_of(8)
        || activation_end > context.byte_size
    {
        return Err(NativeFuelTransferPlanError::InvalidTargetRecipe);
    }
    ranges.push((context.activation_state_offset, activation_end));
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(NativeFuelTransferPlanError::InvalidTargetRecipe);
    }
    Ok(())
}

fn validate_activation_slots(
    target: NativeTarget,
    context: NativeFuelContextLayout,
    slots: &[NativeFuelActivationStateSlot],
) -> Result<(), NativeFuelTransferPlanError> {
    if slots.is_empty() {
        return Err(NativeFuelTransferPlanError::EmptyActivationState);
    }
    let activation_end = context.activation_state_offset + context.activation_state_byte_count;
    let mut expected_offset = context.activation_state_offset;
    let mut values = Vec::with_capacity(slots.len());
    for slot in slots {
        let expected_bytes = saved_value_byte_count(slot.value, target.architecture)?;
        let Some(end) = slot.context_offset.checked_add(slot.byte_count) else {
            return Err(NativeFuelTransferPlanError::InvalidActivationStateSlot);
        };
        if slot.byte_count != expected_bytes
            || !slot.context_offset.is_multiple_of(slot.byte_count)
            || slot.context_offset < context.activation_state_offset
            || end > activation_end
        {
            return Err(NativeFuelTransferPlanError::InvalidActivationStateSlot);
        }
        if slot.context_offset != expected_offset {
            return Err(if slot.context_offset < expected_offset {
                NativeFuelTransferPlanError::NonCanonicalActivationStateSlots
            } else {
                NativeFuelTransferPlanError::IncompleteActivationStateCoverage
            });
        }
        expected_offset = end;
        if values.contains(&slot.value) {
            return Err(NativeFuelTransferPlanError::DuplicateSavedValue);
        }
        values.push(slot.value);
    }
    if expected_offset != activation_end {
        return Err(NativeFuelTransferPlanError::IncompleteActivationStateCoverage);
    }
    if !values.contains(&NativeFuelSavedValue::StackPointer) {
        return Err(NativeFuelTransferPlanError::StateSetMismatch);
    }
    Ok(())
}

fn saved_value_byte_count(
    value: NativeFuelSavedValue,
    architecture: Architecture,
) -> Result<u32, NativeFuelTransferPlanError> {
    match value {
        NativeFuelSavedValue::Register(register) => {
            let invalid_index = match register {
                MachineRegister::X86Xmm(index) | MachineRegister::Aarch64V(index) => index > 31,
                MachineRegister::Aarch64X(index) => index > 30,
                _ => false,
            };
            if register.architecture() != architecture
                || register == MachineRegister::X86Rsp
                || invalid_index
            {
                return Err(NativeFuelTransferPlanError::InvalidActivationStateSlot);
            }
            Ok(match register {
                MachineRegister::X86Xmm(_) | MachineRegister::Aarch64V(_) => 16,
                _ => 8,
            })
        }
        NativeFuelSavedValue::Flags | NativeFuelSavedValue::StackPointer => Ok(8),
    }
}

fn validate_state_sets(
    slots: &[NativeFuelActivationStateSlot],
    interrupted: MachineStateSet,
    saved: MachineStateSet,
    restored: MachineStateSet,
) -> Result<(), NativeFuelTransferPlanError> {
    let mut states = vec![MachineState::InstructionPointer, MachineState::StackPointer];
    for slot in slots {
        states.push(match slot.value {
            NativeFuelSavedValue::Register(
                MachineRegister::X86Xmm(_) | MachineRegister::Aarch64V(_),
            ) => MachineState::VectorRegisters,
            NativeFuelSavedValue::Register(_) => MachineState::GeneralRegisters,
            NativeFuelSavedValue::Flags => MachineState::Flags,
            NativeFuelSavedValue::StackPointer => MachineState::StackPointer,
        });
    }
    let exact = MachineStateSet::new(states);
    if interrupted != exact || saved != exact || restored != exact {
        return Err(NativeFuelTransferPlanError::StateSetMismatch);
    }
    Ok(())
}

fn validate_sponsor_stack(
    target: NativeTarget,
    stack: NativeFuelSponsorStackPlan,
) -> Result<(), NativeFuelTransferPlanError> {
    if stack.alignment == 0
        || !stack.alignment.is_power_of_two()
        || stack.alignment < target.pointer_alignment as u32
        || stack.byte_ceiling == 0
        || !stack
            .byte_ceiling
            .is_multiple_of(u64::from(stack.alignment))
    {
        return Err(NativeFuelTransferPlanError::InvalidSponsorStack);
    }
    Ok(())
}

fn validate_entry_identities(
    transfer: NativeFuelRuntimeEntryIdentity,
    resume: NativeFuelRuntimeEntryIdentity,
) -> Result<(), NativeFuelTransferPlanError> {
    if transfer.section_identity == 0
        || transfer.symbol_identity == 0
        || resume.section_identity == 0
        || resume.symbol_identity == 0
    {
        return Err(NativeFuelTransferPlanError::InvalidEntryIdentity);
    }
    if transfer == resume {
        return Err(NativeFuelTransferPlanError::DuplicateEntryIdentity);
    }
    Ok(())
}
