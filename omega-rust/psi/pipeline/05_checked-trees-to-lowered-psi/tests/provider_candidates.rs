use checked_trees_to_lowered_psi::TerminalMachineSelection;
use lowered_psi_to_terminal_psi::terminal_production::{
    TerminalProductionCustody, TerminalProductionTimings,
};
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalEffect, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalStructuralValue, admit_provider_installation_from_artifact,
};
use terminal_psi::OperationKind;

const SOURCE: &str = r#"
    boundary trait Signal {
        machine emit();
    }

    data FirstProvider {}
    machine FirstProvider::emit() satisfies Signal::emit {}

    data SecondProvider {}
    machine SecondProvider::emit() satisfies Signal::emit {}

    data Root {}
    machine Root::enter()
    reaches Signal
    {
        Signal::emit();
    }
"#;

fn checked_closed_operator_candidates() -> typed_trees_to_checked_trees::checked_trees::CheckedTrees
{
    let typed = crate::front_end::typed_program(
        r#"
        data Buffer<Element, const Count: u64> { value: i64; }
        data Indexing {}
        boundary machine [] Indexing::index<Element, const Count: u64>(
            items: Buffer<Element, Count>, offset: i32
        ) -> Buffer<Element, Count> requires Count == Count;
        data Provider {}
        machine Provider::index<Value, const Length: u64>(
            items: Buffer<Value, Length>, offset: i32
        ) -> Buffer<Value, Length> satisfies Indexing::index { items }
        machine run(four: Buffer<i32, 4>, eight: Buffer<i32, 8>) {
            let first: Buffer<i32, 4> = four[(0 as i32)];
            let second: Buffer<i32, 8> = eight[(0 as i32)];
        }
    "#,
    );
    // This tests catalog publication from checked applications, not automatic
    // selection-neutral specialization or native provider-plan installation.
    let requests = [
        typed_trees_to_checked_trees::SelectedGenericOperatorProviderSpecialization {
            requirement_operator: typed.machine_token_bindings()[0].symbol,
            realization_machine: typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Provider::index")
                .expect("provider template")
                .symbol,
        },
    ];
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled()
            .with_selected_generic_operator_providers(&requests),
    )
    .expect("two checked closed operator applications")
}

#[test]
fn closed_operator_candidates_rejoin_distinct_requirement_applications() {
    let checked = checked_closed_operator_candidates();
    let artifact =
        lowered_psi_to_terminal_psi::terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("run"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("closed candidate catalog publishes")
        .into_artifact();
    let module = decode_module(artifact.semantic_bytes()).expect("source-free module");
    let proof =
        terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("source-free proof");
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("closed provider signatures and bodies independently verify");
    let [first, second] = module.provider_candidates.as_slice() else {
        panic!("one exact candidate per closed requirement");
    };
    assert_ne!(first.boundary, second.boundary);
    assert_ne!(first.requirement_identity, second.requirement_identity);
    assert_ne!(first.candidate, second.candidate);
    assert_ne!(first.candidate_identity, second.candidate_identity);
    for candidate in &module.provider_candidates {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| {
                checked
                    .normalized_machine_overload_identity(machine)
                    .is_some_and(|identity| identity.identity() == candidate.candidate_identity)
            })
            .expect("exact provider instance");
        let symbols = typed_trees_to_checked_trees::validation::TopLevelSymbols::build(
            &checked.typed,
            &mut Vec::new(),
        );
        let conformance = &checked.machine_trait_conformances(machine)[0];
        let template = checked
            .machines()
            .iter()
            .find(|requirement| requirement.symbol == conformance.symbol)
            .expect("authored requirement");
        let requirement =
            typed_trees_to_checked_trees::validation::resolve_closed_requirement_application(
                &checked.typed,
                machine,
                template,
                &symbols,
            )
            .expect("exact checked application");
        assert_eq!(
            candidate.requirement_identity,
            checked
                .normalized_machine_overload_identity(requirement)
                .unwrap()
                .identity()
        );
    }
    let selections = module
        .provider_candidates
        .iter()
        .map(|candidate| ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        })
        .collect::<Vec<_>>();
    drop(checked);
    admit_provider_installation_from_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &selections,
    )
    .expect("both exact closed candidates install without source custody");
}

