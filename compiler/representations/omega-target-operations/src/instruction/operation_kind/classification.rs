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
            | Self::WriteReturnRegisterInteger { .. }
            | Self::CopyRuntimeStorageToReturnRegister { .. } => TargetOperationDomain::RuntimeWrite,

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
            | Self::CopyRuntimePointeeToRuntimeFrame { .. } => TargetOperationDomain::RuntimeCopy,

            Self::ReadRuntimeTextLine { .. } => TargetOperationDomain::RuntimeRead,

            Self::BeginPlatformCall | Self::HostOperation { .. } => {
                TargetOperationDomain::HostBoundary
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
