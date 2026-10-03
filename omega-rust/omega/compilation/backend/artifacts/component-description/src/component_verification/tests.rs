//! Component verification tests.

mod component_description_custody_fields;

use std::cell::Cell;
use std::collections::BTreeSet;

use component_description_custody_fields::{
    ComponentDescriptionCustodyCheck, ComponentDescriptionFieldForTest, classify_replay,
    component_description_custody_outcome, foreign_description_donor,
    substitute_component_description_for_test,
};
use optimization_core::{OneFieldSubstitutionMatrix, run_one_field_substitution_matrix};

use super::{
    ComponentVerificationRejection, ComponentVerificationRequest, IndependentRealizationMismatch,
    VerifiedComponent, verify_component,
};
use crate::component_description::{
    COMPONENT_DESCRIPTION_SCHEMA_V2, ComponentDescription, ComponentDescriptionFacts,
    ComponentEntry, ComponentEntryKind, CustodyEvidence, DescriptionDecodeRejection,
    DescriptionFrontier, EntryEvidence, ExportSurface, ImportSlot, InstallationObligation,
    InstallationServiceBound, ObligationKind, OutgoingAuthorityClass, OutgoingEvidence,
    canonical_entry_contract_identity, component_description_identity,
    decode_component_description, encode_component_description, requirement_contract_identity,
    requirement_export_identity,
};
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use language_semantics::CarryPolicy;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, ServiceId,
    StructuralTypeId, SuspensionCrossingId,
};
use terminal_psi::ProofBundle;
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, InstallationReachDependency,
    MachineContract, Operation, OperationKind, OperationResult, ProviderCandidateConformance,
    ProviderRefinement, ProviderSignature, ServiceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    Terminator, VocabularyMarker,
};

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine identity")
}

fn minimal_module() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        scalar_qualifications: Default::default(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).expect("block identity"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: BlockId::new(1).expect("block identity"),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).expect("edge identity"),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: ContractId::new(1).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

/// A module whose entry performs one bare `BoundaryCall` to a declared
/// Unit boundary requirement.
fn boundary_module() -> TerminalModule {
    let mut module = minimal_module();
    module.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        id: BoundaryMachineId::new(1).expect("boundary identity"),
        identity: "IndexedRequirement::apply".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).expect("operation identity"),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary: BoundaryMachineId::new(1).expect("boundary identity"),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    module
}

/// A provider component: it declares the Unit boundary requirement
/// `IndexedRequirement::apply` and retains one checked candidate machine
/// (`IndexedProvider::apply`, machine 2) realizing it, without calling it.
/// This is the closure a consumer's `Independent` selection of
/// `IndexedProvider` for that slot must join to.
fn provider_module() -> TerminalModule {
    let mut module = minimal_module();
    module.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        id: BoundaryMachineId::new(1).expect("boundary identity"),
        identity: "IndexedRequirement::apply".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    // A checked candidate is an attached machine: the Terminal verifier
    // requires its provider type to be a declared structural type.
    let provider_type = StructuralTypeId::new(1).expect("structural type identity");
    module.structural_types.push(StructuralTypeDeclaration {
        id: provider_type,
        identity: "IndexedProvider".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut candidate = module.machines[0].clone();
    candidate.id = machine_id(2);
    candidate.attachment = Some(provider_type);
    candidate.contract.id = ContractId::new(2).expect("contract identity");
    candidate.entry = BlockId::new(2).expect("block identity");
    candidate.blocks[0].id = candidate.entry;
    candidate.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(2).expect("edge identity"),
        trivial_affine_discards: Vec::new(),
    };
    module.machines.push(candidate);
    module
        .provider_candidates
        .push(ProviderCandidateConformance {
            boundary: BoundaryMachineId::new(1).expect("boundary identity"),
            requirement_identity: "IndexedRequirement::apply".into(),
            provider_identity: "IndexedProvider".into(),
            candidate_identity: "IndexedProvider::apply".into(),
            candidate: machine_id(2),
            signature: ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    module
}

/// A provider component whose root entry also calls an installation-bound
/// boundary (`reaches <= Bound`): the verified module retains the declared
/// dependency row, so its concrete root reach stays empty while the bound
/// publishes as the description's one service-bound roster entry.
fn bounded_provider_module() -> TerminalModule {
    let mut module = provider_module();
    let bound_service = ServiceId::new(1).expect("service identity");
    module.services.push(ServiceDeclaration {
        id: bound_service,
        identity: "PortIo".into(),
        parents: Vec::new(),
    });
    let bounded = BoundaryMachineId::new(2).expect("boundary identity");
    module.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        id: bounded,
        identity: "MachineControl::mask".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: vec![bound_service],
    });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).expect("operation identity"),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary: bounded,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    // The caller's published ceiling conservatively covers the dependency's
    // bound; `installation_dependencies` keeps the unresolved row distinct.
    module.machines[0].published_service_ceiling = vec![bound_service];
    module
        .root_service_reach
        .installation_dependencies
        .push(InstallationReachDependency {
            requirement_identity: "MachineControl::mask".into(),
            upper_bound: vec![bound_service],
        });
    module
}

fn artifact_for(module: &TerminalModule) -> terminal_codec::CanonicalTerminalArtifact {
    let proof = ProofBundle::default();
    let record = terminal_codec::build_identity_optimization_execution_record(module, &proof)
        .expect("identity optimization record");
    terminal_codec::CanonicalTerminalArtifact::from_parts(module, &proof, &record, None)
        .expect("canonical artifact")
}

