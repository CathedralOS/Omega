//! Program-local module, catalog, claim, lifecycle, epoch and subject
//! fixtures.

use super::TestObject;
use super::installed_code_fixtures::{extent_id, extent_provider_issuance};
use crate::{
    ExternalRootEntryClaim, InstalledExternalRoot, InstalledProgramLocalRootOccurrence,
    InstalledProgramLocalRootSubject, ProgramLocalEntryActivation, ProgramLocalRootCohortMember,
    ProgramLocalRootCohortSealError, ProgramLocalRootInstallationLedger,
    ProgramLocalRootPrebindingId, ProgramLocalRootScalarBinding, ProgramLocalRootSubjectPlaceId,
};
use effects::{
    ComponentEraCandidate, ComponentEraEntryLedger, ComponentEraEntryReceipt, ComponentEraLedgerId,
    ComponentEraPublicationReceipt, ExecutableTcbManifest, ExecutableTcbProfile,
    ExecutableTcbProfileAcceptance, ExecutionScope, IncompleteScopePolicy,
    ProgramLocalRootEpochLeaseId, ScopeCompleteness, evaluate_executable_tcb_profile,
};
use extents::{
    AddressSpaceId, Extent, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId,
};
use proof_admission::AdmissionProfile;
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, TerminalRootServiceReach, VocabularyMarker,
    program_local_root_introduction_compatibility_report_identity,
};

pub(super) fn program_local_root_module() -> TerminalModule {
    let entry = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let carrier = semantic_vocabulary::StructuralTypeId::new(1).expect("carrier identity");
    let qualification = semantic_vocabulary::StructuralDomainId::new(1).expect("domain identity");
    let algebra = semantic_vocabulary::ContentAlgebra {
        kind: semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
        parameter: "ByteUnit".into(),
    };
    let capacity = semantic_vocabulary::ContentProjectionExpression::CountedQuantity(
        semantic_vocabulary::ContentProjectionScalar::Add(
            Box::new(semantic_vocabulary::ContentProjectionScalar::SubjectField(
                vec!["length".into()],
            )),
            Box::new(semantic_vocabulary::ContentProjectionScalar::Natural(
                "1".into(),
            )),
        ),
    );
    let mut schema = terminal_psi::ProgramLocalRootIntroductionSchema {
        argument_index: 0,
        source_parameter_position: 0,
        qualification,
        carrier,
        projection: semantic_vocabulary::ContentProjectionIdentity {
            domain: semantic_vocabulary::ContentDomainId::new(1).expect("content domain identity"),
            projection_report_fingerprint:
                language_semantics::content::terminal_projection_report_fingerprint(
                    &algebra, &capacity,
                ),
        },
        algebra,
        capacity,
        compatibility_report_identity: 0,
    };
    schema.compatibility_report_identity =
        program_local_root_introduction_compatibility_report_identity(
            "TestRoot::entry",
            "Region::Owned",
            "Region",
            &schema,
        );
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry,
        structural_types: vec![StructuralTypeDeclaration {
            id: carrier,
            identity: "Region".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
                    identity: "length".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                64,
                            )
                            .expect("u64 type"),
                        ),
                    ),
                }],
            },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: qualification,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("semantic domain identity"),
            identity: "Region::Owned".into(),
            carrier,
            content_projection: Some(StructuralContentProjection {
                identity: schema.projection,
                algebra: schema.algebra.clone(),
                expression: schema.capacity.clone(),
            }),
        }],
        services: Vec::new(),
        root_service_reach: TerminalRootServiceReach::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: semantic_vocabulary::BoundaryMachineId::new(1).expect("boundary identity"),
            identity: "TestRoot::entry".into(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: semantic_vocabulary::PlaceId::new(1).expect("place identity"),
                position: 0,
                is_self: false,
                structural_type: carrier,
                multiplicity: StructuralMultiplicity::Linear,
                access: terminal_psi::StructuralAccess::Owned,
                qualifications: vec![qualification],
                projected_qualifications: Vec::new(),
            }],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain: qualification,
            }],
            program_local_root_introductions: vec![schema],
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![terminal_psi::TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: entry,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: terminal_psi::TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: semantic_vocabulary::BlockId::new(1).expect("block identity"),
            blocks: vec![terminal_psi::Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: semantic_vocabulary::BlockId::new(1).expect("block identity"),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: terminal_psi::Terminator::ReturnUnit {
                    edge: semantic_vocabulary::EdgeId::new(1).expect("edge identity"),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: terminal_psi::MachineContract {
                erased_scalar_formals: Vec::new(),
                id: semantic_vocabulary::ContractId::new(1).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

pub(super) fn program_local_two_schema_module() -> TerminalModule {
    let mut module = program_local_root_module();
    let machine = &mut module.boundary_machines[0];
    machine
        .parameter_order
        .push(terminal_psi::BoundaryParameterKind::Structural);
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: semantic_vocabulary::PlaceId::new(2).expect("place identity"),
            position: 1,
            is_self: false,
            structural_type: machine.structural_parameters[0].structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: machine.structural_parameters[0].qualifications.clone(),
            projected_qualifications: Vec::new(),
        });
    machine.requires.push(StructuralDomainRequirement {
        argument_index: 1,
        domain: machine.structural_parameters[0].qualifications[0],
    });
    let mut second = machine.program_local_root_introductions[0].clone();
    second.argument_index = 1;
    second.source_parameter_position = 1;
    second.compatibility_report_identity =
        program_local_root_introduction_compatibility_report_identity(
            "TestRoot::entry",
            "Region::Owned",
            "Region",
            &second,
        );
    machine.program_local_root_introductions.push(second);
    module
}

/// The two-schema module with the interval-set Extent algebra on both
/// introductions: one epoch cohort carries two single-member aggregate
/// groups, which is what cross-group membership rejections need.
pub(super) fn program_local_two_schema_extent_module() -> TerminalModule {
    let mut module = program_local_two_schema_module();
    let algebra = semantic_vocabulary::ContentAlgebra {
        kind: semantic_vocabulary::ContentAlgebraKind::IntervalSet,
        parameter: "Nat".into(),
    };
    let capacity = semantic_vocabulary::ContentProjectionExpression::IntervalSet(vec![(
        semantic_vocabulary::ContentProjectionScalar::SubjectField(vec!["base".into()]),
        semantic_vocabulary::ContentProjectionScalar::Add(
            Box::new(semantic_vocabulary::ContentProjectionScalar::SubjectField(
                vec!["base".into()],
            )),
            Box::new(semantic_vocabulary::ContentProjectionScalar::SubjectField(
                vec!["length".into()],
            )),
        ),
    )]);
    let fingerprint =
        language_semantics::content::terminal_projection_report_fingerprint(&algebra, &capacity);
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut module.structural_types[0].shape
    else {
        panic!("program-local test carrier is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(2).expect("base field identity"),
        identity: "base".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 type"),
        )),
    });
    for schema in &mut module.boundary_machines[0].program_local_root_introductions {
        schema.algebra = algebra.clone();
        schema.capacity = capacity.clone();
        schema.projection.projection_report_fingerprint = fingerprint;
        schema.compatibility_report_identity =
            program_local_root_introduction_compatibility_report_identity(
                "TestRoot::entry",
                "Region::Owned",
                "Region",
                schema,
            );
    }
    module.structural_domains[0].content_projection = Some(StructuralContentProjection {
        identity: module.boundary_machines[0].program_local_root_introductions[0].projection,
        algebra,
        expression: capacity,
    });
    module
}

