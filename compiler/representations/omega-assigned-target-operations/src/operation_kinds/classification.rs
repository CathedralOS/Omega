use super::AssignedOperationKind;
use omega_target_operations::RuntimeTextReadSource;

pub type AssignedOperationDomain = omega_core::operations::OperationDomain;

impl AssignedOperationKind {
    pub fn semantic_domain(&self) -> AssignedOperationDomain {
        match self {
            Self::EnterFunction | Self::LeaveFunction => AssignedOperationDomain::FunctionBoundary,

            Self::EnterDispatchLoop { .. }
            | Self::EnterDispatchCase { .. }
            | Self::SetDispatchState { .. }
            | Self::TerminateDispatch
            | Self::LeaveDispatchCase
            | Self::LeaveDispatchLoop => AssignedOperationDomain::DispatchControl,

            Self::EvaluateDispatchGuard { .. }
            | Self::CompareRuntimeTextLiteral { .. }
            | Self::CompareRuntimeTextStorage { .. }
            | Self::CompareRuntimeStorage { .. }
            | Self::CompareRuntimeStorageValue { .. }
            | Self::CompareRuntimeValues { .. } => AssignedOperationDomain::GuardEvaluation,

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
                AssignedOperationDomain::RuntimeTextAssembly
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
            | Self::WriteReturnRegisterInteger { .. } => AssignedOperationDomain::RuntimeWrite,

            Self::CopyRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimeFrameIndexed { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeMachineIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimePointee { .. } => {
                AssignedOperationDomain::RuntimeCopy
            }

            Self::ReadRuntimeTextLine { .. } => AssignedOperationDomain::RuntimeRead,

            Self::BeginPlatformCall | Self::HostOperation { .. } => {
                AssignedOperationDomain::HostBoundary
            }
        }
    }

    pub fn crosses_host_boundary(&self) -> bool {
        self.semantic_domain() == AssignedOperationDomain::HostBoundary
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
            AssignedOperationDomain::GuardEvaluation
                | AssignedOperationDomain::RuntimeTextAssembly
                | AssignedOperationDomain::RuntimeRead
                | AssignedOperationDomain::RuntimeWrite
                | AssignedOperationDomain::RuntimeCopy
        )
    }
}
