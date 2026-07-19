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
            | Self::CompareRuntimeValues { .. }
            | Self::ComparePlaces { .. }
            | Self::ComparePlaceValue { .. } => AbstractOperationDomain::GuardEvaluation,

            Self::WriteRuntimeTextLiteral { .. }
            | Self::WriteRuntimeTextLiteralSegment { .. }
            | Self::AppendRuntimeTextStoredSuffix { .. }
            | Self::MaterializeTextBufferToPlace { .. }
            | Self::AppendTextStoredToPlace { .. }
            | Self::AppendTextLiteralToPlace { .. } => AbstractOperationDomain::RuntimeTextAssembly,

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
                AbstractOperationDomain::RuntimeWrite
            }

            Self::CopyPlaces { .. } => AbstractOperationDomain::RuntimeCopy,

            Self::WritePlaceInteger { .. } => AbstractOperationDomain::RuntimeWrite,

            Self::WritePlaceBinary { .. } => AbstractOperationDomain::RuntimeWrite,
            Self::WritePlaceString { .. } => AbstractOperationDomain::RuntimeWrite,
            Self::WritePlaceBoundedBuffer { .. } => AbstractOperationDomain::RuntimeWrite,
            Self::WritePlaceAddress { .. } => AbstractOperationDomain::RuntimeWrite,

            Self::ReadRuntimeTextLine { .. }
            | Self::ReadRuntimeByte { .. }
            | Self::WriteRuntimeByte { .. } => AbstractOperationDomain::RuntimeRead,

            Self::BeginPlatformCall
            | Self::HostOperation { .. }
            | Self::PreparePlatformOutputHandle { .. }
            | Self::WritePlatformNewline { .. } => AbstractOperationDomain::HostBoundary,

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
            | Self::PortRead { .. } => AbstractOperationDomain::MachineControl,
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
