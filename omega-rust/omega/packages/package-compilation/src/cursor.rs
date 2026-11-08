// Shared bounded byte cursor for wire decoders in `package-compilation`,
// re-exported from `optimization_core::cursor`. Truncation is the only failure
// the cursor itself owns; each decoder maps it into its own domain error.

pub(crate) use optimization_core::cursor::{Cursor, TakeError, Truncated};
