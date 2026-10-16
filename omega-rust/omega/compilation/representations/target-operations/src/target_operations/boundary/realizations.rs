//! Closed target mechanisms and normalized foreign-call realizations.

use crate::{TargetStructuralArgument, TargetUnitScalarCallArgument};
use calling_conventions::BoundaryEntryPlan;
use semantic_vocabulary::{OperationId, ServiceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataOnlyPortRealization {
    pub effect_operation: OperationId,
    pub service: ServiceId,
    pub port: u16,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectPortReadU8Realization {
    pub service: ServiceId,
    pub port: u16,
}

/// Import-free hosted process termination through the selected kernel's exit
/// syscall. The syscall number and register assignment are target facts, not
/// producer-selected metadata, so this realization carries no configurable
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostedExitProcessI32Realization;

impl HostedExitProcessI32Realization {
    /// Only complete canonical target profiles select this closed realization.
    pub fn supports_target(target: target::NativeTarget) -> bool {
        target::HostedIntrinsicBundle::from_native_target(target).is_some()
    }
}

/// Import-free hosted single-byte standard-input read through `read(2)`. The
/// realization writes one complete conventional `ByteRead` sum into its
/// assigned caller-frame home: zero remains `Eof`, success writes case tag 1
/// and the zero-extended byte payload, and every other syscall result traps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostedReadByteRealization;

impl HostedReadByteRealization {
    /// Only complete canonical target profiles select this closed realization.
    pub fn supports_target(target: target::NativeTarget) -> bool {
        target::HostedIntrinsicBundle::from_native_target(target).is_some()
    }
}

/// Import-free hosted single-byte standard-output write through the selected
/// kernel's `write(2)` ABI. Syscall coordinates remain exact target facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostedWriteByteI32Realization;

impl HostedWriteByteI32Realization {
    /// Only complete canonical target profiles select this closed realization.
    pub fn supports_target(target: target::NativeTarget) -> bool {
        target::HostedIntrinsicBundle::from_native_target(target).is_some()
    }
}

/// Inline IEEE operation on `format` for one selected float provider. The
/// call's scalar arguments become the operation's
/// operands in authored order and its scalar result the operation's result;
/// no call, frame, or import exists. Both supported architectures carry
/// scalar IEEE binary32/binary64 arithmetic, selection, square root and
/// conversion that honor the canonical floating controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NearestIeeeFloatArithmeticRealization {
    pub operation: crate::IeeeFloatArithmeticOperation,
    pub format: semantic_vocabulary::IeeeFloatFormat,
    /// Build-admitted AVX+FMA3 deployment custody. Present exactly on an x86
    /// fused multiply-add; every later stage reads it from this record.
    pub x86_scalar_fma: Option<target::AdmittedX86ScalarFmaProvider>,
}

impl NearestIeeeFloatArithmeticRealization {
    /// AArch64 provides scalar FMA in its baseline FP instruction set. x86's
    /// SSE2 baseline does not: an x86 FMA is supported only with the admitted
    /// AVX+FMA3 provider for this exact target. Equal signatures cannot
    /// donate that authority or select two roundings instead, and the
    /// provider cannot ride on any other operation or architecture.
    pub fn supports_target(&self, target: target::NativeTarget) -> bool {
        let fused = self.operation == crate::IeeeFloatArithmeticOperation::FusedMultiplyAdd;
        match (target.architecture, self.x86_scalar_fma) {
            (target::Architecture::Aarch64, None) => true,
            (target::Architecture::X86_64, None) => !fused,
            // Provider planning verified the admission against the selected
            // profile; this record only has to stay on its own target.
            (target::Architecture::X86_64, Some(provider)) => {
                fused && provider.profile().native_target() == target
            }
            (target::Architecture::Aarch64, Some(_)) => false,
        }
    }

    /// Whether a boundary with these scalar parameters and this scalar
    /// result has exactly this realization's signature.
    pub fn signature_matches(
        &self,
        parameters: &[semantic_vocabulary::ScalarType],
        result: semantic_vocabulary::ScalarType,
    ) -> bool {
        self.operation
            .signature_matches(self.format, parameters, result)
    }
}

/// Import-free Linux standard-output realization through the kernel's
/// `write(2)` ABI. The emitted loop consumes the complete immutable payload
/// and one trailing newline or traps; no hosted import is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxWriteLineRealization;

/// A provider execution whose complete native effect is the successful
/// completion of the boundary call's retained ownership claims.
///
/// This realization has no scalar input, result, byte-sequence payload, or
/// target instruction. The boundary occurrence, admitted provider execution,
/// structural arguments, and completion receipts remain explicit in the
/// surrounding [`crate::TargetUnitOperation::BoundarySettlement`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClaimCompletionOnlyRealization;

/// An evaluated external entry and its mechanism-specific custody. Argument
/// and result transport belongs to the retained plan, not to an import-only
/// lowering route. A kernel entry has neither a linker locator nor a claim on
/// a user-space callee's stack; it must never manufacture either one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedForeignCallBinding {
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub mechanism: NormalizedForeignCallMechanism,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedForeignCallMechanism {
    Import {
        locator: target::NormalizedForeignLocator,
        same_stack_contribution: task_plans::AdmittedSameStackContribution,
    },
    SystemCall(NormalizedSystemCallBinding),
    Table {
        target: target::TargetProfile,
        byte_offset: u32,
        dispatch_only: bool,
        same_stack_contribution: task_plans::AdmittedSameStackContribution,
    },
}

/// Exact source-free custody for a direct kernel entry. Unlike an imported
/// user-space callee it consumes no foreign receiver-stack reserve. The
/// ordinary lowering stages still owe argument placement, result custody and
/// the exact supervisor-entry instruction and clobbers in this retained plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSystemCallBinding {
    pub target: target::TargetProfile,
    pub number: i64,
    pub requirement_identity: String,
    pub provider_plan_report_identity: u64,
    pub provider_plan_commitment: [u8; 32],
}

