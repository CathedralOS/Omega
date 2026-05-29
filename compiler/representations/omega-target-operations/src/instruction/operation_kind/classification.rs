use super::TargetOperationKind;
use crate::RuntimeTextReadSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOperationDomain {
    FunctionBoundary,
    DispatchControl,
    GuardEvaluation,
    RuntimeTextAssembly,
    RuntimeRead,
    RuntimeWrite,
    RuntimeCopy,
    HostBoundary,
}

impl TargetOperationKind {
    pub fn semantic_domain(&self) -> TargetOperationDomain {
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
            | Self::WriteRuntimePointeeBinary { .. }
            | Self::WriteRuntimeFrameIndexedInteger { .. }
            | Self::WriteRuntimeFrameBaseIndexedInteger { .. }
            | Self::WriteRuntimeMachineIndexedInteger { .. }
            | Self::WriteRuntimeFrameIndexedBinary { .. }
            | Self::WriteRuntimeFrameBaseIndexedBinary { .. }
            | Self::WriteRuntimeMachineString { .. }
            | Self::WriteRuntimePointeeString { .. }
            | Self::WriteRuntimeFrameIndexedString { .. }
            | Self::WriteRuntimeMachineIndexedString { .. }
            | Self::WriteRuntimeStorageAddressToRuntimeFrame { .. }
            | Self::WriteRuntimePointeeAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeFrameIndexedAddressToRuntimeFrame { .. }
            | Self::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame { .. }
            | Self::WriteReturnRegisterInteger { .. } => TargetOperationDomain::RuntimeWrite,

            Self::CopyRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimeFrameIndexed { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeMachineIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimePointee { .. } => TargetOperationDomain::RuntimeCopy,

            Self::ReadRuntimeTextLine { .. } => TargetOperationDomain::RuntimeRead,

            Self::BeginPlatformCall | Self::HostOperation { .. } => {
                TargetOperationDomain::HostBoundary
            }
        }
    }

    pub fn crosses_host_boundary(&self) -> bool {
        self.semantic_domain() == TargetOperationDomain::HostBoundary
            || matches!(
                self,
                Self::ReadRuntimeTextLine {
                    source: RuntimeTextReadSource::HostOperation { .. },
                    ..
                }
            )
    }

    pub fn touches_runtime_storage(&self) -> bool {
        matches!(
            self.semantic_domain(),
            TargetOperationDomain::GuardEvaluation
                | TargetOperationDomain::RuntimeTextAssembly
                | TargetOperationDomain::RuntimeRead
                | TargetOperationDomain::RuntimeWrite
                | TargetOperationDomain::RuntimeCopy
        )
    }
}