fn empty_selection() -> SelectedProviderPlanFacts {
    SelectedProviderPlanFacts::from_selected_plans(Vec::new()).expect("empty selection")
}

fn selected_plan() -> ProviderPlan {
    ProviderPlan {
        name: "SelectedIndexedProvider".into(),
        provider_type: "IndexedProvider".into(),
        provider_type_package_identity: None,
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "IndexedRequirement".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                name: "apply".into(),
                requirement_owner: "IndexedRequirement".into(),
                requirement_owner_package_identity: None,
                requirement_identity: "IndexedRequirement::apply".into(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["IndexedRequirement".into()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "apply".into(),
            requirement_identity: "IndexedRequirement::apply".into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: "IndexedProvider::apply".into(),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    }
}

fn describe(module: &TerminalModule, selected: &SelectedProviderPlanFacts) -> ComponentDescription {
    let artifact = artifact_for(module);
    crate::describe_component_facts(ComponentDescriptionFacts {
        artifact: &artifact,
        selected_provider_plans: selected,
        component_progress: None,
        stack_demand: None,
        realization_identity: None,
    })
    .expect("component description")
}

fn request_for(
    module: &TerminalModule,
    accepted_assumptions: BTreeSet<[u8; 32]>,
) -> ComponentVerificationRequest {
    ComponentVerificationRequest {
        expected_subject: terminal_codec::terminal_psi_identity(module).expect("module identity"),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V2]),
        accepted_assumptions,
        admission_profile: super::AdmissionProfile::default(),
    }
}

fn verify(
    description: &ComponentDescription,
    request: &ComponentVerificationRequest,
) -> Result<VerifiedComponent, ComponentVerificationRejection> {
    verify_component(&encode_component_description(description), request)
}

#[test]
fn verifies_a_complete_minimal_description() {
    let module = minimal_module();
    let description = describe(&module, &empty_selection());
    let request = request_for(&module, BTreeSet::new());
    let verified = verify(&description, &request).expect("complete description verifies");
    assert_eq!(
        verified.frontier(),
        DescriptionFrontier::TerminalArtifactClosure
    );
    assert!(
        verified
            .entries()
            .iter()
            .any(|entry| entry.kind == ComponentEntryKind::Canonical)
    );
    assert!(verified.imports().is_empty());
    assert!(verified.providers().is_empty());
}

#[test]
fn verifies_a_sealed_boundary_requirement() {
    let module = boundary_module();
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()])
        .expect("selected closure");
    let description = describe(&module, &selected);
    assert_eq!(description.imports.len(), 0);
    assert_eq!(description.providers.len(), 1);
    let request = request_for(&module, BTreeSet::new());
    let verified = verify(&description, &request).expect("sealed requirement verifies");
    assert_eq!(verified.providers().len(), 1);
    assert!(
        verified
            .obligations()
            .iter()
            .any(|obligation| obligation.kind == ObligationKind::ProviderOccurrence),
        "a retained provider stays a per-occurrence installation obligation"
    );
}

#[test]
fn exports_checked_realizations_and_joins_the_selected_plan() {
    let module = provider_module();
    let description = describe(&module, &empty_selection());
    let export = requirement_export_identity(
        "IndexedRequirement::apply",
        "IndexedProvider",
        "IndexedProvider::apply",
    );
    assert!(
        description
            .exports
            .iter()
            .any(|surface| surface.identity == export),
        "the retained checked candidate is an exported realization: {:?}",
        description.exports
    );
    assert!(
        description.imports.is_empty() && description.outgoing.is_empty(),
        "an uncalled realization demands nothing outgoing"
    );
    let request = request_for(&module, BTreeSet::new());
    let verified = verify(&description, &request).expect("provider component verifies");
    verified
        .realizes_selected_plan(&selected_plan())
        .expect("the selected plan joins its exact exported realization");
}

#[test]
fn verified_component_publishes_export_contracts_for_the_demand_join() {
    let module = provider_module();
    let description = describe(&module, &empty_selection());
    let request = request_for(&module, BTreeSet::new());
    let verified = verify(&description, &request).expect("provider component verifies");

    let contracts = verified.export_contracts();
    assert_eq!(
        contracts.len(),
        verified.exports().len(),
        "every export surface carries exactly one join contract"
    );
    for (surface, contract) in verified.exports().iter().zip(contracts.iter()) {
        assert_eq!(
            surface.identity, contract.identity,
            "contract rows align with the surface roster by identity"
        );
    }

    // The offered contract is the demanded requirement's contract identity:
    // an import slot on `IndexedRequirement::apply` joins this surface by
    // digest equality, never by parsing the opaque export identity.
    let offered = contracts
        .iter()
        .find(|contract| {
            contract.identity
                == requirement_export_identity(
                    "IndexedRequirement::apply",
                    "IndexedProvider",
                    "IndexedProvider::apply",
                )
        })
        .expect("the realization export carries a contract");
    assert_eq!(
        offered.contract_identity,
        requirement_contract_identity("IndexedRequirement::apply")
    );

    let canonical = contracts
        .iter()
        .find(|contract| contract.identity.starts_with("export:canonical:"))
        .expect("the canonical entry carries a contract");
    assert_eq!(
        canonical.contract_identity,
        canonical_entry_contract_identity(module.entry)
    );
    assert_ne!(
        canonical.contract_identity, offered.contract_identity,
        "the canonical entry can never satisfy a requirement demand"
    );
}

