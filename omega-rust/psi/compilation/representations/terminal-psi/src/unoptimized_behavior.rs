//! Semantic operation reach retained before optional code transformations.
//!
//! These are calls, service contributions and outcomes, not a producer's
//! assertion that a policy passed. A composition consumer still follows the
//! selected bodies and opaque contracts. Historical sites need not remain
//! executable after optimization; their namespaces and semantic targets remain
//! explicit so optimization selection cannot erase an admission obligation.

use crate::{OperationKind, TerminalMachine, Terminator};
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
    /// Mechanical projection of the produced body, before optional rewrites.
    /// No selected providers, target policy or source queries enter this step.
    pub fn capture(machine: &TerminalMachine) -> Self {
        let mut operations = Vec::new();
        for block in &machine.blocks {
            for operation in &block.operations {
                if let Some(callee) = operation.kind.direct_callee() {
                    operations.push(UnoptimizedBehaviorOperation::Invocation {
                        block: block.id,
                        site: InvocationSite::Operation(operation.id),
                        callee,
                    });
                }
                match &operation.kind {
                    OperationKind::BoundaryCall { boundary, .. } => {
                        operations.push(UnoptimizedBehaviorOperation::BoundaryInvocation {
                            block: block.id,
                            site: InvocationSite::Operation(operation.id),
                            boundary: *boundary,
                        })
                    }
                    OperationKind::PortWrite { service, .. }
                    | OperationKind::RegisterAccess {
                        service: Some(service),
                        ..
                    } => operations.push(UnoptimizedBehaviorOperation::InstructionService {
                        block: block.id,
                        operation: operation.id,
                        service: *service,
                    }),
                    _ => {}
                }
            }
            match &block.terminator {
                Terminator::InvokeNoNormal { edge, callee, .. } => {
                    operations.push(UnoptimizedBehaviorOperation::Invocation {
                        block: block.id,
                        site: InvocationSite::InvokeNoNormal(*edge),
                        callee: *callee,
                    })
                }
                Terminator::ExternalCompletion { edge, boundary, .. } => {
                    operations.push(UnoptimizedBehaviorOperation::BoundaryInvocation {
                        block: block.id,
                        site: InvocationSite::ExternalCompletion(*edge),
                        boundary: *boundary,
                    })
                }
                Terminator::Crash { cause, .. } => {
                    operations.push(UnoptimizedBehaviorOperation::Crash {
                        block: block.id,
                        cause: *cause,
                    })
                }
                _ => {}
            }
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    if let crate::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
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
