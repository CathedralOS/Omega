//! Retained semantic operations, never an absence verdict or provider choice.

use super::super::wire_fields::{Id, InvocationSite, List, wire_enum, wire_record};
use semantic_vocabulary::CrashCause;
use terminal_psi::{UnoptimizedBehaviorOperation, UnoptimizedMachineBehavior};

wire_record! {
    Behavior: UnoptimizedMachineBehavior { machine: Id, operations: List<Operation> }
}

wire_enum! {
    Operation: UnoptimizedBehaviorOperation, "unoptimized semantic operation" {
        0 => Invocation { block: Id, site: InvocationSite, callee: Id },
        1 => BoundaryInvocation { block: Id, site: InvocationSite, boundary: Id },
        2 => InstructionService { block: Id, operation: Id, service: Id },
        3 => Crash { block: Id, cause: Cause },
        4 => NominalCleanup { block: Id, callee: Id },
    }
    Cause: CrashCause, "semantic crash cause" {
        0 => Trap,
        1 => Abort,
    }
}
