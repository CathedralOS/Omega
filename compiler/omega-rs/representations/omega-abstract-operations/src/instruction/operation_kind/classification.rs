use super::AbstractOperationKind;
use omega_core::operations::{OperationDomain, OperationSemanticQuery};

pub type AbstractOperationDomain = OperationDomain;

impl OperationSemanticQuery for AbstractOperationKind {
    fn semantic_domain(&self) -> AbstractOperationDomain {
        match self {
            Self::EnterFunction | Self::LeaveFunction => AbstractOperationDomain::FunctionBoundary,

            Self::EnterDispatchLoop { .. }
            | Self::EnterDispatchCase { .. }
            | Self::SetDispatchState { .. }
            | Self::TerminateDispatch
            | Self::LeaveDispatchCase
            | Self::LeaveDispatchLoop => AbstractOperationDomain::DispatchControl,

            Self::EvaluateDispatchGuard { .. }
            | Self::CompareRuntimeTextLiteral { .. }
            | Self::CompareRuntimeTextStorage { .. }
            | Self::CompareRuntimeStorage { .. }
            | Self::CompareRuntimeStorageValue { .. }
            | Self::CompareRuntimeValues { .. } => AbstractOperationDomain::GuardEvaluation,

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
                AbstractOperationDomain::RuntimeTextAssembly
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
            | Self::CopyRuntimeStorageToReturnRegister { .. } => AbstractOperationDomain::RuntimeWrite,

            Self::CopyPlaces { .. }
            | Self::CopyRuntimeMachineDoubleIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeFrameBaseDoubleIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimeMachineDoubleIndexed { .. }
            | Self::CopyRuntimeMachineIndexedToRuntimeMachineIndexed { .. } => AbstractOperationDomain::RuntimeCopy,

            Self::ReadRuntimeTextLine { .. }
            | Self::ReadRuntimeByte { .. }
            | Self::WriteRuntimeByte { .. } => AbstractOperationDomain::RuntimeRead,

            Self::BeginPlatformCall
            | Self::HostOperation { .. }
            | Self::PreparePlatformOutputHandle { .. }
            | Self::WritePlatformNewline { .. } => AbstractOperationDomain::HostBoundary,

            Self::MachineHalt | Self::PortWrite { .. } | Self::PortRead { .. } => {
                AbstractOperationDomain::MachineControl
            }
        }
    }

    fn crosses_host_boundary(&self) -> bool {
        self.semantic_domain() == AbstractOperationDomain::HostBoundary
            || matches!(
                self,
                Self::ReadRuntimeTextLine { .. }
                    | Self::ReadRuntimeByte { .. }
                    | Self::WriteRuntimeByte { .. }
            )
    }
}

impl AbstractOperationKind {
    pub fn semantic_domain(&self) -> AbstractOperationDomain {
        OperationSemanticQuery::semantic_domain(self)
    }

    pub fn crosses_host_boundary(&self) -> bool {
        OperationSemanticQuery::crosses_host_boundary(self)
    }

    pub fn touches_runtime_storage(&self) -> bool {
        OperationSemanticQuery::touches_runtime_storage(self)
    }
}
