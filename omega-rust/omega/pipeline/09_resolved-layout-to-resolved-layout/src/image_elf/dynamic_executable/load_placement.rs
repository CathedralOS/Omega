//! Stages 16-18: plan the load layout, apply the section header placements and the dynamic address fixups.
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input; the modules are listed in chain order.

pub(crate) mod load_layout;
pub(crate) mod placed_section_headers;
pub(crate) mod resolved_dynamic_table;
