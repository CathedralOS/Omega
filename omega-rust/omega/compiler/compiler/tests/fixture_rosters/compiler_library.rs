//! Native artifact inputs owned by compiler library tests, without execution claims.

pub(crate) const NO_SELECTION_EMPTY_ENTRY: &str = "optimizer/no_selection_empty_entry";

#[allow(dead_code, reason = "inventory entrypoint shared with library tests")]
pub(crate) const PASS_CANARIES: &[&str] = &[NO_SELECTION_EMPTY_ENTRY];