pub(super) fn program_local_extent_module() -> TerminalModule {
    let mut module = program_local_root_module();
    let algebra = semantic_vocabulary::ContentAlgebra {
        kind: semantic_vocabulary::ContentAlgebraKind::IntervalSet,
        parameter: "Nat".into(),
    };
    let capacity = semantic_vocabulary::ContentProjectionExpression::IntervalSet(vec![(
        semantic_vocabulary::ContentProjectionScalar::SubjectField(vec!["base".into()]),
        semantic_vocabulary::ContentProjectionScalar::Add(
            Box::new(semantic_vocabulary::ContentProjectionScalar::SubjectField(
                vec!["base".into()],
            )),
            Box::new(semantic_vocabulary::ContentProjectionScalar::SubjectField(
                vec!["length".into()],
            )),
        ),
    )]);
    let schema = &mut module.boundary_machines[0].program_local_root_introductions[0];
    schema.algebra = algebra;
    schema.capacity = capacity;
    schema.projection.projection_report_fingerprint =
        language_semantics::content::terminal_projection_report_fingerprint(
            &schema.algebra,
            &schema.capacity,
        );
    schema.compatibility_report_identity =
        program_local_root_introduction_compatibility_report_identity(
            "TestRoot::entry",
            "Region::Owned",
            "Region",
            schema,
        );
    module.structural_domains[0].content_projection = Some(StructuralContentProjection {
        identity: schema.projection,
        algebra: schema.algebra.clone(),
        expression: schema.capacity.clone(),
    });
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut module.structural_types[0].shape
    else {
        panic!("program-local test carrier is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(2).expect("base field identity"),
        identity: "base".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 type"),
        )),
    });
    module
}