#[test]
fn rejects_omitted_and_forged_realization_exports() {
    let module = provider_module();
    let request = request_for(&module, BTreeSet::new());

    let mut omitted = describe(&module, &empty_selection());
    omitted
        .exports
        .retain(|surface| !surface.identity.starts_with("export:requirement:"));
    assert!(matches!(
        verify(&omitted, &request),
        Err(ComponentVerificationRejection::MissingExport(_))
    ));

    let mut forged = describe(&module, &empty_selection());
    forged.exports.push(ExportSurface {
        identity: requirement_export_identity(
            "IndexedRequirement::apply",
            "ForgedProvider",
            "ForgedProvider::apply",
        ),
    });
    assert!(matches!(
        verify(&forged, &request),
        Err(ComponentVerificationRejection::UnexpectedDerivedExport(_))
    ));

    // A description for another subject cannot stand in for the provider
    // component even when it carries the same export spelling.
    let other = minimal_module();
    let mut substituted = describe(&other, &empty_selection());
    substituted.exports.push(ExportSurface {
        identity: requirement_export_identity(
            "IndexedRequirement::apply",
            "IndexedProvider",
            "IndexedProvider::apply",
        ),
    });
    assert!(matches!(
        verify(&substituted, &request),
        Err(ComponentVerificationRejection::WrongSubject { .. })
    ));
}

#[test]
fn selected_plan_join_rejects_every_substitution() {
    let module = provider_module();
    let description = describe(&module, &empty_selection());
    let request = request_for(&module, BTreeSet::new());
    let verified = verify(&description, &request).expect("provider component verifies");

    let mut other_provider = selected_plan();
    other_provider.provider_type = "OtherProvider".into();
    assert_eq!(
        verified.realizes_selected_plan(&other_provider),
        Err(IndependentRealizationMismatch::ProviderTypeMismatch {
            requirement_identity: "IndexedRequirement::apply".into(),
            selected_provider: "OtherProvider".into(),
        })
    );

    let mut other_machine = selected_plan();
    other_machine.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: "IndexedProvider::other".into(),
        machine_package_identity: None,
    };
    assert_eq!(
        verified.realizes_selected_plan(&other_machine),
        Err(IndependentRealizationMismatch::MachineMismatch {
            requirement_identity: "IndexedRequirement::apply".into(),
            selected_machine: "IndexedProvider::other".into(),
        })
    );

    let mut other_requirement = selected_plan();
    other_requirement.schema.methods[0].requirement_identity = "Other::apply".into();
    other_requirement.rows[0].requirement_identity = "Other::apply".into();
    assert_eq!(
        verified.realizes_selected_plan(&other_requirement),
        Err(IndependentRealizationMismatch::MissingRealization {
            requirement_identity: "Other::apply".into(),
        })
    );

    let mut unchecked = selected_plan();
    unchecked.rows[0].binding = ProviderBinding::Syscall { number: 60 };
    assert_eq!(
        verified.realizes_selected_plan(&unchecked),
        Err(IndependentRealizationMismatch::UncheckedRow {
            requirement_identity: "IndexedRequirement::apply".into(),
        })
    );

    let mut drifted_package = selected_plan();
    drifted_package.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: "IndexedProvider::apply".into(),
        machine_package_identity: semantic_vocabulary::PackageKeyIdentity::from_digest([3u8; 32]),
    };
    assert!(matches!(
        verified.realizes_selected_plan(&drifted_package),
        Err(IndependentRealizationMismatch::InvalidPlan { .. })
    ));

    let mut nameless = selected_plan();
    nameless.provider_type = String::new();
    assert_eq!(
        verified.realizes_selected_plan(&nameless),
        Err(IndependentRealizationMismatch::EmptyProviderType {
            plan: "SelectedIndexedProvider".into(),
        })
    );

    // A component whose module still retains an unresolved
    // installation-bound reach row is not a closed callable component even
    // when the selected realization itself matches: the published bound is
    // an obligation installation still owes.
    let bounded = bounded_provider_module();
    let bounded_description = describe(&bounded, &empty_selection());
    let bounded_verified = verify(
        &bounded_description,
        &request_for(&bounded, BTreeSet::new()),
    )
    .expect("a component retaining a bound still verifies");
    assert_eq!(
        bounded_verified.realizes_selected_plan(&selected_plan()),
        Err(IndependentRealizationMismatch::UnresolvedInstallationRows {
            requirement_identities: vec!["MachineControl::mask".into()],
        })
    );

    let mut partial = selected_plan();
    partial.rows.clear();
    assert!(matches!(
        verified.realizes_selected_plan(&partial),
        Err(IndependentRealizationMismatch::InvalidPlan { .. })
    ));

    // A verified component of another subject realizes nothing for the plan.
    let bare = minimal_module();
    let bare_description = describe(&bare, &empty_selection());
    let bare_verified = verify(&bare_description, &request_for(&bare, BTreeSet::new()))
        .expect("bare component verifies");
    assert_eq!(
        bare_verified.realizes_selected_plan(&selected_plan()),
        Err(IndependentRealizationMismatch::MissingRealization {
            requirement_identity: "IndexedRequirement::apply".into(),
        })
    );
}

#[test]
fn rejects_corrupt_descriptions() {
    let module = minimal_module();
    let description = describe(&module, &empty_selection());
    let bytes = encode_component_description(&description);
    let request = request_for(&module, BTreeSet::new());
    assert!(matches!(
        verify_component(&[], &request),
        Err(ComponentVerificationRejection::Corrupt(
            DescriptionDecodeRejection::InvalidMagic | DescriptionDecodeRejection::Corrupt(_)
        ))
    ));
    assert!(matches!(
        verify_component(&bytes[..bytes.len() - 3], &request),
        Err(ComponentVerificationRejection::Corrupt(_))
    ));
    let mut mutated = bytes.clone();
    mutated[0] = b'X';
    assert!(matches!(
        verify_component(&mutated, &request),
        Err(ComponentVerificationRejection::Corrupt(
            DescriptionDecodeRejection::InvalidMagic
        ))
    ));
}