#[test]
fn closed_operator_catalog_rejects_substituted_or_missing_application_custody() {
    let baseline = checked_closed_operator_candidates();
    let provider = baseline
        .machine_specializations
        .iter()
        .position(|receipt| !receipt.operator_realizations.is_empty())
        .expect("provider specialization");
    let provider_receipt = &baseline.machine_specializations[provider];
    let requirement = baseline
        .machine_specializations
        .iter()
        .position(|receipt| {
            receipt.template == provider_receipt.operator_realizations[0].requirement_symbol
                && receipt.const_argument_identities == provider_receipt.const_argument_identities
        })
        .expect("matching requirement specialization");
    for receipt_index in [provider, requirement] {
        for mutation in 0..5 {
            let mut checked = baseline.clone();
            match mutation {
                0 => {
                    checked.typed.machine_specializations.remove(receipt_index);
                }
                1 => {
                    let duplicate = checked.typed.machine_specializations[receipt_index].clone();
                    checked.typed.machine_specializations.push(duplicate);
                }
                2 => {
                    checked.typed.machine_specializations[receipt_index]
                        .normalized_template_identity
                        .push_str("substituted");
                }
                3 => {
                    checked.typed.machine_specializations[receipt_index].commitment =
                        Default::default();
                }
                _ => {
                    let other = baseline
                        .machine_specializations
                        .iter()
                        .find(|receipt| {
                            receipt.template
                                == baseline.machine_specializations[receipt_index].template
                                && receipt.instance
                                    != baseline.machine_specializations[receipt_index].instance
                        })
                        .expect("other closed tuple of the same template");
                    checked.typed.machine_specializations[receipt_index]
                        .const_argument_identities = other.const_argument_identities.clone();
                }
            }
            let lowered = checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("run"),
            );
            assert!(
                lowered.is_err(),
                "receipt {receipt_index}, mutation {mutation} cannot publish a candidate: {lowered:?}"
            );
        }
    }
}

const STRUCTURAL_PROVIDER_SOURCE: &str = r#"
    pub data Extent [linear] {
        base: addr;
        length: u64;
    }

    pub boundary machine no_wrap(base: addr, length: u64) -> bool;

    pub domain Extent::Granted
    requires
        no_wrap(self.base, self.length)
    established by
        ProgramEntry::enter;

    pub boundary trait ProgramEntry {
        machine enter(extent: Extent in Granted);
    }

    boundary machine Extent::settle(self)
    requires
        self in Extent::Granted
    ensures true;

    data ProgramProvider {}
    machine ProgramProvider::enter(extent: Extent in Granted)
        satisfies ProgramEntry::enter
    {
        extent.settle();
    }

    data Root {}
    machine Root::enter(extent: Extent in Granted)
    reaches ProgramEntry invokes ProgramEntry;
    {
        ProgramEntry::enter(extent);
    }
"#;

const PROGRAM_STORAGE_PROVIDER_SOURCE: &str = r#"
    pub data Extent [linear] {
        base: addr;
        length: u64;
    }

    pub boundary machine no_wrap(base: addr, length: u64) -> bool;

    pub domain Extent::Granted
    requires
        no_wrap(self.base, self.length)
    established by
        ProgramStorageEntry::enter;

    pub boundary trait ProgramStorageEntry {
        machine enter(
            image: Extent in Granted,
            initial_storage: Extent in Granted
        );
    }

    boundary machine Extent::settle(self)
    requires
        self in Extent::Granted
    ensures true;

    data ProgramStorageProvider {}
    machine ProgramStorageProvider::enter(
        image: Extent in Granted,
        initial_storage: Extent in Granted
    )
    satisfies ProgramStorageEntry::enter
    {
        image.settle();
        initial_storage.settle();
    }

    data ProgramLocalProducer {}
    machine ProgramLocalProducer::handoff(
        image: Extent in Granted,
        initial_storage: Extent in Granted
    )
    reaches ProgramStorageEntry invokes ProgramStorageEntry;
    {
        ProgramStorageEntry::enter(image, initial_storage);
    }
"#;