pub(super) fn program_local_terminal_object(module: &TerminalModule) -> TestObject {
    TestObject {
        identity: terminal_codec::terminal_psi_identity(module).expect("terminal program identity"),
        entry: module.entry,
        bytes: vec![0; 64],
    }
}

pub(super) fn program_local_root_catalog(
    module: &TerminalModule,
) -> terminal_codec::VerifiedProgramLocalRootProducerCatalog {
    let proof = terminal_verifier::ProofBundle::default();
    let verified = terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default())
        .expect("program-local terminal module verifies");
    terminal_codec::VerifiedProgramLocalRootProducerCatalog::from_verified(&verified)
        .expect("verified program-local producer catalog")
}

pub(super) fn program_local_claim() -> ExternalRootEntryClaim {
    program_local_claim_at(0)
}

pub(super) fn program_local_claim_at(parameter_index: usize) -> ExternalRootEntryClaim {
    ExternalRootEntryClaim {
        parameter_index,
        domain: "Region::Owned".into(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
    }
}

pub(super) fn program_local_tcb_acceptance(seed: u64) -> ExecutableTcbProfileAcceptance {
    evaluate_executable_tcb_profile(
        &ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::CallerAddressSpace,
                selected_provider_closure_report_identity: seed,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        },
        &ExecutableTcbProfile {
            name: format!("program-local-era-{seed}"),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        },
    )
    .expect("component-era TCB profile acceptance")
}

pub(super) fn program_local_lifecycle(
    ledger_identity: u64,
    era_identity: u64,
    artifact_occurrence_digest: installation_evidence::InstalledArtifactOccurrenceDigest,
    artifact_instance_compatibility_report_identity: u64,
    entry_contract_identity: &str,
) -> ComponentEraEntryLedger {
    let mut ledger = ComponentEraEntryLedger::new(
        ComponentEraLedgerId::from_normalized_identity(ledger_identity)
            .expect("component-era ledger identity"),
        "TestRootBinding/v1".into(),
        entry_contract_identity.into(),
        2,
        program_local_tcb_acceptance(ledger_identity),
    )
    .expect("component-era ledger");
    publish_program_local_era(
        &mut ledger,
        era_identity,
        artifact_occurrence_digest,
        artifact_instance_compatibility_report_identity,
        entry_contract_identity,
        era_identity + 100,
        false,
    );
    ledger
}

pub(super) fn publish_program_local_era(
    ledger: &mut ComponentEraEntryLedger,
    era_identity: u64,
    artifact_occurrence_digest: installation_evidence::InstalledArtifactOccurrenceDigest,
    artifact_instance_compatibility_report_identity: u64,
    entry_contract_identity: &str,
    publication_identity: u64,
    previous_era_closed: bool,
) {
    let candidate = ComponentEraCandidate {
        era_identity,
        artifact_occurrence_digest,
        artifact_instance_compatibility_report_identity,
        binding_contract_identity: "TestRootBinding/v1".into(),
        entry_contract_identity: entry_contract_identity.into(),
        entry_plan_identity: format!("entry-plan:{era_identity}"),
        entry_plan_admission_receipt_identity: format!("entry-plan-receipt:{era_identity}"),
        executable_tcb_acceptance: program_local_tcb_acceptance(era_identity),
    };
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        publication_identity,
        ledger,
        &candidate,
        true,
        previous_era_closed,
    );
    ledger
        .publish(candidate, receipt)
        .expect("publish component era");
}

pub(super) fn program_local_epoch_lease(
    ledger: &mut ComponentEraEntryLedger,
    lease_identity: u64,
    era_identity: u64,
    entry_contract_identity: &str,
) -> effects::ProgramLocalRootEpochLease {
    ledger
        .acquire_program_local_root_epoch_lease(
            ProgramLocalRootEpochLeaseId::from_normalized_identity(lease_identity)
                .expect("program-local epoch lease identity"),
            era_identity,
            entry_contract_identity,
        )
        .expect("program-local epoch lease")
}

/// Enter one real activation of the generated installed-entry bridge on the
/// test lifecycle. The receipt linearizes into the era published with the
/// matching `publish_program_local_era` plan identity.
pub(super) fn program_local_activation(
    lifecycle: &mut ComponentEraEntryLedger,
    invocation: u64,
    era_identity: u64,
) -> ProgramLocalEntryActivation {
    ProgramLocalEntryActivation::enter(
        lifecycle,
        ComponentEraEntryReceipt::from_runtime(
            invocation,
            lifecycle,
            era_identity,
            format!("entry-plan:{era_identity}"),
            true,
        ),
    )
    .expect("the generated entry activation enters the current open era")
}

