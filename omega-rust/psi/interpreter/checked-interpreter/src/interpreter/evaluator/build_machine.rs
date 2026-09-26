//! The build machine's facets: what an `omega build` evaluation reads, writes
//! and must account for, as opposed to ordinary program execution.
//!
//! Each module is one facet the build machine exposes to authored `build.omg`
//! code, and each contributes methods to `Evaluator` rather than a type of its
//! own: `paths` and `log` are the rooted path vocabulary and the build log,
//! `root_bindings` and `provider_selections` are what the build declares,
//! `product_entries`, `product_providers` and `product_schemas` are what it
//! produces, `output_obligations` is what it still owes at the end, and
//! `behavior_exclusions` is what it has been told not to do.

pub(in crate::interpreter::evaluator) mod behavior_exclusions;
pub(in crate::interpreter::evaluator) mod log;
pub(in crate::interpreter::evaluator) mod output_obligations;
pub(in crate::interpreter::evaluator) mod paths;
pub(in crate::interpreter::evaluator) mod product_entries;
pub(in crate::interpreter::evaluator) mod product_providers;
pub(in crate::interpreter::evaluator) mod product_schemas;
pub(in crate::interpreter::evaluator) mod provider_selections;
pub(in crate::interpreter::evaluator) mod root_bindings;
