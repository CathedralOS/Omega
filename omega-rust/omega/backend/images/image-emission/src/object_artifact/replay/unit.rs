//! Unit-body replay: frames, internal Unit and scalar calls, continuations,
//! affine cleanup, descriptor joins, and staged primitive stores.

pub(crate) mod affine_cleanup;
pub(crate) mod call_custody;
pub(crate) mod continuations;
pub(crate) mod dynamic_descriptor_join;
pub(crate) mod installed_provider_scalar_call;
pub(crate) mod scalar_call_custody;
pub(crate) mod stack;
pub(crate) mod structural_scalar_field_store;
pub(crate) mod write_only_primitive_store;