impl NormalizedForeignCallBinding {
    /// Source transport includes a table used only for dispatch; the wire ABI
    /// does not. Keep that operand out of the boundary plan, so it cannot shift
    /// argument registers, stack slots, callback ordinals or result placement.
    /// The table occupies the target's volatile indirect-call scratch and is
    /// consumed by the address load before entering the foreign callee.
    pub fn operand_placements(&self) -> ForeignCallOperands<'_> {
        use calling_conventions::{MachineRegister, ValueLocation, ValuePlacement, ValueShape};
        let dispatch = match &self.mechanism {
            NormalizedForeignCallMechanism::Table {
                target,
                dispatch_only: true,
                ..
            } => {
                let register = match target.native_target().architecture {
                    target::Architecture::X86_64 => MachineRegister::X86R11,
                    target::Architecture::Aarch64 => MachineRegister::Aarch64X(16),
                };
                Some(ValuePlacement {
                    shape: ValueShape::integer(8, 8),
                    locations: vec![ValueLocation::Register {
                        register,
                        byte_size: 8,
                        value_byte_offset: 0,
                    }],
                })
            }
            _ => None,
        };
        ForeignCallOperands {
            dispatch,
            parameters: &self.boundary_entry_plan.call.parameters,
        }
    }

    pub fn dispatch_operand_count(&self) -> usize {
        usize::from(matches!(self.table_dispatch(), Some((_, true))))
    }

    /// Resolve only a mechanism-supported native profile. In particular, an
    /// untrusted kernel row cannot ask the native catalog to realize Alpha or
    /// resolve a host-dependent alias while replaying portable custody.
    pub fn native_target(&self) -> Option<target::NativeTarget> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import { locator, .. } => {
                Some(locator.target().native_target())
            }
            NormalizedForeignCallMechanism::Table { target, .. } => Some(target.native_target()),
            NormalizedForeignCallMechanism::SystemCall(binding) => match binding.target {
                target::TargetProfile::LinuxX64 | target::TargetProfile::LinuxArm64 => {
                    Some(binding.target.native_target())
                }
                _ => None,
            },
        }
    }

    /// The admitted contribution stores an index into the module identity
    /// table; system-call bindings still carry text. `None` means the
    /// contribution's index is outside the table.
    pub fn requirement_identity<'identities>(
        &'identities self,
        identities: &'identities terminal_psi::TerminalIdentityTable,
    ) -> Option<&'identities str> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import {
                same_stack_contribution,
                ..
            }
            | NormalizedForeignCallMechanism::Table {
                same_stack_contribution,
                ..
            } => identities.text(same_stack_contribution.requirement_identity()),
            NormalizedForeignCallMechanism::SystemCall(binding) => {
                Some(binding.requirement_identity.as_str())
            }
        }
    }

    pub fn provider_plan_report_identity(&self) -> u64 {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import {
                same_stack_contribution,
                ..
            }
            | NormalizedForeignCallMechanism::Table {
                same_stack_contribution,
                ..
            } => same_stack_contribution.provider_plan_report_identity(),
            NormalizedForeignCallMechanism::SystemCall(binding) => {
                binding.provider_plan_report_identity
            }
        }
    }

    /// Imports retain their ordinary calling policy. Kernel entries instead
    /// require the exact target ABI, including clobbers and supervisor entry;
    /// generic authored-plan validation alone permits weaker promises.
    /// This physical check does not establish source argument/result custody.
    pub fn has_valid_entry_mechanism(&self) -> bool {
        use calling_conventions::{CallSignature, CallingPolicy, EntryControl};
        let Some(target) = self.native_target() else {
            return false;
        };
        let call = &self.boundary_entry_plan.call;
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import { .. }
            | NormalizedForeignCallMechanism::Table { .. } => {
                call.policy == CallingPolicy::native_for_target(target)
                    && call.entry_control == EntryControl::CallReturn
            }
            NormalizedForeignCallMechanism::SystemCall(binding) => {
                let Some(policy) = CallingPolicy::native_syscall_for_target(target) else {
                    return false;
                };
                let signature = CallSignature {
                    parameters: call
                        .parameters
                        .iter()
                        .map(|parameter| parameter.shape)
                        .collect(),
                    result: call.result.as_ref().map(|result| result.shape),
                };
                binding.number >= 0
                    && !binding.requirement_identity.is_empty()
                    && binding.provider_plan_report_identity != 0
                    && calling_conventions::evaluate_call_plan(policy, &signature).as_ref()
                        == Ok(call)
            }
        }
    }

    /// Kernel coordinates remain in the existing external-call custody record.
    pub fn system_call(&self) -> Option<&NormalizedSystemCallBinding> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::SystemCall(binding) => Some(binding),
            NormalizedForeignCallMechanism::Import { .. }
            | NormalizedForeignCallMechanism::Table { .. } => None,
        }
    }

    pub fn import(
        &self,
    ) -> Option<(
        &target::NormalizedForeignLocator,
        &task_plans::AdmittedSameStackContribution,
    )> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution,
            } => Some((locator, same_stack_contribution)),
            NormalizedForeignCallMechanism::SystemCall(_)
            | NormalizedForeignCallMechanism::Table { .. } => None,
        }
    }

    /// User-space counterparties share stack and floating-control obligations,
    /// whether their entry address comes from a linker or a named table field.
    pub fn same_stack_contribution(&self) -> Option<&task_plans::AdmittedSameStackContribution> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import {
                same_stack_contribution,
                ..
            }
            | NormalizedForeignCallMechanism::Table {
                same_stack_contribution,
                ..
            } => Some(same_stack_contribution),
            NormalizedForeignCallMechanism::SystemCall(_) => None,
        }
    }

    pub fn table_dispatch(&self) -> Option<(u32, bool)> {
        match self.mechanism {
            NormalizedForeignCallMechanism::Table {
                byte_offset,
                dispatch_only,
                ..
            } => Some((byte_offset, dispatch_only)),
            _ => None,
        }
    }
}

