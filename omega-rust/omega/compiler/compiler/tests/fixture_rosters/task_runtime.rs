//! Exact fixture inputs shared by task lifecycle tests and the corpus inventory.

pub const CORE_TASK_LIFECYCLE_OPERATIONS: &str = "core/task_lifecycle_operations";
pub const CORE_TASK_CORE_SCOPE_LOSS: &str = "core/task_core_scope_loss";
pub const CORE_TASK_PARKED_CONTINUATION_PROJECTION_REJECTED: &str =
    "core/task_parked_continuation_projection_rejected";
pub const CORE_TASK_PARKED_CONTINUATION_RECAST_REJECTED: &str =
    "core/task_parked_continuation_recast_rejected";
pub const CORE_TASK_PARKED_CONTINUATION_ADDRESS_REJECTED: &str =
    "core/task_parked_continuation_address_rejected";
pub const CORE_TASK_PARKED_CONTINUATION_MUTATION_REJECTED: &str =
    "core/task_parked_continuation_mutation_rejected";

pub const BLOCKEXEC_BLOCKING_EXECUTOR_CUSTODY_CLAIMS_COMPILE: &str =
    "blockexec/blocking_executor_custody_claims_compile";
pub const BLOCKEXEC_BLOCKING_EXECUTOR_CONSUMER_DEPEND_COMPILE: &str =
    "blockexec/blocking_executor_consumer_depend_compile";
pub const BLOCKEXEC_BLOCKING_EXECUTOR_ISOLATED_PROVIDER_CONFORMANCE_COMPILE: &str =
    "blockexec/blocking_executor_isolated_provider_conformance_compile";
pub const BLOCKEXEC_BLOCKING_EXECUTOR_TICKET_SCOPE_LOSS: &str =
    "blockexec/blocking_executor_ticket_scope_loss";
pub const BLOCKEXEC_BLOCKING_EXECUTOR_ISOLATED_PROVIDER_INCOMPLETE_CONFORMANCE: &str =
    "blockexec/blocking_executor_isolated_provider_incomplete_conformance";

pub const PASS_CANARIES: &[&str] = &[
    CORE_TASK_LIFECYCLE_OPERATIONS,
    BLOCKEXEC_BLOCKING_EXECUTOR_CUSTODY_CLAIMS_COMPILE,
    BLOCKEXEC_BLOCKING_EXECUTOR_CONSUMER_DEPEND_COMPILE,
    BLOCKEXEC_BLOCKING_EXECUTOR_ISOLATED_PROVIDER_CONFORMANCE_COMPILE,
];

pub const FAIL_CANARIES: &[&str] = &[
    CORE_TASK_CORE_SCOPE_LOSS,
    BLOCKEXEC_BLOCKING_EXECUTOR_TICKET_SCOPE_LOSS,
    BLOCKEXEC_BLOCKING_EXECUTOR_ISOLATED_PROVIDER_INCOMPLETE_CONFORMANCE,
];

pub const PARKED_CONTINUATION_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        "projection",
        CORE_TASK_PARKED_CONTINUATION_PROJECTION_REJECTED,
    ),
    ("recast", CORE_TASK_PARKED_CONTINUATION_RECAST_REJECTED),
    ("address", CORE_TASK_PARKED_CONTINUATION_ADDRESS_REJECTED),
    ("mutation", CORE_TASK_PARKED_CONTINUATION_MUTATION_REJECTED),
];
