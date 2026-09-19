//! Native artifact inputs owned by compiler library tests, without execution claims.

pub(crate) const NO_SELECTION_EMPTY_ENTRY: &str = "optimizer/no_selection_empty_entry";
pub(crate) const CORE_DROP_EXPLICIT_CONSUME: &str = "drops/core_drop_explicit_consume";

#[allow(dead_code, reason = "inventory entrypoint shared with library tests")]
pub(crate) const PASS_CANARIES: &[&str] = &[NO_SELECTION_EMPTY_ENTRY, CORE_DROP_EXPLICIT_CONSUME];
