use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_codec::{decode_module, encode_module};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalStructuralValue, admit_provider_installation_from_artifact,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
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

fn checked_closed_operator_candidates() -> checked_trees::CheckedTrees {
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
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("two checked closed operator applications")
}

#[test]
fn closed_token_candidates_are_independent_of_provider_selection() {
    let typed = crate::front_end::typed_program(
        r#"
        data Buffer<Element, const Count: u64> { value: i64; }
        data Indexing {}
        boundary machine [] Indexing::index<Element, const Count: u64>(
            items: Buffer<Element, Count>, offset: i32
        ) -> Buffer<Element, Count> requires Count == Count;
        data FirstProvider {}
        machine FirstProvider::index<Value, const Length: u64>(
            items: Buffer<Value, Length>, offset: i32
        ) -> Buffer<Value, Length> satisfies Indexing::index { items }
        data SecondProvider {}
        machine SecondProvider::index<Value, const Length: u64>(
            items: Buffer<Value, Length>, offset: i32
        ) -> Buffer<Value, Length> satisfies Indexing::index { items }
        // An equally shaped machine without a satisfaction edge is not a candidate.
        data Unrelated {}
        machine Unrelated::index<Value, const Length: u64>(
            items: Buffer<Value, Length>, offset: i32
        ) -> Buffer<Value, Length> { items }
        machine run(four: Buffer<i32, 4>, eight: Buffer<i32, 8>) {
            let first: Buffer<i32, 4> = four[(0 as i32)];
            let second: Buffer<i32, 8> = eight[(0 as i32)];
        }
    "#,
    );
    let mut baseline = None;
    for selected_name in [
        None,
        Some("FirstProvider::index"),
        Some("SecondProvider::index"),
    ] {
        let selections = selected_name
            .into_iter()
            .map(|name| {
                typed_trees_to_checked_trees::SelectedGenericOperatorProviderSpecialization {
                    requirement_operator: typed.machine_token_bindings()[0].symbol,
                    realization_machine: typed
                        .machines()
                        .iter()
                        .find(|machine| machine.name.as_str() == name)
                        .unwrap()
                        .symbol,
                }
            })
            .collect::<Vec<_>>();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed.clone(),
            &typed_trees_to_checked_trees::CheckingRequest::settled()
                .with_selected_generic_operator_providers(&selections),
        )
        .expect("all checked candidates materialize from the same closed demands");
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("run"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("selection-neutral artifact")
        .into_artifact();
        drop(checked);
        let module = decode_module(artifact.semantic_bytes()).unwrap();
        assert_eq!(
            module.provider_candidates.len(),
            4,
            "two providers for each tuple"
        );
        assert!(module.provider_candidates.iter().all(|candidate| {
            ["FirstProvider", "SecondProvider"].contains(&candidate.provider_identity.as_str())
        }));
        for provider in ["FirstProvider", "SecondProvider"] {
            let selections = module
                .provider_candidates
                .iter()
                .filter(|candidate| candidate.provider_identity == provider)
                .map(|candidate| ProviderInstallationSelection {
                    boundary: candidate.boundary,
                    provider_identity: candidate.provider_identity.clone(),
                    candidate: candidate.candidate,
                })
                .collect::<Vec<_>>();
            assert_eq!(selections.len(), 2);
            let installation = admit_provider_installation_from_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &selections,
            )
            .expect("either complete provider installs from the identical source-free artifact");
            let entry = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            let arguments = entry
                .structural_parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| TerminalStructuralValue {
                    opaque_identity: position as u64 + 1,
                    structural_type: parameter.structural_type,
                    qualifications: parameter.qualifications.clone(),
                    path: Vec::new(),
                })
                .collect::<Vec<_>>();
            let mut execution = TerminalExecution::start_installed_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                TerminalStructuralInputs {
                    arguments: &arguments,
                    ..Default::default()
                },
                &installation,
            )
            .expect("execute both closed applications after source custody is dropped");
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
                execution.effects().is_empty(),
                "both calls execute installed checked bodies"
            );
        }
        let bytes = (
            artifact.semantic_bytes().to_vec(),
            artifact.proof_bytes().to_vec(),
        );
        if let Some(expected) = &baseline {
            assert_eq!(&bytes, expected, "selection must not change Psi output");
        } else {
            baseline = Some(bytes);
        }
    }
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
