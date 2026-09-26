//! Representation policy agreement tests.

use super::{PackageKeyIdentity, PackageSourceConsumptionCommitment};
use crate::record::PackagePolicyRepresentationAgreementError;
use crate::record::PackagePolicyRepresentationProducerInstance;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewNominalOwner;
use crate::record::{
    PackagePolicyCallbacks, PackagePolicyCallingOpaqueUse, PackagePolicyCallingPlan,
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceShape,
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract, PackagePolicyPreemption,
    PackagePolicyRepresentation, PackagePolicyRepresentationAvailability,
    PackagePolicyRepresentationDemand, PackagePolicyRepresentationSelection,
    PackagePolicyStatePlan, PackageReviewBoundaryCallingPolicy, PackageReviewEvidenceInterface,
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
};

const SOURCE_INSTANCE: PackageSourceConsumptionCommitment =
    PackageSourceConsumptionCommitment::for_test([5; 32]);

fn package(byte: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([byte; 32]).unwrap()
}

fn nominal(owner: PackageReviewNominalOwner, path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner,
        path: path.to_owned(),
    }
}

fn producer_instance<'a>(
    policy: &'a PackagePolicyRepresentation,
    expected_source_instance: PackageSourceConsumptionCommitment,
    reviewed_source_instance: PackageSourceConsumptionCommitment,
) -> PackagePolicyRepresentationProducerInstance<'a> {
    PackagePolicyRepresentationProducerInstance {
        policy,
        expected_source_instance,
        reviewed_source_instance,
    }
}

fn target(profile: PackageReviewRepresentationTargetProfile) -> PackageReviewRepresentationTarget {
    PackageReviewRepresentationTarget {
        profile,
        architecture: PackageReviewRepresentationArchitecture::X86_64,
        object_format: PackageReviewRepresentationObjectFormat::Coff,
        pointer_size: 8,
        pointer_alignment: 8,
    }
}

fn opaque_use(
    opaque: PackageReviewNominalIdentity,
    conformance: PackageReviewNominalIdentity,
    carrier: PackageReviewNominalIdentity,
) -> PackagePolicyCallingOpaqueUse {
    PackagePolicyCallingOpaqueUse {
        opaque,
        carrier,
        selection_owner: PackageReviewNominalOwner::Package(package(1)),
        application: PackagePolicyClosedConformanceApplication {
            declaration: conformance,
            lifetime_arguments: Vec::new(),
            type_arguments: Vec::new(),
            const_arguments: Vec::new(),
            machine_arguments: Vec::new(),
            subject: None,
            trait_identity: nominal(
                PackageReviewNominalOwner::ToolchainSource(
                    crate::record::PackageReviewToolchainSourceIdentity { digest: [7; 32] },
                ),
                "OpaqueRepresentation",
            ),
            trait_lifetime_arguments: Vec::new(),
            trait_arguments: Vec::new(),
            rows: Vec::new(),
        },
        origin: crate::record::PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance,
        lifecycle: crate::record::PackageReviewOpaqueRepresentationLifecycleDisposition::Inert,
        copy_disposition:
            crate::record::PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly,
        occurrences: Vec::new(),
    }
}

