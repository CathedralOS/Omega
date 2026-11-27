//! Bind a provider demand to the exact Psi artifact authorized by the receiving host.
#![forbid(unsafe_code)]

use provider_demand::{IssuedProviderDemand, ProviderDemandInputs};
use psi_authorization::{ArtifactTrustError, AuthorizedTerminalArtifact};

/// Stage 01 consumes the demand together with the authorization checked here.
pub struct AdmittedProviderDemand<Publisher> {
    demand: IssuedProviderDemand<Publisher>,
    authorization: AuthorizedTerminalArtifact,
}

pub fn admit<Publisher>(
    authorization: AuthorizedTerminalArtifact,
    demand: IssuedProviderDemand<Publisher>,
) -> Result<AdmittedProviderDemand<Publisher>, ArtifactTrustError> {
    let artifact = match demand.inputs() {
        ProviderDemandInputs::Unscoped { artifact, .. } => artifact,
        ProviderDemandInputs::Target { origins, .. } => origins.artifact(),
    };
    authorization.require_artifact(artifact)?;
    Ok(AdmittedProviderDemand {
        demand,
        authorization,
    })
}

impl<Publisher> AdmittedProviderDemand<Publisher> {
    pub fn inputs(&self) -> &ProviderDemandInputs {
        self.demand.inputs()
    }

    pub fn into_parts(self) -> (IssuedProviderDemand<Publisher>, AuthorizedTerminalArtifact) {
        (self.demand, self.authorization)
    }
}

#[cfg(test)]
mod tests {
    use super::admit;
    use provider_demand::{IssuedProviderDemand, ProviderDemandInputs};
    use psi_authorization::{ArtifactTrustError, HostProducerTrust, authorize_terminal_artifact};
    use std::sync::Arc;
    use terminal_codec::{
        CanonicalTerminalArtifact, TerminalDebugMap, build_identity_optimization_execution_record,
    };

    #[test]
    fn provider_admission_binds_host_trust_to_the_exact_published_artifact() {
        let module = terminal_psi::TerminalModule::declarations();
        let record = build_identity_optimization_execution_record(&module).unwrap();
        let original = Arc::new(
            CanonicalTerminalArtifact::from_parts(module.clone(), record.clone(), None).unwrap(),
        );
        let issue = || {
            IssuedProviderDemand::issue(
                (),
                ProviderDemandInputs::Unscoped {
                    artifact: Arc::clone(&original),
                    roots: Vec::new(),
                },
            )
        };

        let receive = |artifact, producer: Option<&HostProducerTrust>| {
            let authorization = authorize_terminal_artifact(artifact, producer)?;
            admit(authorization, issue())
        };
        let reopened =
            Arc::new(CanonicalTerminalArtifact::from_bytes(&original.to_bytes()).unwrap());
        assert!(matches!(
            receive(Arc::clone(&reopened), None),
            Err(ArtifactTrustError::ProducerTrustRequired)
        ));
        let host = HostProducerTrust::accept("admission test");
        let (_, admitted) = receive(Arc::clone(&reopened), Some(&host))
            .unwrap()
            .into_parts();
        assert_eq!(admitted.artifact(), reopened.as_ref());
        assert!(!admitted.artifact().requires_host_trust());

        let debug = TerminalDebugMap {
            semantic: original.view().identity(),
            files: Vec::new(),
            sites: Vec::new(),
        };
        let changed =
            Arc::new(CanonicalTerminalArtifact::from_parts(module, record, Some(&debug)).unwrap());
        assert_eq!(changed.view().identity(), original.view().identity());
        let authorized_identity = changed.manifest().identity();
        let demanded_identity = original.manifest().identity();
        assert_ne!(authorized_identity, demanded_identity);
        assert!(matches!(
            receive(changed, Some(&host)),
            Err(ArtifactTrustError::ArtifactMismatch { authorized, requested })
                if authorized == authorized_identity && requested == demanded_identity
        ));
    }
}
