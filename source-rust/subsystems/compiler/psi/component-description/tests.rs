//! These sealed inputs were emitted by the original component publisher and
//! provider-plan writer. Neither writer is part of the receiving dependency graph.
use super::*;
use crate::records::ExportSurface;
use crate::retained_plan::ProviderPlanInput;
use std::collections::BTreeSet;
use wire::digest_encoder::content_identity;

#[test]
fn received_component_keeps_its_bytes_and_rejects_unproved_exports_and_assumptions() {
    let bytes = include_bytes!("tests/provider-free.component");
    let (mut description, artifact) = decode_component_description_with_artifact(bytes).unwrap();
    assert_eq!(
        encode_component_description_checked(&description).unwrap(),
        bytes
    );
    let request = ComponentVerificationRequest {
        expected_subject: artifact.view().identity(),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V3]),
        accepted_assumptions: AcceptedComponentAssumptions::Exact(BTreeSet::new()),
    };
    let verified = verify_component(bytes, &request).unwrap();
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
    let mut wrong_projection = bytes.to_vec();
    wrong_projection[field - 8 - 1] = 0;
    assert_eq!(
        ProviderPlanInput::read(&wrong_projection, &content_identity(&wrong_projection))
            .unwrap_err(),
        "unknown provider plan result membership projection"
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
