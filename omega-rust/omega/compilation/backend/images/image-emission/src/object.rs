//! The object lane: the sealed `object_artifact` carrier, `function_fragments`
//! as its only builder, `private_object_composition` linking callback body
//! objects without flattening their namespaces, and the component container
//! callables in `component_object` and `component_shared_image`.

pub(crate) mod component_object;
pub(crate) mod component_shared_image;
pub(crate) mod function_fragments;
pub(crate) mod object_artifact;
pub(crate) mod private_object_composition;
