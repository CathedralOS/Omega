//! Selected-form row route descriptors consumed by the encoding producer.
//!
//! Each [`SelectedInstructionKind`] declares the route its encoding row takes:
//! a resolved-address memory or stack form binding one
//! `PhysicalAddressOperation` family, an internal machine call template
//! carrying an unresolved fixup, a deferred control-flow row whose bytes wait
//! for resolved branch layout, or an ordinary self-contained form. Only the
//! producer reads these descriptors; `validation::row` keeps its own
//! kind-keyed matching so a routing mistake cannot self-certify — the same
//! producer/validator split the selected-lowering pair-rule descriptors use
//! for their result channels.

use physical_instructions::PhysicalAddressOperation;
use selected_instructions::SelectedInstructionKind;

/// The `PhysicalAddressOperation` family an address-routed row binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AddressOperationFamily {
    Store,
    AddressOffset,
    Load64,
    LoadPacked,
    StorePacked,
    Load8,
    Load16,
    Load32,
    Load8Indexed,
    HostedReadByte,
    HostedWriteByteI32,
    Store64,
    FrameAddress,
}

impl AddressOperationFamily {
    /// Admit only the declared symbolic operation family. A resolved address
    /// naming a different family contradicts the selected kind's declared
    /// route however self-consistent its displacement is.
    pub(super) fn admits(self, operation: PhysicalAddressOperation) -> bool {
        use PhysicalAddressOperation as Operation;
        matches!(
            (self, operation),
            (Self::Store, Operation::Store { .. })
                | (Self::AddressOffset, Operation::AddressOffset { .. })
                | (Self::Load64, Operation::Load64 { .. })
                | (Self::LoadPacked, Operation::LoadPacked { .. })
                | (Self::StorePacked, Operation::StorePacked { .. })
                | (Self::Load8, Operation::Load8 { .. })
                | (Self::Load16, Operation::Load16 { .. })
                | (Self::Load32, Operation::Load32 { .. })
                | (Self::Load8Indexed, Operation::Load8Indexed { .. })
                | (Self::HostedReadByte, Operation::HostedReadByte { .. })
                | (
                    Self::HostedWriteByteI32,
                    Operation::HostedWriteByteI32 { .. }
                )
                | (Self::Store64, Operation::Store64 { .. })
                | (Self::FrameAddress, Operation::FrameAddress { .. })
        )
    }
}

/// The hosted runtime channel a route's ISA entry point serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HostedChannel {
    /// An ordinary ISA encoder; no hosted runtime channel.
    None,
    /// The hosted byte store through the runtime write-byte entry.
    WriteByteI32,
    /// The hosted byte load through the runtime read-byte entry.
    ReadByte,
    /// The hosted process exit through the runtime exit entry.
    ExitProcessI32,
}

/// The route one selected instruction's encoding row takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowRoute {
    /// The row encodes through a resolved physical address in the declared
    /// operation family: the memory and stack dimension of the row contract.
    ResolvedAddress {
        operation: AddressOperationFamily,
        channel: HostedChannel,
    },
    /// The row encodes an internal machine call template whose fixup waits
    /// for resolved layout.
    InternalCallTemplate,
    /// The row encodes a normalized foreign call template whose import field
    /// stays unresolved until object construction binds it to the declared
    /// import symbol.
    NormalizedForeignCallTemplate,
    /// The row owns no bytes; resolved branch layout supplies them: the
    /// control-flow dimension of the row contract.
    DeferredControlFlow,
    /// The row is a self-contained encoded form.
    Ordinary { channel: HostedChannel },
}

/// Declare the encoding route of one selected instruction kind.
pub(super) fn route_of(kind: SelectedInstructionKind) -> RowRoute {
    use SelectedInstructionKind as Kind;
    let addressed = |operation| RowRoute::ResolvedAddress {
        operation,
        channel: HostedChannel::None,
    };
    match kind {
        Kind::Store { .. } => addressed(AddressOperationFamily::Store),
        Kind::AddressOffset { .. } => addressed(AddressOperationFamily::AddressOffset),
        Kind::Load64 { .. } => addressed(AddressOperationFamily::Load64),
        Kind::LoadPacked { .. } => addressed(AddressOperationFamily::LoadPacked),
        Kind::StorePacked { .. } => addressed(AddressOperationFamily::StorePacked),
        Kind::Load8 { .. } => addressed(AddressOperationFamily::Load8),
        Kind::Load16 { .. } => addressed(AddressOperationFamily::Load16),
        Kind::Load32 { .. } => addressed(AddressOperationFamily::Load32),
        Kind::Load8Indexed => addressed(AddressOperationFamily::Load8Indexed),
        Kind::Store64 { .. } => addressed(AddressOperationFamily::Store64),
        Kind::FrameAddress { .. } => addressed(AddressOperationFamily::FrameAddress),
        Kind::HostedReadByte { .. } => RowRoute::ResolvedAddress {
            operation: AddressOperationFamily::HostedReadByte,
            channel: HostedChannel::ReadByte,
        },
        Kind::HostedWriteByteI32 { .. } => RowRoute::ResolvedAddress {
            operation: AddressOperationFamily::HostedWriteByteI32,
            channel: HostedChannel::WriteByteI32,
        },
        Kind::CallScalar { .. } | Kind::CallUnit { .. } | Kind::CallAggregate { .. } => {
            RowRoute::InternalCallTemplate
        }
        Kind::NormalizedForeignCall { .. } => RowRoute::NormalizedForeignCallTemplate,
        Kind::ConditionalBranchNonZero
        | Kind::ConditionalBranchU64LessThan
        | Kind::ConditionalBranchI64LessThan
        | Kind::Jump => RowRoute::DeferredControlFlow,
        Kind::HostedExitProcessI32 => RowRoute::Ordinary {
            channel: HostedChannel::ExitProcessI32,
        },
        _ => RowRoute::Ordinary {
            channel: HostedChannel::None,
        },
    }
}