#[test]
fn rejects_an_incompatible_schema() {
    let module = minimal_module();
    let description = describe(&module, &empty_selection());
    let mut request = request_for(&module, BTreeSet::new());
    // The retired V1 envelope — no separate service-bound roster — is not
    // admitted for a V2 description.
    request.accepted_schemas = BTreeSet::from([1]);
    assert!(matches!(
        verify(&description, &request),
        Err(ComponentVerificationRejection::IncompatibleSchema {
            schema: COMPONENT_DESCRIPTION_SCHEMA_V2
        })
    ));
}

#[test]
fn publishes_installation_bounds_separately_from_concrete_reach() {
    let module = bounded_provider_module();
    let description = describe(&module, &empty_selection());

    // The retained dependency's declared bound is its own roster entry; it
    // never appears as a `ServiceCeiling` outgoing row, because the bound is
    // what installation still owes — not reach the component's own closure
    // already publishes.
    assert_eq!(
        description.service_bounds,
        vec![InstallationServiceBound {
            requirement_identity: "MachineControl::mask".into(),
            bound: vec![ServiceId::new(1).expect("service identity")],
        }]
    );
    assert!(
        !description
            .outgoing
            .iter()
            .any(|row| row.class == OutgoingAuthorityClass::ServiceCeiling),
        "the module's concrete root reach is empty: {:?}",
        description.outgoing
    );

    let request = request_for(&module, BTreeSet::new());
    let verified = verify(&description, &request).expect("bounded component verifies");
    assert_eq!(
        verified.service_bounds(),
        description.service_bounds.as_slice()
    );

    // Omitting or forging the bound roster rejects on replay; the verifier
    // re-derives it from the module, never from producer claims.
    let mut omitted = describe(&module, &empty_selection());
    omitted.service_bounds.clear();
    assert!(matches!(
        verify(&omitted, &request),
        Err(ComponentVerificationRejection::MissingServiceBound(_))
    ));
    let mut forged = describe(&module, &empty_selection());
    forged.service_bounds.push(InstallationServiceBound {
        requirement_identity: "Forged::requirement".into(),
        bound: vec![ServiceId::new(1).expect("service identity")],
    });
    assert!(matches!(
        verify(&forged, &request),
        Err(ComponentVerificationRejection::UnexpectedServiceBound(_))
    ));

    // Nor can the bound masquerade as concrete reach: adding the service as
    // a ceiling row is authority the artifact lacks.
    let mut masquerade = describe(&module, &empty_selection());
    masquerade
        .outgoing
        .push(crate::component_description::OutgoingAuthority {
            class: OutgoingAuthorityClass::ServiceCeiling,
            identity: "service-ceiling:1".into(),
            evidence: OutgoingEvidence::ModuleDerived,
        });
    assert!(matches!(
        verify(&masquerade, &request),
        Err(ComponentVerificationRejection::UnexpectedDerivedAuthority(
            _
        ))
    ));
}

#[test]
fn rejects_unaccepted_and_unbound_assumptions() {
    let module = minimal_module();
    let assumption = [7u8; 32];

    let mut declared = describe(&module, &empty_selection());
    declared.assumptions.push(assumption);
    declared.entries.push(ComponentEntry {
        kind: ComponentEntryKind::Startup,
        identity: "startup:post-link".into(),
        evidence: EntryEvidence::AssumptionBound(assumption),
    });
    let accepted = request_for(&module, BTreeSet::from([assumption]));
    verify(&declared, &accepted).expect("accepted assumption verifies");

    let unaccepted = request_for(&module, BTreeSet::new());
    assert!(matches!(
        verify(&declared, &unaccepted),
        Err(ComponentVerificationRejection::UnacceptedAssumption(_))
    ));

    let mut unbound = describe(&module, &empty_selection());
    unbound.entries.push(ComponentEntry {
        kind: ComponentEntryKind::Timer,
        identity: "timer:tick".into(),
        evidence: EntryEvidence::AssumptionBound(assumption),
    });
    let accepted = request_for(&module, BTreeSet::from([assumption]));
    assert!(matches!(
        verify(&unbound, &accepted),
        Err(ComponentVerificationRejection::UnboundAssumptionReference(
            _
        ))
    ));
}

#[test]
fn rejects_provider_roster_substitution() {
    let module = boundary_module();
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()])
        .expect("selected closure");
    let request = request_for(&module, BTreeSet::new());

    let mut wrong_seal = describe(&module, &selected);
    for row in &mut wrong_seal.outgoing {
        if row.class == OutgoingAuthorityClass::BoundaryRequirement {
            row.evidence = OutgoingEvidence::ProviderSealed([9u8; 32]);
        }
    }
    assert!(matches!(
        verify(&wrong_seal, &request),
        Err(ComponentVerificationRejection::UnsealedProvider(_))
    ));

    let mut smuggled = describe(&module, &selected);
    smuggled.providers[0]
        .requirement_identities
        .push("Smuggled::requirement".into());
    assert!(matches!(
        verify(&smuggled, &request),
        Err(ComponentVerificationRejection::SmuggledProviderRequirement(
            _
        )) | Err(ComponentVerificationRejection::InconsistentProviderClosure(
            _
        ))
    ));
}

// ---------------------------------------------------------------------------
// One-field substitution coverage
// ---------------------------------------------------------------------------

