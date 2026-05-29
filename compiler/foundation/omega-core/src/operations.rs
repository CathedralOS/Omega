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
