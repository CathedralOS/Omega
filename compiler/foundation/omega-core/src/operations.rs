#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationDomain {
    FunctionBoundary,
    DispatchControl,
    GuardEvaluation,
    RuntimeTextAssembly,
    RuntimeRead,
    RuntimeWrite,
    RuntimeCopy,
    HostBoundary,
}

impl OperationDomain {
    pub fn touches_runtime_storage(self) -> bool {
        matches!(
            self,
            Self::GuardEvaluation
                | Self::RuntimeTextAssembly
                | Self::RuntimeRead
                | Self::RuntimeWrite
                | Self::RuntimeCopy
        )
    }
}

pub trait OperationSemanticQuery {
    fn semantic_domain(&self) -> OperationDomain;

    fn crosses_host_boundary(&self) -> bool {
        self.semantic_domain() == OperationDomain::HostBoundary
    }

    fn touches_runtime_storage(&self) -> bool {
        self.semantic_domain().touches_runtime_storage()
    }
}