/// A module whose entry performs three bare `BoundaryCall`s — one sealed by
/// the retained selected provider, one left unsealed as an installation
/// import, one installation-bound (`reaches <= Bound`) — and records one
/// suspension call site, so every component-description roster is populated
/// for one-field substitution coverage. The entry also declares the bound
/// service concretely, so the same service publishes both as a
/// `ServiceCeiling` outgoing row and inside the retained bound: the two
/// stay distinct fields even when they name the same service.
fn described_module() -> TerminalModule {
    let mut module = boundary_module();
    let bound_service = ServiceId::new(1).expect("service identity");
    module.services.push(ServiceDeclaration {
        id: bound_service,
        identity: "PortIo".into(),
        parents: Vec::new(),
    });
    module.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        id: BoundaryMachineId::new(2).expect("boundary identity"),
        identity: "Unsealed::requirement".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        id: BoundaryMachineId::new(3).expect("boundary identity"),
        identity: "Bound::requirement".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: vec![bound_service],
    });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(2).expect("operation identity"),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary: BoundaryMachineId::new(2).expect("boundary identity"),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(3).expect("operation identity"),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary: BoundaryMachineId::new(3).expect("boundary identity"),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    // The caller's published ceiling covers both its own declared reach and
    // the bound boundary's published ceiling. The retained dependency row's
    // bound overlaps the concrete reach on purpose: the description must
    // publish them as distinct rosters anyway.
    module.machines[0].declared_service_reach = vec![bound_service];
    module.machines[0].published_service_ceiling = vec![bound_service];
    module.root_service_reach.concrete = vec![bound_service];
    module
        .root_service_reach
        .installation_dependencies
        .push(InstallationReachDependency {
            requirement_identity: "Bound::requirement".into(),
            upper_bound: vec![bound_service],
        });
    // A suspension call site contributes a resumption entry row and a custody
    // row. The verifier requires every site to pair with an exact plan: same
    // operation, crossing, and target, plus the plan's replayed frontier
    // commitment.
    let plan = TerminalSuspensionCallPlan {
        operation: OperationId::new(1).expect("operation identity"),
        crossing: SuspensionCrossingId::new(1).expect("crossing identity"),
        target: TerminalSuspensionCallTarget::Boundary(
            BoundaryMachineId::new(1).expect("boundary identity"),
        ),
        effective: CarryPolicy::PERMISSIVE,
        live_value_count: 0,
        live_values: Vec::new(),
    };
    // The operation's own marker is the authoritative crossing demand: a plan
    // naming a call side that does not declare it is rewritten evidence.
    let marked = module
        .machines
        .iter_mut()
        .flat_map(|machine| machine.blocks.iter_mut())
        .flat_map(|block| block.operations.iter_mut())
        .find(|candidate| candidate.id == plan.operation)
        .map(|candidate| candidate.suspension_crossing = Some(plan.crossing))
        .is_some();
    assert!(
        marked,
        "the suspension plan names an operation in the module"
    );
    module
        .suspension_call_sites
        .push(TerminalSuspensionCallSite {
            operation: plan.operation,
            crossing: plan.crossing,
            target: plan.target,
            frontier_commitment: terminal_psi::suspension_frontier_commitment(&plan),
        });
    module.suspension_call_plan_count = 1;
    module.suspension_call_plans.push(plan);
    module
}

/// Replay-side assertion for a substitution that stays independently
/// representable: the codec must preserve the mutated description exactly
/// (mutators keep every roster in canonical order), the honestly recomputed
/// description identity must differ from the authentic identity, and
/// independent replay must reject with the expected category.
fn assert_substitution_rejected_by_replay(
    field: &str,
    description: &ComponentDescription,
    request: &ComponentVerificationRequest,
    mutate: impl Fn(&mut ComponentDescription),
    expected: impl Fn(&ComponentVerificationRejection) -> bool,
) {
    verify(description, request).unwrap_or_else(|rejection| {
        panic!("{field}: authentic description verifies: {rejection:?}")
    });
    let mut changed = description.clone();
    mutate(&mut changed);
    assert_ne!(
        changed, *description,
        "{field}: substitution changes the description"
    );
    let bytes = encode_component_description(&changed);
    let replayed = decode_component_description(&bytes).unwrap_or_else(|rejection| {
        panic!("{field}: substituted description decodes: {rejection:?}")
    });
    assert_eq!(
        replayed, changed,
        "{field}: codec preserves the substituted description"
    );
    assert_ne!(
        component_description_identity(&bytes),
        component_description_identity(&encode_component_description(description)),
        "{field}: recomputed identity differs from the authentic description"
    );
    match verify_component(&bytes, request) {
        Err(rejection) => assert!(
            expected(&rejection),
            "{field}: independent replay rejects as expected, got {rejection:?}"
        ),
        Ok(_) => panic!("{field}: independent replay accepted the substitution"),
    }
}

/// Decode-side assertion for a substitution that is not independently
/// representable: the canonical codec rejects the mutated bytes before any
/// identity or replay could admit them.
fn assert_substitution_rejected_at_decoding(
    field: &str,
    description: &ComponentDescription,
    mutate: impl Fn(&mut ComponentDescription),
    expected: DescriptionDecodeRejection,
) {
    let mut changed = description.clone();
    mutate(&mut changed);
    assert_ne!(
        changed, *description,
        "{field}: substitution changes the description"
    );
    assert_eq!(
        decode_component_description(&encode_component_description(&changed)),
        Err(expected),
        "{field}: canonical decoding rejects the substitution"
    );
}

