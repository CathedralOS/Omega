#![forbid(unsafe_code)]

//! Transactional deployment composition above compilation.
//!
//! Start at `component_deployment.rs` for installation and closure of live
//! authority. `flat_output.rs` publishes an already finalized runnable to disk.
//! Neither compilation nor writing a file substitutes for installation custody.

mod component_deployment;
mod flat_output;

pub use self::component_deployment::{
    BeginClaimedDeploymentError, BeginDeploymentError, ComponentDeploymentSession,
    ComponentProgressAttestationBinding, DeploymentFinalizationError,
    ProgressClosedComponentDeployment, ProgressClosureError, ProviderClosedComponentDeployment,
    ProviderClosureError, begin_component_deployment,
    begin_component_deployment_with_claimed_registry,
};
pub use flat_output::{
    ComponentFlatOutputPublicationError, ComponentFlatOutputReceipt,
    ComponentFlatOutputValidationError, PublishedComponentFlatOutput,
    publish_component_flat_output,
};
