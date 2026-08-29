//! Digest-bound, conservative edit plans for dependency declarations.

mod layout;
mod model;
mod planning;
mod rendering;

pub use model::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildFileReplacement,
};
pub use planning::{plan_dependency_addition, plan_dependency_replacement};
pub use rendering::canonical_dependency_statement;

const BUILD_FILE_NAME: &str = "build.omg";
const BUILD_MACHINE_NAME: &str = "build";
const BUILDER_PARAMETER_NAME: &str = "builder";

#[cfg(test)]
mod tests;