/// Wire-level assertion: raw byte substitutions that no encoder-produced
/// description can express are rejected at canonical decoding.
fn assert_wire_rejected_at_decoding(
    field: &str,
    description: &ComponentDescription,
    mutate: impl Fn(&mut Vec<u8>),
    expected: DescriptionDecodeRejection,
) {
    let mut bytes = encode_component_description(description);
    mutate(&mut bytes);
    assert_eq!(
        decode_component_description(&bytes),
        Err(expected),
        "{field}: canonical decoding rejects the substitution"
    );
}

/// Identity-only assertion for declared, non-authoritative fields: the
/// substitution still encodes and still changes the recomputed description
/// identity, but independent replay admits it because the field is declared
/// evidence rather than replayable fact. These cases pin down exactly which
/// fields the description itself does not authenticate — binding slots,
/// report coordinates, human-readable detail, the upstream-bound closure
/// digest, the optional realization identity, and assumption-bound declared
/// rows — so no declared field silently acquires replay authority.
fn assert_substitution_bound_only_by_identity(
    field: &str,
    description: &ComponentDescription,
    request: &ComponentVerificationRequest,
    mutate: impl Fn(&mut ComponentDescription),
) {
    verify(description, request).unwrap_or_else(|rejection| {
        panic!("{field}: authentic description verifies: {rejection:?}")
    });
    let mut changed = description.clone();
    mutate(&mut changed);
    assert_ne!(
        changed, *description,
        "{field}: substitution changes the description"
    );
    let bytes = encode_component_description(&changed);
    let replayed = decode_component_description(&bytes).unwrap_or_else(|rejection| {
        panic!("{field}: substituted description decodes: {rejection:?}")
    });
    assert_eq!(
        replayed, changed,
        "{field}: codec preserves the substituted description"
    );
    assert_ne!(
        component_description_identity(&bytes),
        component_description_identity(&encode_component_description(description)),
        "{field}: recomputed identity differs from the authentic description"
    );
    verify_component(&bytes, request).unwrap_or_else(|rejection| {
        panic!("{field}: replay rejected a declared evidence-only field: {rejection:?}")
    });
}

/// Byte offset of the import roster's count inside the canonical encoding,
/// recomputed from the description's own fields: magic (8), schema (4),
/// frontier tag (1), then the length-framed embedded artifact.
fn imports_count_offset(description: &ComponentDescription) -> usize {
    8 + 4 + 1 + 4 + description.artifact_bytes.len()
}

/// Byte offset of the entry roster's count inside the canonical encoding:
/// after the length-framed import and export rosters.
fn entries_count_offset(description: &ComponentDescription) -> usize {
    let mut offset = imports_count_offset(description) + 4;
    for slot in &description.imports {
        offset += 4 + 4 + slot.requirement_identity.len() + 32;
    }
    offset += 4;
    for export in &description.exports {
        offset += 4 + export.identity.len();
    }
    offset
}

