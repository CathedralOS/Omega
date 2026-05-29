use super::AbstractOperationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractOperationDomain {
    FunctionBoundary,
    DispatchControl,
    GuardEvaluation,
    RuntimeTextAssembly,
    RuntimeRead,
    RuntimeWrite,
    RuntimeCopy,
    HostBoundary,
}

impl AbstractOperationKind {
    pub fn semantic_domain(&self) -> AbstractOperationDomain {
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
            | Self::WriteReturnRegisterInteger { .. } => AbstractOperationDomain::RuntimeWrite,

            Self::CopyRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimeFrameIndexed { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeFrameIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimeFrame { .. }
            | Self::CopyRuntimeFrameFixedIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeMachineIndexedToRuntimeStorage { .. }
            | Self::CopyRuntimeStorageToRuntimePointee { .. } => {
                AbstractOperationDomain::RuntimeCopy
            }

            Self::ReadRuntimeTextLine { .. } => AbstractOperationDomain::RuntimeRead,

            Self::BeginPlatformCall
            | Self::HostOperation { .. }
            | Self::PreparePlatformOutputHandle { .. }
            | Self::WritePlatformNewline { .. } => AbstractOperationDomain::HostBoundary,
        }
    }

    pub fn crosses_host_boundary(&self) -> bool {
        self.semantic_domain() == AbstractOperationDomain::HostBoundary
            || matches!(self, Self::ReadRuntimeTextLine { .. })
    }

    pub fn touches_runtime_storage(&self) -> bool {
        matches!(
            self.semantic_domain(),
            AbstractOperationDomain::GuardEvaluation
                | AbstractOperationDomain::RuntimeTextAssembly
                | AbstractOperationDomain::RuntimeRead
                | AbstractOperationDomain::RuntimeWrite
                | AbstractOperationDomain::RuntimeCopy
        )
    }
}
