//! Stages 11-15: plan the section name table and roster, serialize the section header table, and lay out the indexed and relative section payloads.
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input; the modules are listed in chain order.

pub(crate) mod relative_section_layout;
pub(crate) mod section_header_bytes;
pub(crate) mod section_name_table;
pub(crate) mod section_payload_roster;
pub(crate) mod section_roster;
