use crate::CrashRouteBucket;
use semantic_vocabulary::{MachineId, OperationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOperationCrashContract {
    pub machine: MachineId,
    pub operation: OperationId,
    pub published_routes: Vec<CrashRouteBucket>,
    pub crash_continuations: Vec<CrashRouteBucket>,
}
