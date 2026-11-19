//! Provider choices and completed Build records issued with one portable program.
use crate::logical_reach::ClosedLogicalProviderReach;
use diagnostics::Diagnostic;
use evaluation_receipts::NativeBindingMaterialization;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_codec::{CanonicalTerminalArtifact, TerminalArtifactIdentity};

pub type SourceNativeBinding = Result<NativeBindingMaterialization, String>;

/// Includes declarations outside the executable entry's reachable requirements.
pub struct SourcePackageReviewRequirements {
    pub numeric: Result<Vec<BoundaryMachineId>, Vec<Diagnostic>>,
    pub bindings: Result<Vec<(BoundaryMachineId, NativeBindingMaterialization)>, Vec<Diagnostic>>,
}

pub enum SourceProviderDemand {
    Unscoped {
        roots: Vec<MachineId>,
    },
    Target {
        closed: ClosedLogicalProviderReach,
        labels: Vec<(BoundaryMachineId, String)>,
        bindings: Vec<SourceNativeBinding>,
        review: Option<SourcePackageReviewRequirements>,
    },
}

/// One publication's selected deployment. Receiving it requires the same program and target.
pub struct PublishedTargetProviders {
    manifest: TerminalArtifactIdentity,
    target: String,
    demand: SourceProviderDemand,
}

impl PublishedTargetProviders {
    pub fn new(
        artifact: &CanonicalTerminalArtifact,
        target: &str,
        demand: SourceProviderDemand,
    ) -> Self {
        Self {
            manifest: artifact.manifest().identity(),
            target: target.to_owned(),
            demand,
        }
    }

    pub fn receive(
        self,
        artifact: &CanonicalTerminalArtifact,
        target: &str,
    ) -> Result<SourceProviderDemand, Vec<Diagnostic>> {
        if self.manifest != artifact.manifest().identity() || self.target != target {
            return Err(vec![Diagnostic::error(
                "source provider publication names a different program or deployment target",
            )]);
        }
        Ok(self.demand)
    }
}