/// Every representable field of a canonical component description is
/// authenticated: a one-field substitution either stays independently
/// representable and is rejected by independent replay, or violates canonical
/// form and is rejected at decoding before identity or replay could admit it.
#[test]
fn component_description_rejects_every_one_field_substitution() {
    let module = described_module();
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()])
        .expect("selected closure");
    let description = describe(&module, &selected);
    let request = request_for(&module, BTreeSet::new());
    verify(&description, &request).expect("authentic description verifies");

    let bytes = encode_component_description(&description);
    let decoded = decode_component_description(&bytes).expect("canonical decode");
    assert_eq!(decoded, description);
    assert_eq!(encode_component_description(&decoded), bytes);

    // The fixture populates every roster: one sealed requirement, two
    // unsealed imports (one ordinary, one installation-bound), canonical
    // plus suspension-resumption entries, one custody row, one retained
    // provider, one service bound, and both obligation kinds.
    assert_eq!(description.imports.len(), 2);
    assert_eq!(description.exports.len(), 1);
    assert_eq!(description.entries.len(), 2);
    assert_eq!(description.outgoing.len(), 4);
    assert_eq!(description.service_bounds.len(), 1);
    assert_eq!(description.custody.len(), 1);
    assert_eq!(description.providers.len(), 1);
    assert_eq!(description.obligations.len(), 3);
    let donor = foreign_description_donor();
    let donor_request = request_for(&minimal_module(), BTreeSet::new());
    verify(&donor, &donor_request).expect("the foreign donor verifies under its own subject");

    // The driver substitutes before every check, so `leg` always names the
    // lane an internal invariant failure belongs to.
    let leg = Cell::new(ComponentDescriptionFieldForTest::SchemaBumped);
    let substitute = |record: &mut ComponentDescription,
                      field: ComponentDescriptionFieldForTest,
                      donor: &ComponentDescription| {
        leg.set(field);
        substitute_component_description_for_test(record, field, donor);
    };
    // The family's independent checker: the canonical codec plus
    // `verify_component` replay under the admitting request. A record the
    // codec still carries must round-trip byte-exact before the replay
    // verdict is read; the refusal is either the decode's own exact
    // rejection (Corrupt) or the replay's named kind (Replay).
    let check =
        |record: &ComponentDescription| -> Result<[u8; 32], ComponentDescriptionCustodyCheck> {
            let name = format!("{:?}", leg.get());
            let bytes = encode_component_description(record);
            if let Ok(decoded) = decode_component_description(&bytes) {
                assert_eq!(
                    decoded, *record,
                    "{name}: the codec preserves the substituted description"
                );
            }
            match verify_component(&bytes, &request) {
                Err(ComponentVerificationRejection::Corrupt(inner)) => {
                    Err(ComponentDescriptionCustodyCheck::Corrupt(inner))
                }
                Err(rejection) => Err(ComponentDescriptionCustodyCheck::Replay(classify_replay(
                    &rejection,
                ))),
                Ok(_) => Ok(component_description_identity(&bytes)),
            }
        };

    // The retained custody view is the canonical encoding's identity: every
    // leg changes the encoded bytes, so the recomputed containing identity
    // honestly diverges before the checker's verdict is read.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "canonical component description",
        fields: ComponentDescriptionFieldForTest::INVENTORY,
        honest: &|| description.clone(),
        donor,
        custody: &|record: &ComponentDescription| {
            component_description_identity(&encode_component_description(record))
        },
        substitute: &substitute,
        check: &check,
        outcome: &component_description_custody_outcome,
        joined_replay: None,
    });

    // A derived row cannot hide behind declared evidence: its key stays in
    // the derived set, so an `AssumptionBound` substitution is rejected as
    // forbidden evidence rather than admitted as a declaration. Binding the
    // digest into the roster (and the request) stages the row so the class
    // check is the rejection under test.
    let assumption = [7u8; 32];
    let accepted = request_for(&module, BTreeSet::from([assumption]));
    assert_substitution_rejected_by_replay(
        "outgoing[unsealed].evidence::assumption-bound",
        &description,
        &accepted,
        move |d| {
            d.assumptions.push(assumption);
            let row = d
                .outgoing
                .iter_mut()
                .find(|row| row.identity == "Unsealed::requirement")
                .expect("unsealed row");
            row.evidence = OutgoingEvidence::AssumptionBound(assumption);
            d.outgoing.sort();
        },
        |r| {
            matches!(
                r,
                ComponentVerificationRejection::InvalidAuthorityEvidence(_)
            )
        },
    );

    // Roster order is presentation-only in this family: the encoder
    // canonicalizes it, so a pure reorder is not a substitution at all.
    let mut reordered = description.clone();
    reordered.obligations.swap(0, 1);
    assert_ne!(reordered, description);
    assert_eq!(
        encode_component_description(&reordered),
        encode_component_description(&description),
        "a pure roster reorder is the same canonical description"
    );

    // Raw wire substitutions no encoder-produced description can express are
    // rejected at canonical decoding.
    let imports_at = imports_count_offset(&description);
    let entries_at = entries_count_offset(&description);
    let first_entry_identity_len = description
        .entries
        .iter()
        .min()
        .expect("canonical entry")
        .identity
        .len();
    let wire_rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut Vec<u8>)>,
        DescriptionDecodeRejection,
    )> = vec![
        (
            "wire::magic",
            Box::new(|bytes| bytes[0] = b'X'),
            DescriptionDecodeRejection::InvalidMagic,
        ),
        (
            "wire::frontier-tag",
            Box::new(|bytes| bytes[12] = 9),
            DescriptionDecodeRejection::UnsupportedTag("frontier tag"),
        ),
        (
            "wire::entry-kind-tag",
            Box::new(move |bytes| bytes[entries_at + 4] = 9),
            DescriptionDecodeRejection::UnsupportedTag("entry kind"),
        ),
        (
            "wire::entry-evidence-tag",
            Box::new(move |bytes| {
                bytes[entries_at + 4 + 1 + 4 + first_entry_identity_len] = 9;
            }),
            DescriptionDecodeRejection::UnsupportedTag("entry evidence"),
        ),
        (
            "wire::import-roster-bound",
            Box::new(move |bytes| {
                bytes[imports_at..imports_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            }),
            DescriptionDecodeRejection::RosterBoundExceeded("import"),
        ),
        (
            "wire::trailing",
            Box::new(|bytes| bytes.push(0)),
            DescriptionDecodeRejection::Corrupt("trailing bytes"),
        ),
        (
            "wire::truncated",
            Box::new(|bytes| {
                bytes.pop();
            }),
            DescriptionDecodeRejection::Corrupt("truncated input"),
        ),
    ];
    for (field, mutate, expected) in wire_rejected {
        assert_wire_rejected_at_decoding(field, &description, mutate, expected);
    }
}

