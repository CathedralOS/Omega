use super::TargetOperationKind;
use crate::RuntimeTextReadSource;
use omega_core::operations::{OperationDomain, OperationSemanticQuery};

pub type TargetOperationDomain = OperationDomain;

impl OperationSemanticQuery for TargetOperationKind {
    fn semantic_domain(&self) -> TargetOperationDomain {
        match self {
            Self::EnterFunction | Self::LeaveFunction => TargetOperationDomain::FunctionBoundary,

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

            Self::AtomicFetchAdd { .. }
            | Self::AtomicCompareExchange { .. }
            | Self::WriteEntryArgumentRegister { .. }
            | Self::WriteEntryArgumentsSliceDescriptor { .. }
            | Self::WriteRuntimeStorageConvert { .. }
            | Self::AppendRuntimeMachineBoundedBufferSource { .. }
            | Self::AppendRuntimeMachineBoundedBufferLiteral { .. }
            | Self::AppendWireLiteralByte { .. }
            | Self::AppendWireScalarVarint { .. }
            | Self::AppendWireTextBytes { .. }
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

            Self::WritePlaceInteger { .. } => TargetOperationDomain::RuntimeWrite,

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

    pub fn touches_runtime_storage(&self) -> bool {
        OperationSemanticQuery::touches_runtime_storage(self)
    }
}
