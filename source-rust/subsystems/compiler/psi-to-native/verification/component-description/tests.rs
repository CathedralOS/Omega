//! The provider plan below is a sealed input from the provider-plan writer, which
//! is not part of the receiving dependency graph. The provider-free component is
//! built here from an empty Psi program in the current artifact encoding, so a
//! change to that encoding cannot strand these tests on stale bytes.
use super::*;
use crate::inventory::derive_component_inventory;
use crate::records::{
    ComponentDescription, ComponentEntry, ComponentEntryKind, DescriptionFrontier, EntryEvidence,
    ExportSurface, InstallationObligation, ObligationKind,
};
use crate::retained_plan::ProviderPlanInput;
use std::collections::BTreeSet;
use wire::digest_encoder::content_identity;

/// Describes a program with no machines, imports or providers the way the
/// native publisher does: every roster is derived from the embedded artifact.
pub(crate) fn provider_free_component() -> Vec<u8> {
    let module = terminal_psi::TerminalModule::declarations();
    let optimization = terminal_codec::build_identity_optimization_execution_record(&module)
        .expect("an empty program has an identity optimization record");
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, optimization, None)
            .expect("an empty program is a portable artifact");
    let inventory = derive_component_inventory(artifact.semantic_module())
        .expect("an empty program has a component inventory");
    encode_component_description_checked(&ComponentDescription {
        schema: COMPONENT_DESCRIPTION_SCHEMA_V3,
        frontier: DescriptionFrontier::TerminalArtifactClosure,
        artifact_bytes: artifact.to_bytes(),
        imports: Vec::new(),
        exports: inventory
            .export_contracts
            .iter()
            .map(|contract| ExportSurface {
                identity: contract.identity.clone(),
            })
            .collect(),
        entries: inventory.entries,
        outgoing: Vec::new(),
        service_bounds: inventory.service_bounds,
        custody: inventory.custody,
        providers: Vec::new(),
        // The publisher seals its provider selection even when it is empty; the
        // verifier compares this digest only against a nonempty provider roster.
        provider_closure_digest: [5; 32],
        obligations: Vec::new(),
        assumptions: Vec::new(),
        realization_identity: None,
    })
    .expect("a provider-free description encodes")
}

#[test]
fn received_component_keeps_its_bytes_and_rejects_unproved_exports_and_assumptions() {
    let bytes = provider_free_component();
    let (mut description, artifact) = decode_component_description_with_artifact(&bytes).unwrap();
    assert_eq!(
        encode_component_description_checked(&description).unwrap(),
        bytes
    );
    let request = ComponentVerificationRequest {
        expected_subject: artifact.view().identity(),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V3]),
        accepted_assumptions: AcceptedComponentAssumptions::Exact(BTreeSet::new()),
    };
    let verified = verify_component(&bytes, &request).unwrap();
    assert_eq!(verified.artifact(), &artifact);

    description.exports.push(ExportSurface {
        identity: "invented export".into(),
    });
    let forged = encode_component_description(&description);
    assert!(matches!(verify_component(&forged, &request),
        Err(ComponentVerificationRejection::UnexpectedDerivedExport(identity)) if identity == "invented export"));
    description.exports.pop();
    let assumption = [42; 32];
    description.assumptions.push(assumption);
    let changed = encode_component_description(&description);
    assert!(matches!(verify_component(&changed, &request),
        Err(ComponentVerificationRejection::UnacceptedAssumption(digest)) if digest == assumption));
}

