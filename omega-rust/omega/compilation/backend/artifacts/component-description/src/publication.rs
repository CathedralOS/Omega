//! Serialize a component description and its producer-observed subject from one
//! decoded Terminal module. This produces bytes, not verification or installation.

use crate::component_description::describe_decoded_component_facts;
use crate::{ComponentDescriptionFacts, DescribeError, encode_component_description};
use terminal_psi::TerminalPsiIdentity;

/// Produce canonical description bytes beside the semantic subject observed on
/// the supplied artifact. Package custody binds these coordinates to a package;
/// a later consumer must independently select its expected subject and profile
/// and verify the description. Neither result grants authority.
pub fn publish_component_description(
    facts: ComponentDescriptionFacts<'_>,
) -> Result<(TerminalPsiIdentity, Vec<u8>), DescribeError> {
    let module = terminal_codec::decode_module(facts.artifact.semantic_bytes())
        .map_err(DescribeError::ArtifactDecode)?;
    let description = describe_decoded_component_facts(facts, &module)?;
    let subject =
        terminal_codec::terminal_psi_identity(&module).map_err(DescribeError::ArtifactIdentity)?;
    Ok((subject, encode_component_description(&description)))
}

#[cfg(test)]
mod tests {
    use super::publish_component_description;
    use crate::{
        AdmissionProfile, COMPONENT_DESCRIPTION_SCHEMA_V2, ComponentDescriptionFacts,
        ComponentVerificationRejection, ComponentVerificationRequest, test_support,
        verify_component,
    };
    use effects::SelectedProviderPlanFacts;
    use std::collections::BTreeSet;

    #[test]
    fn publication_preserves_canonical_bytes_and_requires_independent_subject_acceptance() {
        for module in [
            test_support::bare_module(),
            test_support::provider_module("Pick", "PickProvider", "PickProvider::run"),
        ] {
            let proof = terminal_psi::ProofBundle::default();
            let record =
                terminal_codec::build_identity_optimization_execution_record(&module, &proof)
                    .unwrap();
            let artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(
                &module, &proof, &record, None,
            )
            .unwrap();
            let selected = SelectedProviderPlanFacts::from_selected_plans(Vec::new()).unwrap();
            let (subject, bytes) = publish_component_description(ComponentDescriptionFacts {
                artifact: &artifact,
                selected_provider_plans: &selected,
                component_progress: None,
                stack_demand: None,
                realization_identity: None,
            })
            .unwrap();
            assert_eq!(bytes, test_support::describe_module(&module));
            assert_eq!(subject, test_support::module_subject(&module));
            let mut request = ComponentVerificationRequest {
                expected_subject: test_support::module_subject(&module),
                accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V2]),
                accepted_assumptions: BTreeSet::new(),
                admission_profile: AdmissionProfile::default(),
            };
            verify_component(&bytes, &request).unwrap();
            request.expected_subject = test_support::module_subject(
                &test_support::provider_module("Other", "OtherProvider", "OtherProvider::run"),
            );
            assert!(matches!(
                verify_component(&bytes, &request),
                Err(ComponentVerificationRejection::WrongSubject { .. })
            ));
        }
    }
}
