//! Exact selected-conformance applications and callable bounds.

mod application;
mod bounds;
mod model;

pub(crate) use application::project_selected_conformance_application;
pub(crate) use bounds::project_conformance_bounds;
