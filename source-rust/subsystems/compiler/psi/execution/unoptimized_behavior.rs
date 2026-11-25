use crate::{ExternalReach, TerminalMachine, Terminator};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, CrashCause, InvocationSite, MachineId, OperationId, ServiceId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnoptimizedMachineBehavior {
    pub machine: MachineId,
    pub operations: Vec<UnoptimizedBehaviorOperation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnoptimizedBehaviorOperation {
    Invocation {
        block: BlockId,
        site: InvocationSite,
        callee: MachineId,
    },
    BoundaryInvocation {
        block: BlockId,
        site: InvocationSite,
        boundary: BoundaryMachineId,
    },
    InstructionService {
        block: BlockId,
        operation: OperationId,
        service: ServiceId,
    },
    Crash {
        block: BlockId,
        cause: CrashCause,
    },
    NominalCleanup {
        block: BlockId,
        callee: MachineId,
    },
}

impl UnoptimizedMachineBehavior {
    pub fn capture(machine: &TerminalMachine) -> Self {
        let mut operations = Vec::new();
        for block in &machine.blocks {
            for operation in &block.operations {
                let site = InvocationSite::Operation(operation.id);
                match operation.kind.external_reach() {
                    Some(ExternalReach::Machine(callee)) => {
                        operations.push(UnoptimizedBehaviorOperation::Invocation {
                            block: block.id,
                            site,
                            callee,
                        })
                    }
                    Some(ExternalReach::Boundary(boundary)) => {
                        operations.push(UnoptimizedBehaviorOperation::BoundaryInvocation {
                            block: block.id,
                            site,
                            boundary,
                        })
                    }
                    Some(ExternalReach::InstructionService(Some(service))) => {
                        operations.push(UnoptimizedBehaviorOperation::InstructionService {
                            block: block.id,
                            operation: operation.id,
                            service,
                        })
                    }
                    // The inventory records named services and callees only.
                    Some(ExternalReach::InstructionService(None) | ExternalReach::ComputedCall)
                    | None => {}
                }
            }
            match block.terminator.external_reach() {
                Some((site, ExternalReach::Machine(callee))) => {
                    operations.push(UnoptimizedBehaviorOperation::Invocation {
                        block: block.id,
                        site,
                        callee,
                    })
                }
                Some((site, ExternalReach::Boundary(boundary))) => {
                    operations.push(UnoptimizedBehaviorOperation::BoundaryInvocation {
                        block: block.id,
                        site,
                        boundary,
                    })
                }
                Some((
                    _,
                    ExternalReach::InstructionService(_) | ExternalReach::ComputedCall,
                ))
                | None => {}
            }
            if let Terminator::Crash { cause, .. } = &block.terminator {
                operations.push(UnoptimizedBehaviorOperation::Crash {
                    block: block.id,
                    cause: *cause,
                });
            }
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    if let crate::TerminalAffineCleanupAction::InvokeOwned(cleanup) = action {
                        operations.push(UnoptimizedBehaviorOperation::NominalCleanup {
                            block: block.id,
                            callee: cleanup.cleanup_machine,
                        });
                    }
                }
            }
        }
        Self {
            machine: machine.id,
            operations,
        }
    }
}