#[test]
fn retained_plan_authenticates_the_entire_preimage_and_its_schema() {
    let bytes = include_bytes!("tests/checked-provider.v8");
    let commitment = content_identity(bytes);
    let plan = ProviderPlanInput::read(bytes, &commitment).unwrap();
    assert_eq!(plan.provider, "Provider");
    assert_eq!(plan.rows.len(), 1);
    assert_eq!(plan.rows[0].requirement, "Boundary::consume(opaque)");
    assert_eq!(
        plan.rows[0].checked_adapter,
        Some(("Provider::consume(opaque)", None))
    );

    // The published method has one parameter. Rehashing an invocation of
    // parameter 1 cannot make it part of that signature.
    let tag = bytes
        .windows(b"scheduler".len())
        .position(|part| part == b"scheduler")
        .unwrap();
    let parameter = tag - (8 + 1 + 4);
    let mut wrong_position = bytes.to_vec();
    wrong_position[parameter..parameter + 4].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        ProviderPlanInput::read(&wrong_position, &content_identity(&wrong_position)).unwrap_err(),
        "synchronous invocation refers to a parameter outside the signature"
    );

    let field = bytes
        .windows(b"field".len())
        .position(|part| part == b"field")
        .unwrap();
    // The historical record starts with three ordinary result projections.
    // Insert the new input selector before them. It takes a public parameter
    // ordinal, unlike field/case/index selectors which continue the path.
    let path_start = field - 8 - 1;
    let mut first_input = bytes.to_vec();
    assert_eq!(
        &first_input[path_start - 8..path_start],
        &3u64.to_le_bytes()
    );
    first_input[path_start - 8..path_start].copy_from_slice(&4u64.to_le_bytes());
    first_input.splice(path_start..path_start, [4, 0, 0, 0, 0]);
    ProviderPlanInput::read(&first_input, &content_identity(&first_input)).unwrap();
    first_input[path_start + 1..path_start + 5].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        ProviderPlanInput::read(&first_input, &content_identity(&first_input)).unwrap_err(),
        "membership outside method signature"
    );

    let mut wrong_projection = bytes.to_vec();
    wrong_projection[path_start] = 0;
    assert_eq!(
        ProviderPlanInput::read(&wrong_projection, &content_identity(&wrong_projection))
            .unwrap_err(),
        "invalid provider membership selector"
    );

    let mut changed = bytes.to_vec();
    changed.push(0);
    assert_eq!(
        ProviderPlanInput::read(&changed, &commitment).unwrap_err(),
        "provider plan input differs from its complete commitment"
    );
    assert_eq!(
        ProviderPlanInput::read(&changed, &content_identity(&changed)).unwrap_err(),
        "provider plan input has trailing bytes"
    );
}

#[test]
fn assumption_rows_bind_the_review_to_verified_description_bytes() {
    let (mut description, artifact) =
        decode_component_description_with_artifact(&provider_free_component()).unwrap();
    let assumption = [42; 32];
    description.assumptions.push(assumption);
    description.entries.push(ComponentEntry {
        kind: ComponentEntryKind::Cleanup,
        identity: "device shutdown".to_owned(),
        evidence: EntryEvidence::AssumptionBound(assumption),
    });
    description.entries.sort();
    let request = ComponentVerificationRequest {
        expected_subject: artifact.view().identity(),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V3]),
        accepted_assumptions: AcceptedComponentAssumptions::PendingConsumerAcceptance,
    };
    let bytes = encode_component_description_checked(&description).unwrap();
    let proposed = verify_component(&bytes, &request).unwrap();
    let rows = proposed.assumption_rows().unwrap();
    assert_eq!(rows[&assumption].len(), 1);
    assert!(rows[&assumption][0].contains("device shutdown"));
    let accepted = verify_component(
        &bytes,
        &ComponentVerificationRequest {
            accepted_assumptions: AcceptedComponentAssumptions::Exact(BTreeSet::from([assumption])),
            ..request.clone()
        },
    )
    .unwrap();
    assert_eq!(accepted.assumption_rows().unwrap(), rows);

    let stack = InstallationObligation {
        kind: ObligationKind::StackProvision,
        identity: "stack-provision:1:32:16".into(),
        detail: "measured supplier stack".into(),
    };
    let locator = [19; 32];
    let mut native = description.clone();
    native.realization_identity = Some(locator);
    native.obligations.push(stack.clone());
    native.obligations.sort();
    let native_bytes = encode_component_description_checked(&native).unwrap();
    proposed
        .verify_native_realization(&native_bytes, locator, &stack)
        .unwrap();
    assert!(
        proposed
            .verify_native_realization(&native_bytes, [20; 32], &stack)
            .is_err()
    );
    let mut changed_stack = stack.clone();
    changed_stack.identity = "stack-provision:1:16:16".into();
    assert!(
        proposed
            .verify_native_realization(&native_bytes, locator, &changed_stack)
            .is_err()
    );
    native
        .entries
        .iter_mut()
        .find(|row| row.evidence == EntryEvidence::AssumptionBound(assumption))
        .unwrap()
        .identity
        .push_str(" changed");
    native.entries.sort();
    assert!(
        proposed
            .verify_native_realization(
                &encode_component_description_checked(&native).unwrap(),
                locator,
                &stack
            )
            .is_err()
    );

    description
        .entries
        .iter_mut()
        .find(|row| row.evidence == EntryEvidence::AssumptionBound(assumption))
        .unwrap()
        .identity
        .push_str(" v2");
    description.entries.sort();
    let changed = verify_component(
        &encode_component_description_checked(&description).unwrap(),
        &request,
    )
    .unwrap();
    assert_ne!(
        rows[&assumption],
        changed.assumption_rows().unwrap()[&assumption]
    );
}
