//! Stages 1-4: plan the dynamic-link inputs and the dynamic sections, then serialize them and plan their descriptors.
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input; the modules are listed in chain order.

pub(crate) mod dynamic_link;
pub(crate) mod dynamic_section_bytes;
pub(crate) mod dynamic_section_descriptors;
pub(crate) mod dynamic_sections;