#[test]
fn invalid_unit_provider_plan_names_the_exact_candidate() {
    let baseline = crate::front_end::checked_program(SOURCE);
    for candidate in ["FirstProvider::emit", "SecondProvider::emit"] {
        let symbol = baseline
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == candidate)
            .expect("authored provider candidate")
            .symbol;
        for duplicate in [false, true] {
            let mut checked = baseline.clone();
            let plans = &mut checked.facts.flow.terminal_unit_effects;
            let reason = if duplicate {
                let repeated = plans.for_machine(symbol).expect("candidate plan").clone();
                plans.machines.push(repeated);
                "attached Unit closure contains duplicate checked machine plans"
            } else {
                plans.machines.retain(|plan| plan.machine != symbol);
                "attached Unit closure is missing a checked transitive machine plan"
            };
            assert!(matches!(
                checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("Root::enter")),
                Err(checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
                    machine, reason: actual_reason, ..
                }) if machine == candidate && actual_reason == reason
            ));
        }
    }
}

#[test]
fn checked_unit_provider_candidates_are_cataloged_without_selection_or_call_rewrite() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("zero-argument Unit provider catalog lowers");

    let module = &lowered.semantic_module;
    assert_eq!(module.boundary_machines.len(), 1);
    assert_eq!(module.provider_candidates.len(), 2);
    assert_eq!(module.machines.len(), 3, "root plus both adapter bodies");
    let boundary = module.boundary_machines[0].id;
    let requirement_identity = &module.provider_candidates[0].requirement_identity;
    assert!(!requirement_identity.is_empty());
    assert!(module.provider_candidates.iter().all(|candidate| {
        candidate.boundary == boundary
            && candidate.requirement_identity == *requirement_identity
            && candidate.signature.parameters.is_empty()
            && candidate.refinement.positional_parameters.is_empty()
            && candidate.refinement.required_domains.is_empty()
            && module
                .machines
                .iter()
                .any(|machine| machine.id == candidate.candidate)
    }));
    assert_eq!(
        module
            .provider_candidates
            .iter()
            .map(|candidate| candidate.provider_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["FirstProvider", "SecondProvider"]
    );
    let expected_identities = ["FirstProvider::emit", "SecondProvider::emit"].map(|name| {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("the exact source provider declaration");
        checked
            .typed
            .normalized_machine_overload_identity(machine)
            .expect("source provider has a normalized overload identity")
            .identity()
    });
    assert_eq!(
        module
            .provider_candidates
            .iter()
            .map(|candidate| candidate.candidate_identity.as_str())
            .collect::<Vec<_>>(),
        expected_identities
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    assert!(matches!(
        entry.blocks[0].operations[0].kind,
        OperationKind::BoundaryCall { boundary: called, .. } if called == boundary
    ));

    let encoded = encode_module(module).expect("encode catalog");
    let decoded = decode_module(&encoded).expect("decode and independently verify catalog shape");
    assert_eq!(decoded, *module);
}

#[test]
fn checked_unit_provider_candidates_retain_linear_qualified_structural_inputs() {
    let checked = crate::front_end::checked_program(STRUCTURAL_PROVIDER_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("structural Unit provider catalog lowers");

    let module = &lowered.semantic_module;
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one exact ProgramEntry provider candidate")
    };
    assert_eq!(candidate.provider_identity, "ProgramProvider");
    let provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ProgramProvider::enter")
        .expect("the exact structural-input provider declaration");
    let expected_identity = checked
        .typed
        .normalized_machine_overload_identity(provider)
        .expect("source provider has a normalized overload identity")
        .identity();
    assert_eq!(candidate.candidate_identity, expected_identity);
    let [signature] = candidate.signature.parameters.as_slice() else {
        panic!("provider signature retains one structural root")
    };
    assert_eq!(signature.position, 0);
    assert!(!signature.is_self);
    assert_eq!(
        signature.multiplicity,
        terminal_psi::StructuralMultiplicity::Linear
    );
    assert_eq!(signature.access, terminal_psi::StructuralAccess::Owned);
    let [qualification] = signature.qualifications.as_slice() else {
        panic!("provider signature retains the Granted qualification")
    };
    assert_eq!(
        candidate.refinement.positional_parameters,
        [terminal_psi::ProviderParameterRefinement {
            boundary_index: 0,
            candidate_index: 0,
        }]
    );
    assert_eq!(candidate.refinement.required_domains.len(), 1);
    assert_eq!(candidate.refinement.required_domains[0].argument_index, 0);
    assert_eq!(
        candidate.refinement.required_domains[0].domain,
        *qualification
    );

    let provider = module
        .machines
        .iter()
        .find(|machine| machine.id == candidate.candidate)
        .expect("provider body remains an ordinary Terminal machine");
    assert_eq!(provider.structural_parameters.len(), 1);
    assert_eq!(provider.entry_claims.len(), 1);
    assert!(matches!(
        provider.blocks[0].operations[0].kind,
        OperationKind::BoundaryCall {
            ref completion_receipts,
            ..
        } if completion_receipts.len() == 1
    ));

    let encoded = encode_module(module).expect("encode structural provider catalog");
    assert_eq!(
        decode_module(&encoded).expect("decode structural provider catalog"),
        *module
    );

    let mut access_tamper = module.clone();
    let candidate_machine = access_tamper.provider_candidates[0].candidate;
    access_tamper
        .machines
        .iter_mut()
        .find(|machine| machine.id == candidate_machine)
        .expect("candidate machine")
        .structural_parameters[0]
        .access = terminal_psi::StructuralAccess::SharedBorrow;
    assert!(matches!(
        terminal_verifier::validate_module(&access_tamper),
        Err(terminal_verifier::ModuleError::InvalidProviderCandidate { .. })
    ));
}

#[test]
fn installed_structural_provider_receives_and_settles_the_exact_linear_claim() {
    let checked = crate::front_end::checked_program(STRUCTURAL_PROVIDER_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("structural provider artifact lowers");
    let [candidate] = lowered.semantic_module.provider_candidates.as_slice() else {
        panic!("one exact structural provider candidate")
    };
    let [signature] = candidate.signature.parameters.as_slice() else {
        panic!("one exact structural provider parameter")
    };
    let [qualification] = signature.qualifications.as_slice() else {
        panic!("one exact structural provider qualification")
    };
    let semantic = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof artifact");
    let installation = admit_provider_installation_from_artifact(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        }],
    )
    .expect("exact structural provider installation is admitted");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xfeed,
        structural_type: signature.structural_type,
        qualifications: vec![*qualification],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_installed_artifact(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
        &installation,
    )
    .expect("structural provider execution starts");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::default(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(
        matches!(execution.effects(), [TerminalEffect::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    }] if *boundary != candidate.boundary
        && structural_arguments == &[argument]
        && completion_receipts.len() == 1)
    );
}

