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

const ARRAY_READER_SOURCE: &str = r#"
        boundary trait ArrayReader {
            machine inspect(&mut self, prefix: i32, values: [u8; 4], suffix: i32) -> i32;
        }
        data ArrayReaderProvider {}
        machine ArrayReaderProvider::inspect(&mut self, prefix: i32, values: [u8; 4], suffix: i32) -> i32
        satisfies ArrayReader::inspect {
            transition prefix == 30 && suffix == 40 && values[0] == 11 {
                true -> (70)
                _ -> (71)
            }
        }
        data Root<'a> { reader: &'a mut ArrayReader; values: [u8; 4]; result: i32; }
        machine Root::run(&mut self) reaches ArrayReader {
            self.result = self.reader.inspect(30, self.values, 40);
        }
        "#;

#[test]
fn boundary_array_parameter_forwarding_rejoins_the_authored_source() {
    let trait_source = ARRAY_READER_SOURCE
        .replace(
            "machine Root::run(&mut self) reaches ArrayReader {",
            "machine Root::run(context: &mut Root, first: [u8; 4], second: [u8; 4]) reaches ArrayReader {",
        )
        .replace("inspect(30, self.values, 40)", "inspect(30, second, 40)")
        .replace("inspect(&mut self, ", "inspect(")
        .replace("self.result", "context.result")
        .replace("self.reader.inspect", "ArrayReader::inspect")
        .replace(
            "data Root<'a> { reader: &'a mut ArrayReader; values: [u8; 4]; result: i32; }",
            "data Root { values: [u8; 4]; result: i32; }",
        );
    let direct_source = r#"
        pub data ArrayReader {}
        pub boundary requirement ArrayReader::inspect(prefix: i32, values: [u8; 4], suffix: i32) -> i32;
        data Root { values: [u8; 4]; result: i32; }
        machine Root::run(context: &mut Root, first: [u8; 4], second: [u8; 4]) {
            context.result = ArrayReader::inspect(30, second, 40);
        }
    "#;
    for (declaration, source) in [("trait", trait_source.as_str()), ("direct", direct_source)] {
        let checked = crate::front_end::checked_program(source);
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Root::run"),
        )
        .unwrap_or_else(|error| panic!("{declaration} array parameter custody: {error:?}"));
        let bytes = encode_module(&lowered.semantic_module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, lowered.semantic_module);
        terminal_verifier::verify_module(
            &decoded,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();

        let source_symbol = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::run")
            .unwrap()
            .symbol;
        for mutation in ["different parameter", "borrowed actual"] {
            let mut changed = checked.clone();
            let caller = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|machine| machine.machine == source_symbol)
                .unwrap();
            let arguments = caller
                .operations
                .iter_mut()
                .filter_map(checked_trees::CheckedUnitEffectOperationPlan::call_mut)
                .find(|call| call.is_boundary() && call.scalar_result().is_some())
                .map(|call| &mut call.structural_arguments)
                .expect("retained boundary invocation");
            if mutation == "different parameter" {
                arguments[0].source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 1,
                    };
            } else {
                arguments[0].access = checked_trees::CheckedStructuralAccess::SharedBorrow;
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("Root::run"),
                )
                .is_err(),
                "{declaration} boundary forwarding must reject {mutation}",
            );
        }
    }
}

#[test]
fn checked_array_provider_preserves_receiver_and_mixed_arguments() {
    check_array_provider_subject(ARRAY_READER_SOURCE, true);
}

#[test]
fn checked_array_provider_preserves_explicit_subject_and_mixed_arguments() {
    check_array_provider_subject(
        &ARRAY_READER_SOURCE.replace(
            "ArrayReaderProvider::inspect(&mut self,",
            "ArrayReaderProvider::inspect(provider: &mut ArrayReaderProvider,",
        ),
        false,
    );
}

