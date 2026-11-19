//! Closed logical choices a publisher issues for one portable program.
#![forbid(unsafe_code)]

use diagnostics::Diagnostic;
use evaluation_receipts::NativeBindingMaterialization;
use host_provider_inputs::ProviderExecutionOrigins;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::sync::Arc;
use terminal_semantics::{
    callable_values::CallableBoundaryImplementation, logical_providers::ClosedLogicalProviders,
};

/// The package declarations that must be reviewed even when no executable entry reaches them.
pub struct PackageReviewRequirements {
    pub numeric: Result<Vec<BoundaryMachineId>, Vec<Diagnostic>>,
    pub bindings: Result<Vec<(BoundaryMachineId, NativeBindingMaterialization)>, Vec<Diagnostic>>,
}

pub enum ProviderDemandInputs {
    Unscoped {
        artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
        roots: Vec<MachineId>,
    },
    Target {
        profile: target::TargetProfile,
        logical: ClosedLogicalProviders,
        has_bound_applications: bool,
        bodies: Vec<CallableBoundaryImplementation>,
        labels: Vec<(BoundaryMachineId, String)>,
        origins: ProviderExecutionOrigins,
        review: Option<PackageReviewRequirements>,
    },
}

/// An issuer hands over a complete request. Taking its facts consumes the
/// request and never yields the issuer needed to authorize another set.
#[derive(Debug, PartialEq, Eq)]
pub struct ProviderDemandAuthority<Publisher>(Publisher);

impl<Publisher> ProviderDemandAuthority<Publisher> {
    pub fn publisher(&self) -> &Publisher {
        &self.0
    }
}

pub struct IssuedProviderDemand<Publisher> {
    inputs: ProviderDemandInputs,
    publisher: Arc<ProviderDemandAuthority<Publisher>>,
}

impl<Publisher> IssuedProviderDemand<Publisher> {
    pub fn issue(publisher: Publisher, inputs: ProviderDemandInputs) -> Self {
        Self {
            inputs,
            publisher: Arc::new(ProviderDemandAuthority(publisher)),
        }
    }

    pub fn inputs(&self) -> &ProviderDemandInputs {
        &self.inputs
    }

    pub fn into_parts(
        self,
    ) -> (
        ProviderDemandInputs,
        Arc<ProviderDemandAuthority<Publisher>>,
    ) {
        (self.inputs, self.publisher)
    }
}
