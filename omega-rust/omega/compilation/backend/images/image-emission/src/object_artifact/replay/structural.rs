//! Structural-value replay: aggregate layout, condition reads, structural
//! returns, affine projected calls, and partial cleanup partitions.

pub(crate) mod affine_projected_calls;
pub(crate) mod condition_layout;
pub(crate) mod condition_read;
pub(crate) mod partial_cleanup_partition;
pub(crate) mod return_record;