fn policy(
    consumer: PackageKeyIdentity,
    target: PackageReviewRepresentationTarget,
    opaque_owner: PackageReviewNominalOwner,
    conformance_owner: PackageReviewNominalOwner,
    availability: bool,
    declaration: bool,
) -> PackagePolicyRepresentation {
    let opaque = nominal(opaque_owner, "Token");
    let conformance = nominal(conformance_owner, "TokenRepresentation");
    let carrier = nominal(conformance_owner, "Carrier");
    let use_ = opaque_use(opaque.clone(), conformance.clone(), carrier.clone());
    let demand_opaque = use_.opaque().clone();
    let state = PackagePolicyStatePlan {
        initial_regime: PackagePolicyMachineRegime::X86Long64,
        interrupted_state: PackagePolicyMachineStateSet::default(),
        saved_state: PackagePolicyMachineStateSet::default(),
        restored_state: PackagePolicyMachineStateSet::default(),
        permitted_transitive_use: PackagePolicyMachineStateSet::default(),
        stack: PackagePolicyEntryStack::Dedicated { class: 0 },
        preemption: PackagePolicyPreemption::NotApplicable,
    };
    let calling = PackagePolicyCallingPlan {
        boundary_trait: nominal(
            PackageReviewNominalOwner::ToolchainSource(
                crate::record::PackageReviewToolchainSourceIdentity { digest: [8; 32] },
            ),
            "Calling",
        ),
        boundary_arguments: Vec::new(),
        boundary_lifetime_parameter_count: 0,
        requirement: nominal(
            PackageReviewNominalOwner::ToolchainSource(
                crate::record::PackageReviewToolchainSourceIdentity { digest: [9; 32] },
            ),
            "Calling::call",
        ),
        requirement_trait: nominal(
            PackageReviewNominalOwner::ToolchainSource(
                crate::record::PackageReviewToolchainSourceIdentity { digest: [8; 32] },
            ),
            "Calling",
        ),
        requirement_arguments: Vec::new(),
        requirement_lifetime_arguments: Vec::new(),
        requirement_lifetime_parameter_count: 0,
        static_parameters: Vec::new(),
        target,
        shape_graph: crate::record::PackageReviewBoundaryShapeGraph {
            shapes: Vec::new(),
            fields: Vec::new(),
            parameters: Vec::new(),
            result: None,
        },
        semantic_parameters: Vec::new(),
        semantic_result: None,
        native_parameters: Vec::new(),
        callbacks: PackagePolicyCallbacks {
            binders: Vec::new(),
            demands: Vec::new(),
            materializations: Vec::new(),
            layouts: Vec::new(),
        },
        opaque_uses: vec![use_],
        physical: PackagePolicyPhysicalCallingContract {
            policy: PackageReviewBoundaryCallingPolicy::MicrosoftX64,
            parameters: Vec::new(),
            result: None,
            ordinary_clobbers: Vec::new(),
            stack_alignment: 16,
            shadow_bytes: 32,
            entry_control: PackagePolicyEntryControl::CallReturn,
            state,
        },
    };
    PackagePolicyRepresentation {
        package: consumer,
        target,
        declarations: declaration.then_some(opaque.clone()).into_iter().collect(),
        producer_availability: availability
            .then_some(PackagePolicyRepresentationAvailability {
                opaque,
                conformance: PackagePolicyConformanceShape {
                    identity: conformance,
                    lifetime_parameter_count: 0,
                    type_parameters: Vec::new(),
                    subject: crate::record::PackageReviewConformanceSubject::Nominal(
                        carrier.clone(),
                    ),
                    interface: PackageReviewEvidenceInterface {
                        trait_identity: nominal(
                            PackageReviewNominalOwner::ToolchainSource(
                                crate::record::PackageReviewToolchainSourceIdentity {
                                    digest: [7; 32],
                                },
                            ),
                            "OpaqueRepresentation",
                        ),
                        lifetime_arguments: Vec::new(),
                        arguments: Vec::new(),
                        requirements: Vec::new(),
                    },
                },
                carrier,
            })
            .into_iter()
            .collect(),
        selected_availability: Vec::new(),
        demands: vec![PackagePolicyRepresentationDemand {
            opaque: demand_opaque,
            calling,
        }],
    }
}

