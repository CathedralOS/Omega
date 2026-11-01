use super::*;
use language_semantics::opaque_representations::{
    OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
    OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
    SelectedOpaqueApplication, selected_application_commitment,
};

fn application() -> SelectedOpaqueApplication {
    let conformance = [7; 32];
    SelectedOpaqueApplication {
        opaque_identity: "Opaque".into(),
        carrier_identity: "Bytes".into(),
        conformance_identity: "Representation".into(),
        application_report_fingerprint: 7,
        conformance_application_commitment: conformance,
        identity_encoding: vec![1, 2, 3],
        schema_version: OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION,
        origin: OpaqueRepresentationApplicationOrigin::NamedConformance,
        lifecycle: OpaqueRepresentationLifecycleDisposition::Inert,
        copy_disposition: OpaqueRepresentationCopyDisposition::PlacementOnly,
        selected_application_commitment: selected_application_commitment(
            conformance,
            OpaqueRepresentationLifecycleDisposition::Inert,
            OpaqueRepresentationCopyDisposition::PlacementOnly,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
        ),
    }
}

fn review(rows: Vec<Result<SelectedOpaqueApplication, String>>) -> PreparedCallingReview {
    PreparedCallingReview {
        requirement_identity: "Boundary::call".into(),
        callback_binders: vec![],
        direct_callbacks: vec![],
        opaque_selections: rows
            .into_iter()
            .map(|application| PreparedOpaqueReviewSelection {
                application,
                opaque: symbols::SymbolHandle::invalid(),
                carrier: symbols::SymbolHandle::invalid(),
                conformance: symbols::SymbolHandle::invalid(),
            })
            .collect::<Vec<_>>()
            .into(),
    }
}

#[test]
fn opaque_review_consumes_deferred_results_in_original_order() {
    let expected = application();
    let refused = review(vec![
        Err("first original refusal".into()),
        Ok(expected.clone()),
    ]);
    assert_eq!(
        refused.opaque_review(&expected).err().as_deref(),
        Some("first original refusal")
    );
    let refused = review(vec![
        Ok(expected.clone()),
        Ok(expected.clone()),
        Err("later refusal".into()),
    ]);
    assert_eq!(
        refused.opaque_review(&expected).err().as_deref(),
        Some("canonical opaque occurrence has competing source selections")
    );
}

#[test]
fn opaque_review_joins_the_complete_application_not_its_report() {
    let expected = application();
    let mut other = expected.clone();
    other.identity_encoding.push(4);
    let prepared = review(vec![Ok(other.clone()), Ok(expected.clone())]);
    let selected = prepared
        .opaque_review(&expected)
        .expect("one exact complete application");
    assert_eq!(selected.application.as_ref().unwrap(), &expected);
    assert_eq!(
        prepared
            .opaque_review(&other)
            .unwrap()
            .application
            .as_ref()
            .unwrap(),
        &other
    );
    let mut missing = expected;
    missing.carrier_identity = "OtherBytes".into();
    assert!(prepared.opaque_review(&missing).is_err());
}
