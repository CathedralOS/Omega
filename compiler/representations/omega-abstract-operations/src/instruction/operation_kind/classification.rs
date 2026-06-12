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

            Self::WriteRuntimeMachineInteger { .. }
            | Self::WriteRuntimeStorageInteger { .. }
            | Self::WriteRuntimePointeeInteger { .. }
            | Self::WriteRuntimeStorageBinary { .. }
            | Self::WriteRuntimeStorageConvert { .. }
            | Self::WriteRuntimePointeeBinary { .. }
            | Self::WriteRuntimeFrameIndexedInteger { .. }
            | Self::WriteRuntimeFrameBaseIndexedInteger { .. }
            | Self::WriteRuntimeMachineIndexedInteger { .. }
            | Self::WriteRuntimeFrameIndexedBinary { .. }
            | Self::WriteRuntimeFrameBaseIndexedBinary { .. }
            | Self::WriteRuntimeMachineString { .. }
            | Self::WriteRuntimeFrameString { .. }
            | Self::WriteRuntimePointeeString { .. }
            | Self::WriteRuntimeFrameIndexedString { .. }
            | Self::WriteRuntimeMachineIndexedString { .. }
            | Self::WriteRuntimeStorageAddressToRuntimeFrame { .. }
            | Self::WriteRuntimePointeeAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeFrameIndexedAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame { .. }
            | Self::AppendWireLiteralByte { .. }
            | Self::AppendWireScalarVarint { .. }
            | Self::AppendWireTextBytes { .. }
            | Self::ReadWireExpectedByte { .. }
            | Self::ReadWireScalarVarint { .. }
            | Self::WriteReturnRegisterInteger { .. }
            | Self::CopyRuntimeStorageToReturnRegister { .. } => AbstractOperationDomain::RuntimeWrite,

            Self::CopyRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimeFrameIndexed { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimePointee { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimePointee { .. }
            | Self::CopyRuntimeMachineIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimePointee { .. }
            | Self::CopyRuntimePointeeToRuntimeFrame { .. } => AbstractOperationDomain::RuntimeCopy,

            Self::ReadRuntimeTextLine { .. } => AbstractOperationDomain::RuntimeRead,

            Self::BeginPlatformCall
            | Self::HostOperation { .. }
            | Self::PreparePlatformOutputHandle { .. }
            | Self::WritePlatformNewline { .. } => AbstractOperationDomain::HostBoundary,
        }
    }

    fn crosses_host_boundary(&self) -> bool {
        self.semantic_domain() == AbstractOperationDomain::HostBoundary
            || matches!(self, Self::ReadRuntimeTextLine { .. })
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
