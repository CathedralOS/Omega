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
            | Self::CompareRuntimeStorage { .. }
            | Self::CompareRuntimeStorageValue { .. }
            | Self::CompareRuntimeValues { .. } => TargetOperationDomain::GuardEvaluation,

            Self::WriteRuntimeTextLiteral { .. }
            | Self::WriteRuntimeTextLiteralSegment { .. }
            | Self::AppendRuntimeTextStoredSuffix { .. }
            | Self::MaterializeRuntimeTextBuffer { .. }
            | Self::MaterializeRuntimeTextBufferToRuntimePointee { .. }
            | Self::MaterializeRuntimeTextBufferToRuntimeFrameIndexed { .. }
            | Self::AppendRuntimeTextStoredPlace { .. }
            | Self::AppendRuntimeTextStoredPlaceToRuntimePointee { .. }
            | Self::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed { .. }
            | Self::AppendRuntimeTextLiteral { .. }
            | Self::AppendRuntimeTextLiteralToRuntimePointee { .. }
            | Self::AppendRuntimeTextLiteralToRuntimeFrameIndexed { .. } => {
                TargetOperationDomain::RuntimeTextAssembly
            }

            Self::AtomicFetchAdd { .. }
            | Self::AtomicCompareExchange { .. }
            | Self::WriteRuntimeMachineInteger { .. }
            | Self::WriteRuntimeStorageInteger { .. }
            | Self::WriteEntryArgumentRegister { .. }
            | Self::WriteEntryArgumentsSliceDescriptor { .. }
            | Self::WriteRuntimePointeeInteger { .. }
            | Self::WriteRuntimeStorageBinary { .. }
            | Self::WriteRuntimeStorageConvert { .. }
            | Self::WriteRuntimePointeeBinary { .. }
            | Self::WriteRuntimeFrameIndexedInteger { .. }
            | Self::WriteRuntimeFrameBaseIndexedInteger { .. }
            | Self::WriteRuntimeMachineIndexedInteger { .. }
            | Self::WriteRuntimeMachineDoubleIndexedInteger { .. }
            | Self::WriteRuntimeFrameIndexedBinary { .. }
            | Self::WriteRuntimeFrameBaseIndexedBinary { .. }
            | Self::WriteRuntimeMachineIndexedBinary { .. }
            | Self::WriteRuntimeMachineDoubleIndexedBinary { .. }
            | Self::WriteRuntimeMachineString { .. }
            | Self::WriteRuntimeMachineBoundedBuffer { .. }
            | Self::AppendRuntimeMachineBoundedBufferSource { .. }
            | Self::AppendRuntimeMachineBoundedBufferLiteral { .. }
            | Self::WriteRuntimeFrameString { .. }
            | Self::WriteRuntimePointeeString { .. }
            | Self::WriteRuntimePointeeBoundedBuffer { .. }
            | Self::WriteRuntimeFrameIndexedString { .. }
            | Self::WriteRuntimeMachineIndexedString { .. }
            | Self::WriteRuntimeStorageAddressToRuntimeFrame { .. }
            | Self::WriteRuntimePointeeAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeFrameIndexedAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeMachineIndexedAddressToRuntimeFrame { .. }
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
            | Self::CopyRuntimeStorageToReturnRegister { .. } => TargetOperationDomain::RuntimeWrite,

            Self::CopyPlaces { .. }
            | Self::CopyRuntimeMachineDoubleIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeFrameBaseDoubleIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimeMachineDoubleIndexed { .. }
            | Self::CopyRuntimeFrameBaseIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeMachineIndexedToRuntimeMachineIndexed { .. } => TargetOperationDomain::RuntimeCopy,

            Self::ReadRuntimeTextLine { .. }
            | Self::ReadRuntimeByte { .. }
            | Self::WriteRuntimeByte { .. } => TargetOperationDomain::RuntimeRead,

            Self::BeginPlatformCall | Self::HostOperation { .. } => {
                TargetOperationDomain::HostBoundary
            }

            Self::MachineHalt | Self::PortWrite { .. } | Self::PortRead { .. } => {
                TargetOperationDomain::MachineControl
            }
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