pub(super) fn program_local_subject<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    activation: &ProgramLocalEntryActivation,
    subject_place: u64,
    length: Option<u64>,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    program_local_subject_at(root, activation, subject_place, 0, 0, length)
}

pub(super) fn program_local_subject_at<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    activation: &ProgramLocalEntryActivation,
    subject_place: u64,
    argument_index: u32,
    source_parameter_position: u32,
    length: Option<u64>,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    let scalars = length
        .into_iter()
        .map(|length| {
            ProgramLocalRootScalarBinding::subject_field(
                ["length"],
                numerics::bignum::BigInt::from_u64(length),
            )
            .expect("natural subject field")
        })
        .collect::<Vec<_>>();
    InstalledProgramLocalRootSubject::from_generated_entry(
        root,
        activation,
        argument_index,
        source_parameter_position,
        "Region::Owned",
        "Region",
        ProgramLocalRootSubjectPlaceId::from_normalized_identity(subject_place)
            .expect("subject place identity"),
        scalars,
    )
    .expect("exact installed program-local subject")
}

pub(super) fn program_local_extent_subject<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    activation: &ProgramLocalEntryActivation,
    subject_place: u64,
    base: u64,
    length: u64,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    program_local_extent_subject_at(root, activation, subject_place, 0, 0, base, length)
}

/// The interval-set Extent subject at an exact argument index and source
/// parameter position: one introduction schema's member subject.
pub(super) fn program_local_extent_subject_at<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    activation: &ProgramLocalEntryActivation,
    subject_place: u64,
    argument_index: u32,
    source_parameter_position: u32,
    base: u64,
    length: u64,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    InstalledProgramLocalRootSubject::from_generated_entry(
        root,
        activation,
        argument_index,
        source_parameter_position,
        "Region::Owned",
        "Region",
        ProgramLocalRootSubjectPlaceId::from_normalized_identity(subject_place)
            .expect("subject place identity"),
        [
            ProgramLocalRootScalarBinding::subject_field(
                ["base"],
                numerics::bignum::BigInt::from_u64(base),
            )
            .expect("natural base field"),
            ProgramLocalRootScalarBinding::subject_field(
                ["length"],
                numerics::bignum::BigInt::from_u64(length),
            )
            .expect("natural length field"),
        ],
    )
    .expect("exact installed program-local Extent subject")
}

pub(super) fn join_program_local<'root, 'code>(
    installation: &mut ProgramLocalRootInstallationLedger,
    prebinding: ProgramLocalRootPrebindingId,
    root: &'root InstalledExternalRoot<'code>,
    lifecycle: &mut ComponentEraEntryLedger,
    lease_identity: u64,
    era_identity: u64,
    entry_contract_identity: &str,
) -> Result<
    InstalledProgramLocalRootOccurrence<'root, 'code>,
    Box<ProgramLocalRootCohortSealError<'root, 'code>>,
> {
    let lease = program_local_epoch_lease(
        lifecycle,
        lease_identity,
        era_identity,
        entry_contract_identity,
    );
    let cohort = installation.seal_epoch_cohort(
        lifecycle,
        [ProgramLocalRootCohortMember::new(prebinding, root, lease)],
    )?;
    let [occurrence]: [InstalledProgramLocalRootOccurrence<'root, 'code>; 1] = cohort
        .into_runtime()
        .cancel()
        .try_into()
        .expect("single-prebinding test cohort");
    Ok(occurrence)
}

pub(super) fn sole_rejected_cohort_lease(
    error: ProgramLocalRootCohortSealError<'_, '_>,
) -> effects::ProgramLocalRootEpochLease {
    let [member]: [ProgramLocalRootCohortMember<'_, '_>; 1] = error
        .into_members()
        .try_into()
        .expect("single-member rejected cohort");
    member.into_parts().2
}

/// One actual installed backing extent: provider-issued authority over the
/// exact range a program-local root's interval capacity occupies. Materialize
/// consumes it into the held account and retirement returns it.
pub(super) fn installed_backing_extent(
    seed: u64,
    base: u64,
    length: u64,
    mapping_era: u64,
) -> Extent {
    ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(seed),
        extent_id(10 + seed, ExtentLineageId::from_normalized_identity),
        extent_id(10, AddressSpaceId::from_normalized_identity),
        ExtentRights::from_normalized_identities([
            extent_id(100, ExtentRightId::from_normalized_identity),
            extent_id(101, ExtentRightId::from_normalized_identity),
        ]),
        extent_id(20, ExtentProvenanceId::from_normalized_identity),
        extent_id(mapping_era, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .expect("actual installed backing extent")
}