#[test]
fn joins_exact_foreign_declaration_and_availability() {
    let consumer_id = package(1);
    let producer_id = package(2);
    let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
    let consumer = policy(
        consumer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    let producer = policy(
        producer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        true,
        true,
    );
    assert!(
        consumer
            .rejoin_foreign_demands(|identity| (identity == producer_id)
                .then(|| producer_instance(&producer, SOURCE_INSTANCE, SOURCE_INSTANCE)))
            .is_ok()
    );
}

#[test]
fn rejects_absent_producer_policy() {
    let producer_id = package(2);
    let consumer = policy(
        package(1),
        target(PackageReviewRepresentationTargetProfile::WindowsX64),
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    assert!(matches!(
        consumer.rejoin_foreign_demands(|_| None),
        Err(PackagePolicyRepresentationAgreementError::ProducerPolicyAbsent { .. })
    ));
}

#[test]
fn rejects_undeclared_opaque() {
    let producer_id = package(2);
    let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
    let consumer = policy(
        package(1),
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    let producer = policy(
        producer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    assert!(matches!(
        consumer.rejoin_foreign_demands(|_| Some(producer_instance(
            &producer,
            SOURCE_INSTANCE,
            SOURCE_INSTANCE
        ))),
        Err(PackagePolicyRepresentationAgreementError::OpaqueUndeclared { .. })
    ));
}

#[test]
fn rejects_unpublished_availability() {
    let producer_id = package(2);
    let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
    let consumer = policy(
        package(1),
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    let producer = policy(
        producer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        true,
    );
    assert!(matches!(
        consumer.rejoin_foreign_demands(|_| Some(producer_instance(
            &producer,
            SOURCE_INSTANCE,
            SOURCE_INSTANCE
        ))),
        Err(PackagePolicyRepresentationAgreementError::AvailabilityUnpublished { .. })
    ));
}

#[test]
fn rejects_target_mismatch() {
    let producer_id = package(2);
    let consumer = policy(
        package(1),
        target(PackageReviewRepresentationTargetProfile::WindowsX64),
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    let producer = policy(
        producer_id,
        target(PackageReviewRepresentationTargetProfile::LinuxX64),
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        true,
        true,
    );
    assert!(matches!(
        consumer.rejoin_foreign_demands(|_| Some(producer_instance(
            &producer,
            SOURCE_INSTANCE,
            SOURCE_INSTANCE
        ))),
        Err(PackagePolicyRepresentationAgreementError::TargetMismatch { .. })
    ));
}

#[test]
fn local_owner_needs_no_producer_policy() {
    let consumer_id = package(1);
    let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
    let consumer = policy(
        consumer_id,
        target,
        PackageReviewNominalOwner::Package(consumer_id),
        PackageReviewNominalOwner::Package(consumer_id),
        false,
        true,
    );
    assert!(consumer.rejoin_foreign_demands(|_| None).is_ok());
}

#[test]
fn rejects_resolved_policy_owned_by_another_package() {
    let producer_id = package(2);
    let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
    let consumer = policy(
        package(1),
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    let other = policy(
        package(3),
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        true,
        true,
    );
    assert!(matches!(
        consumer.rejoin_foreign_demands(|_| Some(producer_instance(
            &other,
            SOURCE_INSTANCE,
            SOURCE_INSTANCE
        ))),
        Err(PackagePolicyRepresentationAgreementError::ProducerPolicyAbsent { .. })
    ));
}

#[test]
fn rejects_producer_source_instance_mismatch() {
    let producer_id = package(2);
    let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
    let consumer = policy(
        package(1),
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    let producer = policy(
        producer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        true,
        true,
    );
    assert!(matches!(
        consumer.rejoin_foreign_demands(|_| Some(producer_instance(
            &producer,
            SOURCE_INSTANCE,
            PackageSourceConsumptionCommitment::for_test([9; 32])
        ))),
        Err(PackagePolicyRepresentationAgreementError::ProducerSourceInstanceMismatch { .. })
    ));
}

#[test]
fn accepts_producer_selection_equal_to_the_actual_application() {
    let consumer_id = package(1);
    let producer_id = package(2);
    let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
    let consumer = policy(
        consumer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    let mut producer = policy(
        producer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        true,
        true,
    );
    let use_ = &consumer.demands()[0].calling().opaque_uses()[0];
    producer
        .selected_availability
        .push(PackagePolicyRepresentationSelection {
            opaque: use_.opaque().clone(),
            carrier: use_.carrier().clone(),
            selection_owner: PackageReviewNominalOwner::Package(producer_id),
            application: use_.application().clone(),
            origin: use_.origin(),
            lifecycle: use_.lifecycle(),
            copy_disposition: use_.copy_disposition(),
        });
    assert!(
        consumer
            .rejoin_foreign_demands(|_| Some(producer_instance(
                &producer,
                SOURCE_INSTANCE,
                SOURCE_INSTANCE
            )))
            .is_ok(),
        "an equal producer selection with a different selection owner agrees"
    );
}

#[test]
fn rejects_producer_selection_mismatch_at_the_exchange() {
    let consumer_id = package(1);
    let producer_id = package(2);
    let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
    let consumer = policy(
        consumer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        false,
        false,
    );
    let mut producer = policy(
        producer_id,
        target,
        PackageReviewNominalOwner::Package(producer_id),
        PackageReviewNominalOwner::Package(producer_id),
        true,
        true,
    );
    let use_ = &consumer.demands()[0].calling().opaque_uses()[0];
    let mut application = use_.application().clone();
    application.type_arguments = Vec::new();
    application
        .type_arguments
        .push(crate::record::PackageReviewTypeIdentity {
            canonical: "Different".to_owned(),
        });
    producer
        .selected_availability
        .push(PackagePolicyRepresentationSelection {
            opaque: use_.opaque().clone(),
            carrier: use_.carrier().clone(),
            selection_owner: PackageReviewNominalOwner::Package(producer_id),
            application,
            origin: use_.origin(),
            lifecycle: use_.lifecycle(),
            copy_disposition: use_.copy_disposition(),
        });
    assert!(matches!(
        consumer.rejoin_foreign_demands(|_| Some(producer_instance(
            &producer,
            SOURCE_INSTANCE,
            SOURCE_INSTANCE
        ))),
        Err(PackagePolicyRepresentationAgreementError::SelectedApplicationMismatch { .. })
    ));

    let mut disposition = producer.clone();
    disposition.selected_availability.clear();
    disposition.selected_availability.push(
        PackagePolicyRepresentationSelection {
            opaque: use_.opaque().clone(),
            carrier: use_.carrier().clone(),
            selection_owner: PackageReviewNominalOwner::Package(producer_id),
            application: use_.application().clone(),
            origin: use_.origin(),
            lifecycle: use_.lifecycle(),
            copy_disposition:
                crate::record::PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
        },
    );
    assert!(matches!(
        consumer.rejoin_foreign_demands(|_| Some(producer_instance(
            &disposition,
            SOURCE_INSTANCE,
            SOURCE_INSTANCE
        ))),
        Err(PackagePolicyRepresentationAgreementError::SelectedApplicationMismatch { .. })
    ));
}
