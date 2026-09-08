//! Closed target mechanisms and normalized foreign-call realizations.

use crate::TargetUnitScalarCallArgument;
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
        [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
        ]
        .contains(&target)
    }
}

/// Import-free Linux single-byte standard-input read through `read(2)`. The
/// realization writes one complete conventional `ByteRead` sum into its
/// assigned caller-frame home: zero remains `Eof`, success writes case tag 1
/// and the zero-extended byte payload, and every other syscall result traps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxReadByteRealization;

/// Import-free hosted single-byte standard-output write through the selected
/// kernel's `write(2)` ABI. Syscall coordinates remain exact target facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostedWriteByteI32Realization;

impl HostedWriteByteI32Realization {
    /// Only complete canonical target profiles select this closed realization.
    pub fn supports_target(target: target::NativeTarget) -> bool {
        [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
        ]
        .contains(&target)
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

/// Exact source-free custody for one evaluated normalized import leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedForeignCallBinding {
    pub locator: target::NormalizedForeignLocator,
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub same_stack_contribution: task_plans::AdmittedSameStackContribution,
}

/// One occurrence-specific fixed-width integer value materialized for an
/// evaluated normalized foreign call. The exact authored constant or durable
/// scalar-result home remains bound to the ordered placement selected by the
/// evaluated boundary call plan. The bounded native carrier admits the
/// target's complete register-resident fixed-integer argument bank.
pub type NormalizedForeignScalarArgument = TargetUnitScalarCallArgument;

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
builtin_settlement_conversion!(LinuxReadByteRealization);
builtin_settlement_conversion!(HostedWriteByteI32Realization);
builtin_settlement_conversion!(ClaimCompletionOnlyRealization);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryRealization {
    MetadataOnlyPort(MetadataOnlyPortRealization),
    DirectPortReadU8(DirectPortReadU8Realization),
    LinuxWriteLine(LinuxWriteLineRealization),
    HostedExitProcessI32(HostedExitProcessI32Realization),
    LinuxReadByte(LinuxReadByteRealization),
    HostedWriteByteI32(HostedWriteByteI32Realization),
    ClaimCompletionOnly(ClaimCompletionOnlyRealization),
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

impl From<LinuxReadByteRealization> for BoundaryRealization {
    fn from(realization: LinuxReadByteRealization) -> Self {
        Self::LinuxReadByte(realization)
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
