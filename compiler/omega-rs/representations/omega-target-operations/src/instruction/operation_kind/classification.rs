use super::TargetOperationKind;
use crate::RuntimeTextReadSource;
use omega_core::operations::{OperationDomain, OperationSemanticQuery};

pub type TargetOperationDomain = OperationDomain;

impl OperationSemanticQuery for TargetOperationKind {
    fn semantic_domain(&self) -> TargetOperationDomain {
        match self {
            Self::EnterFunction
            | Self::LeaveFunction
            | Self::CallInternalFunction { .. }
            | Self::LoadOutgoingStackAddress { .. }
            | Self::ReserveOutgoingStackFrame { .. }
            | Self::WriteOutgoingStackU64 { .. }
            | Self::CopyEntryIndirectU64ToOutgoingStack { .. }
            | Self::ReleaseOutgoingStackFrame { .. } => TargetOperationDomain::FunctionBoundary,

            Self::EnterDispatchLoop { .. }
            | Self::EnterDispatchCase { .. }
            | Self::SetDispatchState { .. }
            | Self::TerminateDispatch
            | Self::LeaveDispatchCase
            | Self::LeaveDispatchLoop => TargetOperationDomain::DispatchControl,

            Self::EvaluateDispatchGuard { .. }
            | Self::CompareRuntimeTextLiteral { .. }
            | Self::CompareRuntimeTextStorage { .. }
            | Self::CompareRuntimeValues { .. }
            | Self::ComparePlaces { .. }
            | Self::ComparePlaceValue { .. } => TargetOperationDomain::GuardEvaluation,

            Self::WriteRuntimeTextLiteral { .. }
            | Self::WriteRuntimeTextLiteralSegment { .. }
            | Self::AppendRuntimeTextStoredSuffix { .. }
            | Self::MaterializeTextBufferToPlace { .. }
            | Self::AppendTextStoredToPlace { .. }
            | Self::AppendTextLiteralToPlace { .. } => TargetOperationDomain::RuntimeTextAssembly,

            Self::AtomicLoad { .. }
            | Self::AtomicStore { .. }
            | Self::AtomicFetchAdd { .. }
            | Self::AtomicFetchSub { .. }
            | Self::AtomicFetchXor { .. }
            | Self::AtomicFetchOr { .. }
            | Self::AtomicFetchAnd { .. }
            | Self::AtomicSwap { .. }
            | Self::AtomicCompareExchange { .. }
            | Self::WriteEntryArgumentRegister { .. }
            | Self::WriteEntryStackArgument { .. }
            | Self::WriteEntryIndirectArgument { .. }
            | Self::WriteEntryArgumentsSliceDescriptor { .. }
            | Self::WriteRuntimeStorageConvert { .. }
            | Self::WritePlaceConvert { .. }
            | Self::AppendPlaceBoundedBufferSource { .. }
            | Self::AppendPlaceBoundedBufferLiteral { .. }
            | Self::AppendWireLiteralByte { .. }
            | Self::AppendWireScalarVarint { .. }
            | Self::AppendWireTextBytes { .. }
            | Self::AppendWireScalarSlice { .. }
            | Self::ReadWireExpectedByte { .. }
            | Self::ReadWireScalarVarint { .. }
            | Self::ReadWireByteSlice { .. }
            | Self::ReadWireNestedOpen { .. }
            | Self::ReadWireNestedClose { .. }
            | Self::AppendWireRepeatedScalarVarint { .. }
            | Self::ReadWireRepeatedScalarVarint { .. }
            | Self::WriteReturnRegisterInteger { .. }
            | Self::CopyRuntimeStorageToReturnRegister { .. } => {
                TargetOperationDomain::RuntimeWrite
            }

            Self::CopyPlaces { .. } => TargetOperationDomain::RuntimeCopy,

            Self::WritePlaceInteger { .. } | Self::WriteStorageBitField { .. } => {
                TargetOperationDomain::RuntimeWrite
            }

            Self::WritePlaceBinary { .. } => TargetOperationDomain::RuntimeWrite,
            Self::WritePlaceString { .. } => TargetOperationDomain::RuntimeWrite,
            Self::WritePlaceBoundedBuffer { .. } => TargetOperationDomain::RuntimeWrite,
            Self::WritePlaceAddress { .. } => TargetOperationDomain::RuntimeWrite,

            Self::ReadRuntimeTextLine { .. }
            | Self::ReadRuntimeByte { .. }
            | Self::WriteRuntimeByte { .. } => TargetOperationDomain::RuntimeRead,

            Self::BeginPlatformCall | Self::HostOperation { .. } => {
                TargetOperationDomain::HostBoundary
            }

            Self::MachineHalt
            | Self::MemoryFence(_)
            | Self::InterruptControl(_)
            | Self::FlagsSnapshot { .. }
            | Self::FlagsRestore { .. }
            | Self::MsrRead { .. }
            | Self::MsrWrite { .. }
            | Self::ControlRegisterRead { .. }
            | Self::ControlRegisterWrite { .. }
            | Self::PortWrite { .. }
            | Self::PortRead { .. } => TargetOperationDomain::MachineControl,
        }
    }

    fn crosses_host_boundary(&self) -> bool {
        self.semantic_domain() == TargetOperationDomain::HostBoundary
            || matches!(
                self,
                Self::ReadRuntimeTextLine {
                    source: RuntimeTextReadSource::HostOperation { .. },
                    ..
                } | Self::ReadRuntimeByte {
                    source: RuntimeTextReadSource::HostOperation { .. },
                    ..
                } | Self::WriteRuntimeByte {
                    source: RuntimeTextReadSource::HostOperation { .. },
                    ..
                }
            )
    }
}

impl TargetOperationKind {
    pub fn semantic_domain(&self) -> TargetOperationDomain {
        OperationSemanticQuery::semantic_domain(self)
    }

    pub fn crosses_host_boundary(&self) -> bool {
        OperationSemanticQuery::crosses_host_boundary(self)
    }

    /// The concrete host operation whose binding realizes this crossing.
    /// Structural boundary markers carry no operation and therefore no key.
    pub fn host_operation_key(&self) -> Option<crate::HostOperationKey> {
        match self {
            Self::HostOperation { operation_key, .. }
            | Self::ReadRuntimeTextLine {
                source: RuntimeTextReadSource::HostOperation { operation_key },
                ..
            }
            | Self::ReadRuntimeByte {
                source: RuntimeTextReadSource::HostOperation { operation_key },
                ..
            }
            | Self::WriteRuntimeByte {
                source: RuntimeTextReadSource::HostOperation { operation_key },
                ..
            } => Some(*operation_key),
            _ => None,
        }
    }

    pub fn touches_runtime_storage(&self) -> bool {
        OperationSemanticQuery::touches_runtime_storage(self)
    }
}
