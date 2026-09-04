//! Closed target mechanisms and normalized foreign-call realizations.

use crate::TargetUnitScalarCallArgument;
use omega_calling_conventions::BoundaryEntryPlan;
use psi_core::{OperationId, ServiceId};

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

/// Import-free Linux process termination through the kernel's `exit_group`
/// syscall. The syscall number and register assignment are target facts, not
/// producer-selected metadata, so this realization carries no configurable
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxExitGroupI32Realization;

/// Import-free Linux single-byte standard-input read through `read(2)`. The
/// realization writes one complete conventional `ByteRead` sum into its
/// assigned caller-frame home: zero remains `Eof`, success writes case tag 1
/// and the zero-extended byte payload, and every other syscall result traps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxReadByteRealization;

/// Import-free Linux single-byte standard-output write through the kernel's
/// `write(2)` ABI. Syscall coordinates are target facts and remain closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxWriteByteI32Realization;

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
    pub locator: omega_target::NormalizedForeignLocator,
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub same_stack_contribution: omega_task_plans::AdmittedSameStackContribution,
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
builtin_settlement_conversion!(LinuxExitGroupI32Realization);
builtin_settlement_conversion!(LinuxReadByteRealization);
builtin_settlement_conversion!(LinuxWriteByteI32Realization);
builtin_settlement_conversion!(ClaimCompletionOnlyRealization);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryRealization {
    MetadataOnlyPort(MetadataOnlyPortRealization),
    DirectPortReadU8(DirectPortReadU8Realization),
    LinuxWriteLine(LinuxWriteLineRealization),
    LinuxExitGroupI32(LinuxExitGroupI32Realization),
    LinuxReadByte(LinuxReadByteRealization),
    LinuxWriteByteI32(LinuxWriteByteI32Realization),
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

impl From<LinuxExitGroupI32Realization> for BoundaryRealization {
    fn from(realization: LinuxExitGroupI32Realization) -> Self {
        Self::LinuxExitGroupI32(realization)
    }
}

impl From<LinuxReadByteRealization> for BoundaryRealization {
    fn from(realization: LinuxReadByteRealization) -> Self {
        Self::LinuxReadByte(realization)
    }
}

impl From<LinuxWriteByteI32Realization> for BoundaryRealization {
    fn from(realization: LinuxWriteByteI32Realization) -> Self {
        Self::LinuxWriteByteI32(realization)
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
