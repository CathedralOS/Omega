//! Stages 8-10: plan the dynamic tags, serialize the dynamic table and plan its section descriptor.
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input; the modules are listed in chain order.

pub(crate) mod dynamic_table_descriptor;
pub(crate) mod dynamic_tag_bytes;
pub(crate) mod dynamic_tags;
