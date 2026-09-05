mod support;

#[path = "representation_policy/source.rs"]
mod source;

use omega_package_evidence::encoding::PackagePolicyRecoveryLimits;
use omega_package_evidence::record::{
    PackagePolicyRepresentation, PackageReviewNominalOwner,
    PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
    PackageReviewRepresentationTargetProfile, PackageReviewTypeParameterKind,
};
use omega_package_evidence::{
    project_checked_calling_policy, project_checked_representation_policy,
};
use source::Fixture;
use support::*;

fn project(
    checked: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> PackagePolicyRepresentation {
    let policy = project_checked_representation_policy(checked, package)
        .expect("complete source-derived representation policy");
    assert_eq!(policy.package(), package);
    assert_eq!(
        policy.target().profile(),
        PackageReviewRepresentationTargetProfile::WindowsX64
    );
    let bytes = policy
        .canonical_bytes()
        .expect("canonical representation policy");
    let recovered = PackagePolicyRepresentation::recover_canonical(
        &bytes,
        PackagePolicyRecoveryLimits::default(),
    )
    .expect("bounded source-independent representation policy recovery");
    assert_eq!(recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    policy
}

#[test]
fn producer_availability_does_not_fabricate_selection_or_demand() {
    let fixture = Fixture::new(false, false, false);
    let unknown = PackageKeyIdentity::from_digest([99; 32]).unwrap();
    assert!(project_checked_representation_policy(&fixture.checked, unknown).is_err());
    assert!(
        fixture
            .checked
            .opaque_representation_selections()
            .is_empty()
    );
    let policy = project(&fixture.checked, package_identity());
    assert_eq!(policy.declarations().len(), 2);
    assert_eq!(policy.producer_availability().len(), 2);
    assert!(policy.selected_availability().is_empty());
    assert!(policy.demands().is_empty());
    for candidate in policy.producer_availability() {
        assert!(policy.declarations().contains(candidate.opaque()));
        assert_eq!(
            candidate.opaque().owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert_eq!(candidate.carrier().owner(), candidate.opaque().owner());
        assert_eq!(
            candidate.conformance().identity().owner(),
            candidate.opaque().owner()
        );
        assert!(candidate.conformance().type_parameters().is_empty());
    }
}

#[test]
fn unused_placement_and_semantic_copy_selections_both_remain_visible() {
    let fixture = Fixture::new(true, false, false);
    assert_eq!(fixture.checked.opaque_representation_selections().len(), 2);
    assert!(
        fixture
            .checked
            .boundary_calling_plan_realizations()
            .is_empty()
    );
    let policy = project(&fixture.checked, package_identity());
    assert_eq!(policy.selected_availability().len(), 2);
    assert!(
        policy.demands().is_empty(),
        "selection alone creates no crossing"
    );
    for (opaque, carrier, disposition) in [
        (
            "Token",
            "Carrier",
            PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly,
        ),
        (
            "CopyToken",
            "CopyCarrier",
            PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
        ),
    ] {
        let selected = policy
            .selected_availability()
            .iter()
            .find(|selected| selected.opaque().path() == opaque)
            .unwrap();
        assert_eq!(selected.carrier().path(), carrier);
        assert_eq!(selected.copy_disposition(), disposition);
        assert_eq!(
            selected.origin(),
            PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance
        );
        assert_eq!(
            selected.lifecycle(),
            PackageReviewOpaqueRepresentationLifecycleDisposition::Inert
        );
        assert_eq!(
            selected.selection_owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert!(selected.application().subject().is_some());
        assert_eq!(selected.application().trait_arguments().len(), 1);
        assert_eq!(
            selected.application().declaration().path(),
            format!("{opaque}Representation")
        );
    }
    assert!(
        project_checked_representation_policy(&fixture.changed_carrier(), package_identity())
            .is_err()
    );

    let moved = Fixture::moved_selection();
    let moved_policy = project(&moved.checked, package_identity());
    assert_eq!(
        policy, moved_policy,
        "selecting source position is custody, not policy identity"
    );
    assert_eq!(
        policy.canonical_bytes().unwrap(),
        moved_policy.canonical_bytes().unwrap()
    );
}

#[test]
fn used_selection_retains_the_complete_calling_application() {
    let fixture = Fixture::new(true, true, false);
    let policy = project(&fixture.checked, package_identity());
    assert_eq!(policy.selected_availability().len(), 2);
    let [demand] = policy.demands() else {
        panic!("one actual by-value opaque crossing")
    };
    assert_eq!(demand.opaque().path(), "Token");
    let realization = fixture
        .checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| {
            fixture.checked.symbols.name(realization.boundary_trait) == "TransferEntry"
        })
        .unwrap();
    let calling = project_checked_calling_policy(&fixture.checked, realization).unwrap();
    assert_eq!(demand.calling(), &calling);
    assert_eq!(calling.semantic_parameters().len(), 1);
    assert!(calling.semantic_result().is_none());
    let [use_] = calling.opaque_uses() else {
        panic!("only Token has an actual occurrence")
    };
    assert_eq!(use_.opaque(), demand.opaque());
    assert_eq!(use_.occurrences().len(), 1);
    assert!(use_.occurrences()[0].path().is_empty());
    assert!(
        project_checked_representation_policy(&fixture.changed_carrier(), package_identity())
            .is_err()
    );
    assert!(
        project_checked_representation_policy(&fixture.changed_use_type(), package_identity())
            .is_err()
    );
}

#[test]
fn foreign_producer_availability_and_local_consumer_selection_keep_distinct_owners() {
    let fixture = Fixture::new(true, true, true);
    let consumer = project(&fixture.checked, package_identity());
    let producer = project(&fixture.checked, source::foreign_identity());
    let foreign = PackageReviewNominalOwner::Package(source::foreign_identity());
    assert!(consumer.declarations().is_empty());
    assert!(consumer.producer_availability().is_empty());
    assert_eq!(consumer.selected_availability().len(), 2);
    assert_eq!(consumer.demands().len(), 1);
    assert_eq!(producer.declarations().len(), 2);
    assert_eq!(producer.producer_availability().len(), 2);
    assert!(producer.selected_availability().is_empty());
    assert!(producer.demands().is_empty());
    for selected in consumer.selected_availability() {
        assert_eq!(selected.opaque().owner(), foreign);
        assert_eq!(selected.carrier().owner(), foreign);
        assert_eq!(selected.application().declaration().owner(), foreign);
        assert_eq!(
            selected.selection_owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert!(
            producer
                .producer_availability()
                .iter()
                .any(|candidate| candidate.opaque() == selected.opaque()
                    && candidate.carrier() == selected.carrier()
                    && candidate.conformance().identity() == selected.application().declaration())
        );
    }
    let use_ = &consumer.demands()[0].calling().opaque_uses()[0];
    assert_eq!(use_.opaque().owner(), foreign);
    assert_eq!(
        use_.selection_owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn producer_availability_retains_its_unused_public_generic_telescope() {
    let fixture = Fixture::generic_availability();
    let policy = project(&fixture.checked, package_identity());
    let available = policy
        .producer_availability()
        .iter()
        .find(|candidate| candidate.opaque().path() == "Token")
        .unwrap();
    assert_eq!(available.conformance().lifetime_parameter_count(), 1);
    let parameters = available.conformance().type_parameters();
    assert_eq!(parameters.len(), 2);
    assert!(matches!(
        parameters[0].kind(),
        PackageReviewTypeParameterKind::Type
    ));
    assert!(matches!(
        parameters[1].kind(),
        PackageReviewTypeParameterKind::Const(_)
    ));
    assert!(policy.selected_availability().is_empty());
    assert!(policy.demands().is_empty());
}