/// Borrow the wire placements; only the dispatch operand is synthesized.
/// Indices in scalar/structural argument records address this source transport
/// order. Native callback ordinals continue to address the wire plan itself.
pub struct ForeignCallOperands<'plan> {
    dispatch: Option<calling_conventions::ValuePlacement>,
    parameters: &'plan [calling_conventions::ValuePlacement],
}

impl ForeignCallOperands<'_> {
    pub fn len(&self) -> usize {
        self.parameters.len() + usize::from(self.dispatch.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.dispatch.is_none() && self.parameters.is_empty()
    }

    pub fn get(&self, operand_index: usize) -> Option<&calling_conventions::ValuePlacement> {
        match (&self.dispatch, operand_index) {
            (Some(dispatch), 0) => Some(dispatch),
            (Some(_), operand_index) => self.parameters.get(operand_index - 1),
            (None, operand_index) => self.parameters.get(operand_index),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &calling_conventions::ValuePlacement> {
        self.dispatch.iter().chain(self.parameters.iter())
    }
}

/// One occurrence-specific fixed-width integer value materialized for an
/// evaluated normalized foreign call. The exact authored constant or durable
/// scalar-result home remains bound to the ordered placement selected by the
/// evaluated boundary call plan. The bounded native carrier admits the
/// target's complete register-resident fixed-integer argument bank.
pub type NormalizedForeignScalarArgument = TargetUnitScalarCallArgument;

/// One occurrence-specific source-rooted structural value materialized for an
/// evaluated normalized foreign call. The exact caller place, semantic field
/// path, access mode, projected structural type, source byte offset, and
/// source placement remain bound to the ordered placement selected by the
/// evaluated boundary call plan. The bounded native carrier admits borrowed
/// flat-record arguments: the evaluated plan places the projected referent's
/// pointer in the target's argument bank without copying its storage.
pub type NormalizedForeignStructuralArgument = TargetStructuralArgument;

/// Closed native settlement choice. Keeping evaluated imports disjoint from
/// built-in realizations prevents locator custody from being stripped into a
/// no-code boundary settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundarySettlementRealization {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(NormalizedForeignCallBinding),
}

impl From<BoundaryRealization> for BoundarySettlementRealization {
    fn from(realization: BoundaryRealization) -> Self {
        Self::Builtin(realization)
    }
}

macro_rules! builtin_settlement_conversion {
    ($realization:ty) => {
        impl From<$realization> for BoundarySettlementRealization {
            fn from(realization: $realization) -> Self {
                Self::Builtin(realization.into())
            }
        }
    };
}

builtin_settlement_conversion!(MetadataOnlyPortRealization);
builtin_settlement_conversion!(DirectPortReadU8Realization);
builtin_settlement_conversion!(LinuxWriteLineRealization);
builtin_settlement_conversion!(HostedExitProcessI32Realization);
builtin_settlement_conversion!(HostedReadByteRealization);
builtin_settlement_conversion!(HostedWriteByteI32Realization);
builtin_settlement_conversion!(ClaimCompletionOnlyRealization);
builtin_settlement_conversion!(NearestIeeeFloatArithmeticRealization);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryRealization {
    PlacedScalarAccess(PlacedScalarAccessRealization),
    MetadataOnlyPort(MetadataOnlyPortRealization),
    DirectPortReadU8(DirectPortReadU8Realization),
    LinuxWriteLine(LinuxWriteLineRealization),
    HostedExitProcessI32(HostedExitProcessI32Realization),
    HostedReadByte(HostedReadByteRealization),
    HostedWriteByteI32(HostedWriteByteI32Realization),
    ClaimCompletionOnly(ClaimCompletionOnlyRealization),
    NearestIeeeFloatArithmetic(NearestIeeeFloatArithmeticRealization),
}

impl From<NearestIeeeFloatArithmeticRealization> for BoundaryRealization {
    fn from(realization: NearestIeeeFloatArithmeticRealization) -> Self {
        Self::NearestIeeeFloatArithmetic(realization)
    }
}

impl From<MetadataOnlyPortRealization> for BoundaryRealization {
    fn from(realization: MetadataOnlyPortRealization) -> Self {
        Self::MetadataOnlyPort(realization)
    }
}

impl From<DirectPortReadU8Realization> for BoundaryRealization {
    fn from(realization: DirectPortReadU8Realization) -> Self {
        Self::DirectPortReadU8(realization)
    }
}

impl From<HostedExitProcessI32Realization> for BoundaryRealization {
    fn from(realization: HostedExitProcessI32Realization) -> Self {
        Self::HostedExitProcessI32(realization)
    }
}

impl From<HostedReadByteRealization> for BoundaryRealization {
    fn from(realization: HostedReadByteRealization) -> Self {
        Self::HostedReadByte(realization)
    }
}

impl From<HostedWriteByteI32Realization> for BoundaryRealization {
    fn from(realization: HostedWriteByteI32Realization) -> Self {
        Self::HostedWriteByteI32(realization)
    }
}

impl From<LinuxWriteLineRealization> for BoundaryRealization {
    fn from(realization: LinuxWriteLineRealization) -> Self {
        Self::LinuxWriteLine(realization)
    }
}

impl From<ClaimCompletionOnlyRealization> for BoundaryRealization {
    fn from(realization: ClaimCompletionOnlyRealization) -> Self {
        Self::ClaimCompletionOnly(realization)
    }
}

/// A selected scalar operation on a sealed placed declaration, with no backing grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedScalarAccessRealization {
    pub contract: [u8; 32],
    pub demand: terminal_psi::PlacedScalarBoundaryDemand,
}
impl From<PlacedScalarAccessRealization> for BoundaryRealization {
    fn from(realization: PlacedScalarAccessRealization) -> Self {
        Self::PlacedScalarAccess(realization)
    }
}
builtin_settlement_conversion!(PlacedScalarAccessRealization);