fn check_array_provider_subject(source: &str, receiver_spelling: bool) {
    let checked = crate::front_end::checked_program(source);
    let provider = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ArrayReaderProvider::inspect")
        .expect("authored provider");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(provider.symbol)
        .expect("ordinary provider body retains receiver custody");
    assert_eq!(
        graph.states[0]
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self),
        receiver_spelling,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::run"),
    )
    .expect("checked provider receiver and scalar-array-scalar inputs lower together");
    let bytes = encode_module(&lowered.semantic_module).expect("provider conformance encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
    let module = &lowered.semantic_module;
    let conformance = &module.provider_candidates[0];
    let adapter = module
        .machines
        .iter()
        .find(|machine| machine.id == conformance.candidate)
        .unwrap();
    assert!(
        adapter
            .structural_parameters
            .iter()
            .all(|parameter| !parameter.is_self)
    );
    let callee = adapter
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::CallStructuralScalar { callee, .. } => Some(callee),
            _ => None,
        })
        .expect("installation entry calls the unchanged ordinary method");
    let ordinary = module
        .machines
        .iter()
        .find(|machine| machine.id == callee)
        .unwrap();
    assert_eq!(
        ordinary
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self),
        receiver_spelling,
    );
    assert_eq!(ordinary.attachment.is_some(), receiver_spelling);
    let mut redirected = module.clone();
    redirected.provider_candidates[0].candidate = callee;
    assert!(
        terminal_verifier::validate_module(&redirected).is_err(),
        "the ordinary receiver cannot be omitted by redirecting installation"
    );

    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Root::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("receiver adapter publishes a sealed artifact")
    .into_artifact();
    let mut published = decode_module(artifact.semantic_bytes()).unwrap();
    let candidate = &published.provider_candidates[0];
    let selection = [ProviderInstallationSelection {
        boundary: candidate.boundary,
        provider_identity: candidate.provider_identity.clone(),
        candidate: candidate.candidate,
    }];
    admit_provider_installation_from_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &selection,
    )
    .expect("source-free adapter installation");
    let adapter_id = candidate.candidate;
    let adapter = published
        .machines
        .iter_mut()
        .find(|machine| machine.id == adapter_id)
        .unwrap();
    let arguments = adapter
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::CallStructuralScalar { arguments, .. } => Some(arguments),
            _ => None,
        })
        .unwrap();
    arguments.swap(0, 1);
    terminal_verifier::validate_module(&published)
        .expect("same-typed scalar substitution is structurally valid");
    let changed = encode_module(&published).unwrap();
    assert!(
        admit_provider_installation_from_artifact(
            &changed,
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &selection
        )
        .is_err(),
        "unchanged nominal selection and proof cannot admit a different adapter body"
    );
}

#[test]
fn checked_provider_adapter_does_not_invent_stateful_receiver_storage() {
    check_stateful_provider_subject(ARRAY_READER_SOURCE);
    check_stateful_provider_subject(&ARRAY_READER_SOURCE.replace(
        "ArrayReaderProvider::inspect(&mut self,",
        "ArrayReaderProvider::inspect(provider: &mut ArrayReaderProvider,",
    ));
}

fn check_stateful_provider_subject(source: &str) {
    let checked = crate::front_end::checked_program(&source.replace(
        "data ArrayReaderProvider {}",
        "data ArrayReaderProvider { counter: i32; }",
    ));
    let error = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::run"),
    )
    .expect_err("a nonempty provider needs installation-owned storage even when unread");
    assert!(format!("{error:?}").contains("receiver requires installation storage"));
}

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
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
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
}

/// Two closed `[]` applications over distinct const tuples, where each
/// provider's checked body computes a different observable scalar from the
/// same operands: the first reads the structural operand's field, the second
/// folds in the scalar argument. Installing either provider must produce that
/// provider's result — an installed candidate whose body never ran is not
/// execution coverage.
const EXECUTED_OPERATOR_SOURCE: &str = r#"
    data Buffer<Element, const Count: u64> { value: i64; }
    data Indexing {}
    boundary machine [] Indexing::pick<Element, const Count: u64>(
        items: Buffer<Element, Count>, offset: i32
    ) -> i64 requires Count == Count;
    data FirstProvider {}
    machine FirstProvider::pick<Value, const Length: u64>(
        items: Buffer<Value, Length>, offset: i32
    ) -> i64 satisfies Indexing::pick {
        items.value
    }
    data SecondProvider {}
    machine SecondProvider::pick<Value, const Length: u64>(
        items: Buffer<Value, Length>, offset: i32
    ) -> i64 satisfies Indexing::pick {
        items.value ^ (offset as i64)
    }
    machine run(four: Buffer<i32, 4>, eight: Buffer<i32, 8>) -> i64 {
        let first: i64 = four[(7 as i32)];
        let second: i64 = eight[(11 as i32)];
        first ^ second
    }
"#;

