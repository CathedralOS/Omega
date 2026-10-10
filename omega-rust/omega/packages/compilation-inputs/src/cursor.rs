//! Shared bounded byte cursor for the crate's wire decoders: a slice view
//! plus a position. Truncation is the only failure the cursor itself owns;
//! each decoder maps it into its own error type.

pub(crate) use optimization_core::cursor::{Cursor, TakeError, Truncated};
