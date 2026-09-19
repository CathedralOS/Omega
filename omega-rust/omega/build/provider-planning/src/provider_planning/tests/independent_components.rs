//! Component-closure fence: `Independent` selections join verified
//! component descriptions.
//!
//! The provider plan comes from a real source fixture; the realizing
//! component is a canonical Terminal module carrying the checked candidate
//! with that plan's exact coordinates, described by
//! `describe_component_facts` and admitted by `verify_component`. The fence
//! never reads a hand-authored inventory: every join goes through the
//! verified module's provider-candidate catalog.

use std::collections::BTreeSet;

use crate::provider_planning::{
    ProviderBinding, ProviderPlanDerivation, TypedTrees, derive_satisfies_plans,
    select_derived_provider_plans, selected_provider_plan_facts,
    selected_provider_plan_facts_with_independent_components,
};
use crate::{CompositionMode, SelectedProviderPlanWithProvenance};
use component_description::{
    AdmissionProfile, COMPONENT_DESCRIPTION_SCHEMA_V2, ComponentDescriptionFacts,
    ComponentVerificationRequest, VerifiedComponent, describe_component_facts,
    encode_component_description, verify_component,
};
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::ProviderPlan;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, ServiceId,
    StructuralTypeId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, InstallationReachDependency,
    MachineContract, Operation, OperationKind, OperationResult, ProviderCandidateConformance,
    ProviderRefinement, ProviderSignature, ServiceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    VocabularyMarker,
};

/// The consumer side of one independent selection: the typed program, its
/// evaluated bindings, and the one checked provider plan selected with
/// `CompositionMode::Independent` by the build.
struct IndependentSelection {
    typed: TypedTrees,
    evaluated_bindings: crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    selected: Vec<SelectedProviderPlanWithProvenance>,
}

impl IndependentSelection {
    fn plan(&self) -> &ProviderPlan {
        &self.selected[0].derived.plan
    }

    fn realization(&self) -> (String, String, String) {
        let plan = self.plan();
        let ProviderBinding::CheckedAdapter {
            machine_identity, ..
        } = &plan.rows[0].binding
        else {
            panic!("the fixture selects a checked adapter")
        };
        (
            plan.rows[0].requirement_identity.clone(),
            plan.provider_type.clone(),
            machine_identity.clone(),
        )
    }

    fn close(
        &self,
        components: &[VerifiedComponent],
    ) -> Result<SelectedProviderPlanFacts, Vec<diagnostics::Diagnostic>> {
        selected_provider_plan_facts_with_independent_components(
            &self.typed,
            &self.evaluated_bindings,
            self.selected.clone(),
            components,
        )
        .map(|(facts, _)| facts)
    }
}

fn independent_selection(mode: CompositionMode) -> IndependentSelection {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        pub data InterruptAcknowledgement [copy] { token: u64; }
        pub domain InterruptAcknowledgement::Pending;
        pub data LapicCompletion {}

        pub boundary requirement InterruptAcknowledgement::complete(self)
        reaches <= MachineControl + PortIo
        requires self in InterruptAcknowledgement::Pending;

        machine LapicCompletion::complete(
            acknowledgement: InterruptAcknowledgement in Pending
        )
        satisfies InterruptAcknowledgement::complete
        reaches MachineControl
        {
        }
    "#;
    // The fence replays the selecting machine's authored span, so the fixture
    // is parsed under a registered source rather than the source-free route.
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("independent_selection.omg"),
            source.to_owned(),
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize independent selection fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse independent selection fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("resolve independent selection fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type independent selection fixture");
    let evaluated_bindings =
        crate::evaluated_via_bindings::evaluate_via_bindings(&typed, None, None)
            .expect("the checked fixture has an exact empty evaluated-via table");
    let requirement = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("typed top-level requirement");
    let provider = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "LapicCompletion::complete")
        .expect("typed checked provider");
    let provider_type = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "LapicCompletion")
        .expect("nominal provider type");
    let derived = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None));
    assert_eq!(derived.len(), 1, "one exact checked provider plan");
    let selection = crate::ProviderSelection {
        subject: crate::ProviderSelectionSubject::BoundaryRequirement(
            crate::ProviderSelectionIdentity {
                symbol: requirement.symbol,
                package: typed.symbols.symbol_package_identity(requirement.symbol),
                canonical_path: derived[0].plan.schema.trait_name.clone(),
                authored_path: "InterruptAcknowledgement::complete".to_owned(),
            },
        ),
        provider_type: crate::ProviderSelectionIdentity {
            symbol: provider_type.symbol,
            package: typed.symbols.symbol_package_identity(provider_type.symbol),
            canonical_path: derived[0].plan.provider_type.clone(),
            authored_path: "LapicCompletion".to_owned(),
        },
        composition_mode: mode,
        selecting_machine: provider.symbol,
        source_span: typed
            .symbols
            .symbol_provenance_source_span(provider.symbol)
            .expect("the selecting machine retains its authored span"),
    };
    let selected =
        select_derived_provider_plans(&derived, target::NativeTarget::host(), &[], &[selection])
            .expect("the build override selects the declared checked candidate");
    assert_eq!(selected.len(), 1);
    IndependentSelection {
        typed,
        evaluated_bindings,
        selected,
    }
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine identity")
}

