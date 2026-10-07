//! Optimizer module role: crate map. Native product coordination over the program stages.
//!
//! This compiler owner accepts Terminal Psi and a separate realization request,
//! settles entry/provider custody, and sequences transforms through native
//! publication. It is not another program stage: the Omega program route
//! begins at `terminal-psi-to-abstract-operations`. Target setup is supplied by
//! backend owners. Component policy and installation remain outside this owner.
//!
//! Fresh compilation first produces the retained Terminal boundary:
//! `produce_native_product` packages checked source, then calls
//! `realize_retained_native_artifact`, exactly as package re-entry does.
//! Native realization never reads the checked source or selects providers again.

mod native_product;
mod native_realization;
mod physical_pipeline;
mod retained_native_product;

pub use native_product::{ProducedNativeProduct, produce_native_product};
pub use native_realization::{RealizedNativeProduct, realize_native_artifact};
pub use physical_pipeline::{
    OptimizedVerifiedPhysicalPipelineError, stage_optimized_verified_physical_pipeline,
};
pub use retained_native_product::realize_retained_native_artifact;