fn run_artifact_with_provider(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    module: &terminal_psi::TerminalModule,
    provider: &str,
    field_values: &[(u32, i64)],
) -> terminal_interpreter::TerminalExecutionResult {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    use terminal_interpreter::{TerminalScalarValue, TerminalStructuralScalarFieldValue};
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
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
    let scalar_fields = field_values
        .iter()
        .map(|(argument_index, value)| {
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| {
                    declaration.id == arguments[*argument_index as usize].structural_type
                })
                .expect("argument structural type");
            let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
                panic!("Buffer is a record");
            };
            let field = fields
                .iter()
                .find(|field| field.identity == "value")
                .expect("value field");
            TerminalStructuralScalarFieldValue {
                argument_index: *argument_index,
                path: Vec::new(),
                field: field.id,
                value: TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                    value: IntegerValue::Signed(i128::from(*value)),
                },
            }
        })
        .collect::<Vec<_>>();
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
    let installation = admit_provider_installation_from_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &selections,
    )
    .expect("the named provider installs both closed tuples");
    let mut execution = TerminalExecution::start_installed_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            scalar_fields: &scalar_fields,
            ..Default::default()
        },
        &installation,
    )
    .expect("installed provider applications execute");
    let TerminalExecutionStatus::Complete(result) = execution
        .resume(
            &mut TerminalFuelMeter::default(),
            &mut AcceptTerminalEffects,
        )
        .expect("execution completes")
    else {
        panic!("execution must complete")
    };
    result
}

fn i64_value(value: terminal_interpreter::TerminalScalarValue) -> i128 {
    use semantic_vocabulary::IntegerValue;
    let terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type, value, ..
    } = value
    else {
        panic!("integer scalar")
    };
    assert_eq!(
        scalar_type,
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64)
            .unwrap()
    );
    match value {
        IntegerValue::Signed(raw) => raw,
        IntegerValue::Unsigned(raw) => raw as i128,
    }
}