#[test]
fn installed_program_storage_provider_transfers_and_settles_both_owned_extent_claims() {
    let checked = crate::front_end::checked_program(PROGRAM_STORAGE_PROVIDER_SOURCE);
    let produced =
        lowered_psi_to_terminal_psi::terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("ProgramLocalProducer::handoff"),
        )
        .produce(TerminalProductionCustody {
            retain_unoptimized: false,
            entry_identity: Some([0xa5; 32]),
            callback_custody: (),
            timings: &mut TerminalProductionTimings::default(),
        })
        .expect("receipt-coupled ProgramStorage artifact");
    let module = decode_module(produced.artifact().semantic_bytes()).expect("semantic module");
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one exact ProgramStorage provider candidate")
    };
    let [image, initial_storage] = candidate.signature.parameters.as_slice() else {
        panic!("the exact two-Extent provider signature")
    };
    let ([image_domain], [storage_domain]) = (
        image.qualifications.as_slice(),
        initial_storage.qualifications.as_slice(),
    ) else {
        panic!("both Extent roots retain Granted")
    };
    assert_eq!(image_domain, storage_domain);
    assert_eq!(image.access, terminal_psi::StructuralAccess::Owned);
    assert_eq!(
        initial_storage.access,
        terminal_psi::StructuralAccess::Owned
    );
    let installation = admit_provider_installation_from_artifact(
        produced.artifact().semantic_bytes(),
        produced.artifact().proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        }],
    )
    .expect("exact ProgramStorage provider installation is admitted");
    let arguments = [
        TerminalStructuralValue {
            opaque_identity: 0x1a6e,
            structural_type: image.structural_type,
            qualifications: vec![*image_domain],
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 0x570a,
            structural_type: initial_storage.structural_type,
            qualifications: vec![*storage_domain],
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_installed_artifact(
        produced.artifact().semantic_bytes(),
        produced.artifact().proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
        &installation,
    )
    .expect("ProgramStorage provider execution starts");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::default(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(matches!(execution.effects(), [
        TerminalEffect::BoundaryCall { structural_arguments: image, completion_receipts: image_receipts, .. },
        TerminalEffect::BoundaryCall { structural_arguments: storage, completion_receipts: storage_receipts, .. },
    ] if image == &arguments[..1]
        && storage == &arguments[1..]
        && image_receipts.len() == 1
        && storage_receipts.len() == 1));
}
