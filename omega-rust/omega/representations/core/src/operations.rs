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
    /// Privileged CPU control instructions (`hlt`, `cli`/`sti`, port
    /// I/O): raw target instructions with `MachineControl`/`PortIo` service
    /// contracts, touching neither runtime storage nor the host boundary.
    MachineControl,
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

#[cfg(test)]
mod tests {
    use super::{OperationDomain, OperationSemanticQuery};

    struct TestOperation {
        domain: OperationDomain,
    }

    impl OperationSemanticQuery for TestOperation {
        fn semantic_domain(&self) -> OperationDomain {
            self.domain
        }
    }

    #[test]
    fn runtime_storage_domains_are_centralized() {
        assert!(OperationDomain::GuardEvaluation.touches_runtime_storage());
        assert!(OperationDomain::RuntimeTextAssembly.touches_runtime_storage());
        assert!(OperationDomain::RuntimeRead.touches_runtime_storage());
        assert!(OperationDomain::RuntimeWrite.touches_runtime_storage());
        assert!(OperationDomain::RuntimeCopy.touches_runtime_storage());

        assert!(!OperationDomain::FunctionBoundary.touches_runtime_storage());
        assert!(!OperationDomain::DispatchControl.touches_runtime_storage());
        assert!(!OperationDomain::HostBoundary.touches_runtime_storage());
    }

    #[test]
    fn semantic_query_defaults_follow_domain_rules() {
        let host = TestOperation {
            domain: OperationDomain::HostBoundary,
        };
        let copy = TestOperation {
            domain: OperationDomain::RuntimeCopy,
        };

        assert!(host.crosses_host_boundary());
        assert!(!host.touches_runtime_storage());
        assert!(!copy.crosses_host_boundary());
        assert!(copy.touches_runtime_storage());
    }
}