/// The artifact closes the `[]` application without a checked-time provider
/// selection; after source custody drops, installing one provider must run
/// that provider's own checked body — distinct scalar results distinguish
/// executing the selected body from installing either candidate.
#[test]
fn closed_operator_applications_execute_the_installed_provider_body() {
    let checked = crate::front_end::checked_program(EXECUTED_OPERATOR_SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("mixed-input scalar-result applications produce one artifact")
    .into_artifact();
    drop(checked);
    let module = decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(
        module.provider_candidates.len(),
        4,
        "two closed const tuples, one candidate per provider each"
    );
    assert!(module.provider_candidates.iter().all(|candidate| {
        ["FirstProvider", "SecondProvider"].contains(&candidate.provider_identity.as_str())
    }));
    // FirstProvider reads the operand's field: 3 ^ 5 = 6.
    // SecondProvider folds in the scalar `offset`: (3 ^ 7) ^ (5 ^ 11) = 10.
    for (provider, expected) in [("FirstProvider", 6i128), ("SecondProvider", 10i128)] {
        let TerminalExecutionResult::Scalar(value) =
            run_artifact_with_provider(&artifact, &module, provider, &[(0, 3), (1, 5)])
        else {
            panic!("{provider}'s application must return the scalar result")
        };
        assert_eq!(
            i64_value(value),
            expected,
            "{provider}'s own body must execute"
        );
    }
}

/// A named operator uses the same requirement call and checked-provider
/// installation as a machine. Its result remains usable by ordinary arithmetic
/// after checked source custody is dropped.
#[test]
fn named_boundary_operator_executes_through_the_ordinary_requirement_route() {
    let checked = crate::front_end::checked_program(
        r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32
        requires value == value
        ensures result == value + 0 && value == value;
        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        { transition { _ -> (input + 0) } }
        machine run() -> i32 {
            let result: i32 = CheckedMath::offset_zero(70);
            result ^ 1
        }
        boundary trait Host { machine mark(value: u64) reaches Host; }
        data UnitRoot<'s> { host: &'s mut Host; }
        machine UnitRoot::unit_run(&mut self) reaches Host {
            self.host.mark(1);
            let result: i32 = CheckedMath::offset_zero(70);
            self.host.mark(2);
        }
    "#,
    );
    assert_eq!(
        checked.facts.operators.boundary_applications.len(),
        2,
        "the named operator application survives checking as an exact demand"
    );
    let attached = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("UnitRoot::unit_run"),
    )
    .expect("a Unit caller retains the operator between its other effects");
    let attached_bytes =
        encode_module(&attached.semantic_module).expect("mixed boundary kinds encode");
    let attached_module = decode_module(&attached_bytes).expect("mixed boundary kinds decode");
    assert_eq!(attached.semantic_module, attached_module);
    let mut forged = checked.clone();
    forged
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter_mut()
        .find(|declaration| declaration.kind == terminal_psi::BoundaryRequirementKind::Operator)
        .expect("checked operator declaration")
        .kind = terminal_psi::BoundaryRequirementKind::Trait;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &forged,
            TerminalMachineSelection::Name("UnitRoot::unit_run")
        )
        .is_err(),
        "a checked plan cannot substitute the authored requirement kind"
    );
    drop(forged);
    for kind in [
        terminal_psi::BoundaryRequirementKind::Trait,
        terminal_psi::BoundaryRequirementKind::Operator,
    ] {
        let mut substituted = attached_module.clone();
        let declaration = substituted
            .boundary_machines
            .iter_mut()
            .find(|declaration| declaration.kind == kind)
            .expect("both requirement kinds");
        declaration.kind = match kind {
            terminal_psi::BoundaryRequirementKind::Trait => {
                terminal_psi::BoundaryRequirementKind::Operator
            }
            _ => terminal_psi::BoundaryRequirementKind::Trait,
        };
        assert!(
            encode_module(&substituted).is_err(),
            "substituted role cannot remove a required provider root or invent an operator root"
        );
    }
    let produced = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("run"),
    )
    .produce(TerminalProductionCustody {
        retain_unoptimized: true,
        ..TerminalProductionCustody::artifact_only(&mut TerminalProductionTimings::default())
    })
    .expect("named requirement call produces an artifact");
    assert_eq!(
        produced.boundary_operator_scope().occurrences().len(),
        1,
        "the published call retains one exact application occurrence"
    );
    let lowered = produced.unoptimized().expect("retained source join");
    lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
        &checked,
        produced.artifact(),
        lowered,
    )
    .expect("independent replay joins the same source occurrence");
    for mutation in 0..4 {
        let mut substituted = lowered.clone();
        match mutation {
            0 => substituted.source_call_occurrences.clear(),
            1 => {
                let duplicate = substituted.source_call_occurrences[0].clone();
                substituted.source_call_occurrences.push(duplicate);
            }
            2 => {
                substituted.source_call_occurrences[0].source_target =
                    symbols::SymbolHandle::invalid()
            }
            _ => substituted.source_call_occurrences[0].call_ordinal += 1,
        }
        assert!(
            lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
                &checked,
                produced.artifact(),
                &substituted,
            )
            .is_err(),
            "source-call mutation {mutation} must not publish coverage"
        );
    }
    let mut substituted = checked.clone();
    substituted.facts.operators.boundary_applications.clear();
    assert!(
        lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &substituted,
            produced.artifact(),
            lowered,
        )
        .is_err(),
        "a missing application is not canonical empty application custody"
    );
    drop(substituted);
    let artifact = produced.into_artifact();
    drop(checked);
    let module = decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.provider_candidates.len(), 1);
    let TerminalExecutionResult::Scalar(terminal_interpreter::TerminalScalarValue::Integer {
        value,
        ..
    }) = run_artifact_with_provider(&artifact, &module, "CheckedMathProvider", &[])
    else {
        panic!("the installed provider returns an integer");
    };
    assert_eq!(
        value,
        semantic_vocabulary::IntegerValue::Signed(71),
        "the caller consumes the selected provider's result",
    );
}

#[test]
fn named_boundary_return_preserves_scalar_requirements() {
    let checked = crate::front_end::checked_program(
        r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(input: i32) -> i32
        requires input >= 0;
        machine run() -> i32 {
            let result: i32 = CheckedMath::offset_zero(70);
            result
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .boundary_machines
            .iter()
            .all(|boundary| boundary.scalar_requires.is_empty()),
        "the legacy return route cannot discharge or erase scalar boundary requirements"
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("run"),
    )
    .expect("the ordinary requirement-call route retains its precondition");
    let boundary = lowered
        .semantic_module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.kind == terminal_psi::BoundaryRequirementKind::Operator)
        .expect("named requirement declaration");
    assert!(!boundary.scalar_requires.is_empty());
    encode_module(&lowered.semantic_module).expect("retained call obligations are well formed");
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