/// Declared and evidence-only fields are deliberately not replayable: the
/// verifier cannot re-derive a binding-slot index, a report coordinate, an
/// obligation's human-readable detail, the upstream-bound closure digest, the
/// optional realization identity, or the content of assumption-bound declared
/// rows. Each substitution still changes the canonical bytes and therefore
/// the recomputed description identity — these cases pin down exactly which
/// fields the identity alone authenticates, so no declared field silently
/// acquires replay authority. Fields that *are* replay-bound still reject.
#[test]
fn component_description_declared_fields_stay_identity_bound() {
    let module = described_module();
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()])
        .expect("selected closure");
    let description = describe(&module, &selected);
    let request = request_for(&module, BTreeSet::new());

    let identity_bound: Vec<(&'static str, Box<dyn Fn(&mut ComponentDescription)>)> = vec![
        ("imports[1].slot", Box::new(|d| d.imports[1].slot = 7)),
        (
            "providers[0].report_identity",
            Box::new(|d| d.providers[0].report_identity += 1),
        ),
        (
            "obligations[0].detail",
            Box::new(|d| d.obligations[0].detail = "different detail".into()),
        ),
        (
            "provider_closure_digest::nonzero",
            Box::new(|d| d.provider_closure_digest = [9u8; 32]),
        ),
        (
            "realization_identity",
            Box::new(|d| d.realization_identity = Some([9u8; 32])),
        ),
        (
            "imports::insert-second-slot",
            Box::new(|d| {
                d.imports.push(ImportSlot {
                    slot: 2,
                    requirement_identity: "Unsealed::requirement".into(),
                    contract_identity: requirement_contract_identity("Unsealed::requirement"),
                });
            }),
        ),
        (
            "obligations::insert-declared",
            Box::new(|d| {
                d.obligations.insert(
                    0,
                    InstallationObligation {
                        kind: ObligationKind::StackProvision,
                        identity: "stack-provision:1:4096:16".into(),
                        detail: "declared demand".into(),
                    },
                );
            }),
        ),
    ];
    for (field, mutate) in identity_bound {
        assert_substitution_bound_only_by_identity(field, &description, &request, mutate);
    }

    // A declared assumption-bound entry: replay can only require its digest
    // to be roster-bound and accepted; the row's kind, identity, and presence
    // are producer-declared facts the containing identity alone
    // authenticates.
    let assumption = [7u8; 32];
    let mut declared = description.clone();
    declared.assumptions.push(assumption);
    declared.entries.push(ComponentEntry {
        kind: ComponentEntryKind::Startup,
        identity: "startup:post-link".into(),
        evidence: EntryEvidence::AssumptionBound(assumption),
    });
    let accepted = request_for(&module, BTreeSet::from([assumption]));
    verify(&declared, &accepted).expect("declared entry verifies");

    // Weakening a module-derived row's evidence to an accepted assumption is
    // admitted: the row still covers the derived set, and the digest is
    // bound. The declaration stays identity-bound, not replay-bound.
    assert_substitution_bound_only_by_identity(
        "entries[canonical].evidence::assumption-bound",
        &declared,
        &accepted,
        move |d| {
            d.entries
                .iter_mut()
                .find(|entry| entry.kind == ComponentEntryKind::Canonical)
                .expect("canonical entry")
                .evidence = EntryEvidence::AssumptionBound(assumption);
        },
    );
    assert_substitution_bound_only_by_identity(
        "custody[0].evidence::assumption-bound",
        &declared,
        &accepted,
        move |d| {
            d.custody[0].evidence = CustodyEvidence::AssumptionBound(assumption);
        },
    );

    let declared_rows: Vec<(&'static str, Box<dyn Fn(&mut ComponentDescription)>)> = vec![
        (
            "entries[startup].identity",
            Box::new(|d| {
                d.entries
                    .iter_mut()
                    .find(|entry| entry.kind == ComponentEntryKind::Startup)
                    .expect("declared entry")
                    .identity = "startup:renamed".into();
            }),
        ),
        (
            "entries[startup].kind",
            Box::new(|d| {
                let entry = d
                    .entries
                    .iter_mut()
                    .find(|entry| entry.kind == ComponentEntryKind::Startup)
                    .expect("declared entry");
                entry.kind = ComponentEntryKind::Cleanup;
                d.entries.sort();
            }),
        ),
        (
            "entries[startup]::drop",
            Box::new(|d| {
                d.entries
                    .retain(|entry| entry.kind != ComponentEntryKind::Startup);
            }),
        ),
    ];
    for (field, mutate) in declared_rows {
        assert_substitution_bound_only_by_identity(field, &declared, &accepted, mutate);
    }

    // The bound digest itself stays replay-checked: retargeting a declared
    // row at a digest the roster does not name rejects, as does renaming the
    // roster digest out from under its row — whether the new digest is
    // unaccepted or accepted-but-unbound.
    assert_substitution_rejected_by_replay(
        "entries[startup].evidence::rebound",
        &declared,
        &accepted,
        |d| {
            d.entries
                .iter_mut()
                .find(|entry| entry.kind == ComponentEntryKind::Startup)
                .expect("declared entry")
                .evidence = EntryEvidence::AssumptionBound([9u8; 32]);
        },
        |r| {
            matches!(
                r,
                ComponentVerificationRejection::UnboundAssumptionReference(_)
            )
        },
    );
    assert_substitution_rejected_by_replay(
        "assumptions[0]::unaccepted",
        &declared,
        &accepted,
        |d| d.assumptions[0] = [9u8; 32],
        |r| matches!(r, ComponentVerificationRejection::UnacceptedAssumption(_)),
    );
    let accepts_both = request_for(&module, BTreeSet::from([assumption, [9u8; 32]]));
    assert_substitution_rejected_by_replay(
        "assumptions[0]::accepted-but-unbound",
        &declared,
        &accepts_both,
        |d| d.assumptions[0] = [9u8; 32],
        |r| {
            matches!(
                r,
                ComponentVerificationRejection::UnboundAssumptionReference(_)
            )
        },
    );

    // A duplicated assumption digest is not independently representable.
    assert_substitution_rejected_at_decoding(
        "assumptions::duplicate",
        &declared,
        move |d| d.assumptions.push(assumption),
        DescriptionDecodeRejection::NonCanonicalOrder("assumption"),
    );
}

#[test]
fn rejects_a_module_the_terminal_verifier_refuses() {
    // A module that decodes and subject-matches but whose machine contract
    // ensures a clause no evidence discharges is not a verified component:
    // the verifier, not the decode, owns the code-to-inventory claim.
    let mut module = minimal_module();
    module.machines[0]
        .contract
        .ensures
        .push(terminal_psi::ContractClause {
            obligation: semantic_vocabulary::ObligationId::new(1).expect("obligation"),
            proposition: semantic_vocabulary::Proposition::Falsehood,
        });
    let description = describe(&module, &empty_selection());
    let request = request_for(&module, BTreeSet::new());
    assert!(matches!(
        verify(&description, &request),
        Err(ComponentVerificationRejection::ModuleVerification(_))
    ));
}
