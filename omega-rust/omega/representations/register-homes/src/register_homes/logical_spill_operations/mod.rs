//! Durable logical-spill operation record: the versioned transport for
//! target-neutral recovery obligations — store, reload, and operand-rewrite
//! records keyed by canonical identity. Computation, validation, and
//! replay-against-roots live in the producing transform; decoding grants no
//! spill authority.

mod codec;
mod identity;
mod model;

pub use identity::logical_spill_operation_identity;
pub use model::{
    FunctionLogicalSpillOperations, LogicalReloadValueId, LogicalSpillAction,
    LogicalSpillOperationDecodeError, LogicalSpillOperationIdentity, LogicalSpillOperationPlan,
    LogicalSpillOperationPolicy, LogicalSpillReload, LogicalSpillStorage, LogicalSpillStorageClass,
    LogicalSpillStorageId, LogicalSpillStore, LogicalSpillUseRewrite,
};