/// A canonical Terminal module with one Unit entry machine and nothing else.
fn bare_module() -> TerminalModule {
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
                id: ContractId::new(1).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

/// A provider component module: it declares the boundary requirement and
/// retains one checked candidate machine (attached to the provider type)
/// realizing it with the given coordinates.
fn provider_module(
    requirement_identity: &str,
    provider_identity: &str,
    candidate_identity: &str,
) -> TerminalModule {
    let mut module = bare_module();
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(1).expect("boundary identity"),
        identity: requirement_identity.to_owned(),
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
    let provider_type = StructuralTypeId::new(1).expect("structural type identity");
    module.structural_types.push(StructuralTypeDeclaration {
        id: provider_type,
        identity: provider_identity.to_owned(),
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
            requirement_identity: requirement_identity.to_owned(),
            provider_identity: provider_identity.to_owned(),
            candidate_identity: candidate_identity.to_owned(),
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

/// A provider component whose root entry calls an installation-bound
/// boundary (`reaches <= Bound`). The verified module retains the declared
/// dependency row instead of a resolved service row, so the description
/// publishes the dependency's conservative bound as a service-bound roster
/// entry beside the (here empty) concrete reach's ceiling rows.
fn bounded_provider_module(
    requirement_identity: &str,
    provider_identity: &str,
    candidate_identity: &str,
) -> TerminalModule {
    let mut module = provider_module(requirement_identity, provider_identity, candidate_identity);
    let bound_service = ServiceId::new(1).expect("service identity");
    module.services.push(ServiceDeclaration {
        id: bound_service,
        identity: "PortIo".to_owned(),
        parents: Vec::new(),
    });
    let bounded_boundary = BoundaryMachineId::new(2).expect("boundary identity");
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: bounded_boundary,
        identity: "MachineControl::mask".to_owned(),
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
        id: OperationId::new(1).expect("operation identity"),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: bounded_boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    // A caller's published ceiling conservatively covers the dependency's
    // bound; `installation_dependencies` keeps the unresolved row distinct.
    module.machines[0].published_service_ceiling = vec![bound_service];
    module
        .root_service_reach
        .installation_dependencies
        .push(InstallationReachDependency {
            requirement_identity: "MachineControl::mask".to_owned(),
            upper_bound: vec![bound_service],
        });
    module
}

/// Describe and independently verify one component module: the same
/// producer/consumer pair a build route would run, so the fence only ever
/// sees replayed evidence.
fn verified_component(module: &TerminalModule) -> VerifiedComponent {
    let proof = terminal_psi::ProofBundle::default();
    let record = terminal_codec::build_identity_optimization_execution_record(module, &proof)
        .expect("identity optimization record");
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, &proof, &record, None)
            .expect("canonical artifact");
    let selected = SelectedProviderPlanFacts::from_selected_plans(Vec::new())
        .expect("a provider component seals no requirement of its own");
    let description = describe_component_facts(ComponentDescriptionFacts {
        artifact: &artifact,
        selected_provider_plans: &selected,
        component_progress: None,
        stack_demand: None,
        realization_identity: None,
    })
    .expect("component description");
    let request = ComponentVerificationRequest {
        expected_subject: terminal_codec::terminal_psi_identity(module).expect("module identity"),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V2]),
        accepted_assumptions: BTreeSet::new(),
        admission_profile: AdmissionProfile::default(),
    };
    verify_component(&encode_component_description(&description), &request)
        .expect("the provider component verifies")
}

fn message_texts(diagnostics: &[diagnostics::Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect()
}

#[test]
fn independent_selection_closes_against_exactly_one_verified_component() {
    let selection = independent_selection(CompositionMode::Independent);
    let (requirement, provider, machine) = selection.realization();
    let component = verified_component(&provider_module(&requirement, &provider, &machine));

    let facts = selection
        .close(std::slice::from_ref(&component))
        .expect("one verified component realizing the plan closes the independent edge");
    assert_eq!(facts.plans(), std::slice::from_ref(selection.plan()));
}

#[test]
fn independent_selection_rejects_a_component_exporting_an_unresolved_row() {
    let selection = independent_selection(CompositionMode::Independent);
    let (requirement, provider, machine) = selection.realization();
    let component = verified_component(&bounded_provider_module(&requirement, &provider, &machine));

    let rejected = selection
        .close(std::slice::from_ref(&component))
        .expect_err("a component retaining an unresolved installation-bound row is a bound installation still owes, not a resolved realization");
    let messages = message_texts(&rejected);
    assert!(
        messages.iter().any(|message| {
            message.contains("unresolved installation-bound requirement row")
                && message.contains("MachineControl::mask")
                && message.contains("refusing to treat the edge as fused")
        }),
        "unexpected diagnostics: {messages:#?}"
    );

    // The same module without the retained dependency still closes: the
    // rejection reads the verified module's rows, not row equality.
    let component = verified_component(&provider_module(&requirement, &provider, &machine));
    selection
        .close(std::slice::from_ref(&component))
        .expect("an independent edge whose component retains no unresolved row still closes");
}

#[test]
fn independent_selection_rejects_without_any_verified_component() {
    let selection = independent_selection(CompositionMode::Independent);

    let rejected = selection
        .close(&[])
        .expect_err("an independent edge without a verified component never closes");
    let messages = message_texts(&rejected);
    assert!(
        messages.iter().any(|message| {
            message.contains("retains independent composition")
                && message.contains("no verified component description was supplied")
                && message.contains("refusing to treat the edge as fused")
        }),
        "unexpected diagnostics: {messages:#?}"
    );

    // The route without a component input is the same fence with nothing
    // supplied, so existing callers keep rejecting every independent edge.
    let without_input = selected_provider_plan_facts(
        &selection.typed,
        &selection.evaluated_bindings,
        selection.selected.clone(),
    )
    .expect_err("the component-free route rejects independent composition");
    assert_eq!(message_texts(&without_input), messages);
}

#[test]
fn independent_selection_rejects_duplicate_realizations() {
    let selection = independent_selection(CompositionMode::Independent);
    let (requirement, provider, machine) = selection.realization();
    let module = provider_module(&requirement, &provider, &machine);
    let components = [verified_component(&module), verified_component(&module)];

    let rejected = selection
        .close(&components)
        .expect_err("two components realizing one plan do not name one deployment");
    let messages = message_texts(&rejected);
    assert!(
        messages.iter().any(|message| {
            message.contains("2 verified components realize it")
                && message.contains("exactly one component may deploy an independent edge")
        }),
        "unexpected diagnostics: {messages:#?}"
    );
}

#[test]
fn independent_selection_rejects_an_unmatched_extra_component() {
    let selection = independent_selection(CompositionMode::Independent);
    let (requirement, provider, machine) = selection.realization();
    let realizing = verified_component(&provider_module(&requirement, &provider, &machine));
    let extra = verified_component(&bare_module());

    let rejected = selection
        .close(&[realizing, extra])
        .expect_err("a supplied component that realizes no selected plan is not evidence");
    let messages = message_texts(&rejected);
    assert_eq!(
        messages.len(),
        1,
        "only the extra component rejects: {messages:#?}"
    );
    assert!(
        messages[0].contains("realizes no independently selected provider plan"),
        "unexpected diagnostics: {messages:#?}"
    );

    // A fused selection consumes no component either: supplying one is the
    // same unmatched extra.
    let fused = independent_selection(CompositionMode::Fused);
    let rejected = fused
        .close(&[verified_component(&provider_module(
            &requirement,
            &provider,
            &machine,
        ))])
        .expect_err("a fused edge leaves the supplied component unmatched");
    assert!(
        message_texts(&rejected)
            .iter()
            .all(|message| message.contains("realizes no independently selected provider plan")),
        "unexpected diagnostics: {rejected:#?}"
    );
    fused
        .close(&[])
        .expect("a fused selection closes without components");
}

#[test]
fn independent_selection_reports_each_realization_mismatch_distinctly() {
    let selection = independent_selection(CompositionMode::Independent);
    let (requirement, provider, machine) = selection.realization();

    let cases: [(&str, TerminalModule, &str); 3] = [
        (
            "other provider type",
            provider_module(&requirement, "OtherProvider", &machine),
            "is not realized by selected provider",
        ),
        (
            "other machine",
            provider_module(&requirement, &provider, "LapicCompletion::other"),
            "is not realized by selected machine",
        ),
        (
            "no realization",
            bare_module(),
            "verified component realizes no candidate for",
        ),
    ];
    for (case, module, expected) in cases {
        let component = verified_component(&module);
        let rejected = selection
            .close(std::slice::from_ref(&component))
            .err()
            .unwrap_or_else(|| panic!("{case}: a mismatched component must not close the edge"));
        let messages = message_texts(&rejected);
        assert!(
            messages.iter().any(|message| {
                message.contains("no verified component realizes it")
                    && message.contains(expected)
                    && message.contains("refusing to treat the edge as fused")
            }),
            "{case}: unexpected diagnostics: {messages:#?}"
        );
    }
}
