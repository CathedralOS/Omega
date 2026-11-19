//! Source-issued provider choices and completed Build bindings for one Psi publication and target.
#![forbid(unsafe_code)]

use psi_provider_selections::logical_reach::ClosedLogicalProviderReach;
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

pub fn issue_target_provider_demand(
    artifact: &CanonicalTerminalArtifact,
    target: &str,
    demand: SourceProviderDemand,
) -> PublishedTargetProviders {
    PublishedTargetProviders {
        manifest: artifact.manifest().identity(),
        target: target.to_owned(),
        demand,
    }
}

impl PublishedTargetProviders {
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

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_codec::{TerminalDebugMap, build_identity_optimization_execution_record};

    #[test]
    fn source_choices_follow_the_exact_publication_and_target() {
        let module = terminal_psi::TerminalModule::declarations();
        let record = build_identity_optimization_execution_record(&module).unwrap();
        let source =
            CanonicalTerminalArtifact::from_parts(module.clone(), record.clone(), None).unwrap();
        let reopened = CanonicalTerminalArtifact::from_bytes(&source.to_bytes()).unwrap();
        let debug = TerminalDebugMap {
            semantic: source.view().identity(),
            files: Vec::new(),
            sites: Vec::new(),
        };
        let other = CanonicalTerminalArtifact::from_parts(module, record, Some(&debug)).unwrap();
        assert_eq!(other.view().identity(), source.view().identity());
        assert_ne!(other.manifest().identity(), source.manifest().identity());

        let target = "linux_x86_64";
        let closed =
            psi_provider_selections::logical_reach::close_providers(source.semantic_module(), target, None, &[])
                .unwrap();
        let publish = || {
            issue_target_provider_demand(
                &source,
                target,
                SourceProviderDemand::Target {
                    closed: closed.clone(),
                    bindings: Vec::new(),
                    labels: Vec::new(),
                    review: None,
                },
            )
        };
        match publish().receive(&reopened, target).unwrap() {
            SourceProviderDemand::Target {
                closed: received, ..
            } => assert_eq!(received, closed),
            _ => panic!("receiving this publication changed its source demand"),
        }
        for (artifact, deployment) in [(&other, target), (&reopened, "windows_x86_64")] {
            let refusal = publish().receive(artifact, deployment).err().unwrap();
            assert_eq!(
                refusal[0].message,
                "source provider publication names a different program or deployment target"
            );
        }
    }
}
