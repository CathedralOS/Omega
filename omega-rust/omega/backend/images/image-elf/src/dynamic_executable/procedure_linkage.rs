//! Stages 5-7: plan the procedure-linkage relocations, templates and section descriptors.
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input; the modules are listed in chain order.

pub(crate) mod dynamic_import_relocations;
pub(crate) mod dynamic_linkage_descriptors;
pub(crate) mod dynamic_linkage_templates;
