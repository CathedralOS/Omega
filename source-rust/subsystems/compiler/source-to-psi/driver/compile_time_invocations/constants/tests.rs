use super::{
    evaluate_const_arguments, evaluate_constant_initializers_with_source_consent,
    validate_constant_evaluations,
};
use crate::compile_time_invocations::{PreCheckEvaluation, SourceCompletionAccounting};
use crate::source_state::TypedTrees;
use crate::source_state::data::DataMember;
use crate::source_state::types::{FixedArrayLength, TypeReferenceNode};
use crate::test_support::frontend_fixture;
use checked_program_to_psi::production;
use source_completion_accounting::{PureEvaluationRecord, PureTerminalEvaluationOutcome};
use source_provenance::DependencyScope;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::typing;
use syntax_trees_to_symbol_resolved_trees::resolution;

fn typed_source(text: &str) -> TypedTrees {
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x52; 32]).unwrap();
    typed_source_in(text, Some(package), DependencyScope::Product)
}

fn typed_source_in(
    text: &str,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    purpose: DependencyScope,
) -> TypedTrees {
    let mut sources = crate::source_inputs::SourceMap::default();
    let source = sources
        .add_checked_instance(
            "constants/main.omg".into(),
            text,
            "constants".into(),
            package,
            source_admission::SourceOrigin::User,
            source_admission::SourceResolutionStratum::Base,
            purpose,
        )
        .source_id;
    let tokens = frontend_fixture::Lexer::new(text).tokenize().unwrap();
    let syntax = frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap();
    let mut request = resolution::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    let resolved = resolution::resolve(request).unwrap();
    let mut typed = typing::lower_symbol_resolved_trees(&resolved).unwrap();
    PreCheckEvaluation::defer_declared_requests(&mut typed).unwrap();
    typed
}

fn request(entries: &[symbols::SymbolHandle]) -> production::TerminalSourceCompilation<'_> {
    production::TerminalSourceCompilation {
        entries,
        subjects: &[],
        purpose: DependencyScope::Product,
        declaration_metadata: &[],
        selected: &[],
        bindings: &[],
        calling: None,
        providers: None,
        supplies: &[],
    }
}

#[test]
fn a_computed_constant_uses_its_source_receipt_only_when_reached() {
    let mut typed = typed_source(
        "const VALUE: u32 = calc(); const UNREAD: u32 = spare();
        machine spare() -> u32 crashes none { 99 } machine calc() -> u32 crashes none { 17 }
        machine main() -> u32 crashes none { VALUE }",
    );
    let entry = typed.realized_machine_named("main").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    let demanded = evaluate_constant_initializers_with_source_consent(
        &mut typed,
        request(&[entry]),
        &[],
        None,
        None,
        &mut accounting,
    )
    .unwrap();
    assert_eq!(demanded.len(), 1);
    assert!(
        typed
            .const_declaration_by_symbol(demanded[0])
            .unwrap()
            .evaluated_value
            .is_some()
    );
    validate_constant_evaluations(&typed, &accounting).unwrap();
    let [PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
        panic!("one actual invocation")
    };
    assert!(matches!(
        receipt.outcome(),
        PureTerminalEvaluationOutcome::Materialized {
            value: build_time_values::BuildTimeValue::Int(17),
            ..
        }
    ));
    assert_eq!(
        validate_constant_evaluations(&typed, &Default::default()).unwrap_err(),
        "constant result has no accepted invocation receipt"
    );
    let original = Arc::downgrade(accounting.constant_input(receipt).unwrap().unwrap());
    let mut retry = SourceCompletionAccounting::default();
    retry.append(accounting.clone());
    accounting.release_constant_inputs();
    accounting.validate_charges().unwrap();
    validate_constant_evaluations(&typed, &retry).unwrap();
    retry.release_constant_inputs();
    assert_eq!(original.strong_count(), 0);
    retry.validate_charges().unwrap();
}

#[test]
fn only_a_reader_of_an_array_requests_its_extent() {
    let base = typed_source(
        "machine extent() -> u64 { 4u64 } data Packet { bytes: [u8; extent()]; }
        machine reader(packet: @Packet) -> u64 { 0u64 } machine unrelated() -> u64 { 0u64 }",
    );
    for (entry, computes_extent) in [("reader", true), ("unrelated", false)] {
        let mut typed = base.clone();
        let entry = typed.realized_machine_named(entry).unwrap().symbol;
        let mut accounting = SourceCompletionAccounting::default();
        evaluate_const_arguments(&mut typed, request(&[entry]), None, None, &mut accounting)
            .unwrap();
        assert_eq!(accounting.receipts().len(), usize::from(computes_extent));
        accounting.validate_charges().unwrap();
        let packet = typed.data_definition_by_name_str("Packet").unwrap();
        let Some([DataMember::Field(bytes)]) = typed.data_members(packet).as_array() else {
            panic!("the original array field")
        };
        let array = typed
            .type_reference_table
            .type_reference(bytes.type_reference);
        assert_eq!(
            matches!(
                array,
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Literal(4),
                    ..
                }
            ),
            computes_extent
        );
    }
}

#[test]
fn anonymous_const_argument_requires_the_build_capture_that_owns_the_call() {
    use crate::compile_time_invocations::InvocationSourceConsent;
    use crate::source_inputs::SourceMap;

    struct Capture(Arc<SourceMap>);
    impl InvocationSourceConsent for Capture {
        fn allows_selection(
            &self,
            sources: &Arc<SourceMap>,
            requester: source_text::SourceSpan,
            selected: source_text::SourceSpan,
            purpose: DependencyScope,
        ) -> bool {
            Arc::ptr_eq(&self.0, sources)
                && purpose == DependencyScope::Build
                && [requester, selected].into_iter().all(|span| {
                    sources.get(span.source_id).is_some_and(|file| {
                        file.package_identity.is_none()
                            && file.dependency_scope == purpose
                            && file.source.get(span.span.start..span.span.end).is_some()
                    })
                })
        }
    }

    let source = "data Buffer<const N: u64> { bytes: [u8; N]; }
        machine size() -> u64 { 4u64 }
        machine build() { let held: Buffer<size()>; }";
    let typed = typed_source_in(source, None, DependencyScope::Build);
    let edited = typed_source_in(
        &source.replace("4u64", "5u64"),
        None,
        DependencyScope::Build,
    );
    let entry = typed.realized_machine_named("build").unwrap().symbol;
    let sources = typed.symbols.source_map().unwrap();
    let captures = [
        (Some(sources), true),
        (edited.symbols.source_map(), false),
        (None, false),
    ];
    for (capture, allowed) in captures {
        let mut candidate = typed.clone();
        let mut accounting = SourceCompletionAccounting::default();
        let result = evaluate_const_arguments(
            &mut candidate,
            production::TerminalSourceCompilation {
                purpose: DependencyScope::Build,
                ..request(&[entry])
            },
            None,
            capture.map(|sources| Arc::new(Capture(sources)) as Arc<_>),
            &mut accounting,
        );
        if allowed {
            result.unwrap();
            assert_eq!(accounting.receipts().len(), 1);
        } else {
            let errors = result.unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("without reconciled package custody")),
                "{errors:?}"
            );
            assert!(accounting.receipts().is_empty());
        }
    }
}

#[test]
fn a_supplied_requirement_only_evaluates_its_provider_member() {
    let base = typed_source(
        "boundary trait Available {
            machine get() -> u32 reaches none crashes none;
            machine other() -> u32 reaches none crashes none;
        }
        Provider satisfies Available {
            machine get() -> u32 { GOOD }
            machine other() -> u32 { CYCLIC }
        }
        const GOOD: u32 = compute();
        machine compute() -> u32 crashes none { 17 }
        const CYCLIC: u32 = recurse();
        machine recurse() -> u32 crashes none { CYCLIC }
        machine independent_reader() -> u32 crashes none { recurse() }",
    );
    let provider = base.conformances()[0].symbol;
    let interface = &base.traits()[0];
    let requirements = base.trait_machine_signatures(interface);
    let requirement = |name: &str| {
        requirements
            .iter()
            .find(|row| row.name.as_str() == name)
            .unwrap()
            .symbol
    };
    // Only the consuming package is checked as a subject. A supply is the
    // only way this request reaches the dependency that defines the provider.
    let consumer = semantic_vocabulary::PackageKeyIdentity::from_digest([0x53; 32]).unwrap();
    let subjects = [Some(consumer)];
    let reader = base
        .realized_machine_named("independent_reader")
        .unwrap()
        .symbol;
    for (slot, entries, succeeds) in [
        (requirement("get"), &[][..], true),
        (requirement("other"), &[][..], false),
        (interface.symbol, &[][..], false),
        (requirement("get"), &[reader][..], false),
        (requirement("get"), &[provider][..], false),
    ] {
        let supplies = [production::TerminalSourceSupply { slot, provider }];
        let mut typed = base.clone();
        let mut accounting = SourceCompletionAccounting::default();
        let result = evaluate_constant_initializers_with_source_consent(
            &mut typed,
            production::TerminalSourceCompilation {
                subjects: &subjects,
                supplies: &supplies,
                ..request(entries)
            },
            &[],
            None,
            None,
            &mut accounting,
        );
        if succeeds {
            let demanded = result.unwrap();
            assert_eq!(demanded.len(), 1);
            assert_eq!(typed.symbols.name(demanded[0]), "GOOD");
            let [PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
                panic!("only the selected member's initializer runs")
            };
            assert!(matches!(
                receipt.outcome(),
                PureTerminalEvaluationOutcome::Materialized {
                    value: build_time_values::BuildTimeValue::Int(17),
                    ..
                }
            ));
        } else {
            let diagnostics = result.unwrap_err();
            assert!(
                diagnostics
                    .iter()
                    .any(|error| error.message == "constant evaluation dependency cycle"),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn an_externally_declared_provider_member_requests_its_parameter_layout() {
    let mut typed = typed_source(
        "machine extent() -> u64 { 4u64 }
        data Packet { bytes: [u8; extent()]; }
        boundary trait Available {
            machine get(packet: @Packet) -> u32 reaches none crashes none;
        }
        machine read(packet: @Packet) -> u32 satisfies Available::get { 17 }
        Provider satisfies Available { Available::get = read; }",
    );
    let provider = typed.conformances()[0].symbol;
    let slot = typed.trait_machine_signatures(&typed.traits()[0])[0].symbol;
    let supplies = [production::TerminalSourceSupply { slot, provider }];
    let mut accounting = SourceCompletionAccounting::default();
    evaluate_const_arguments(
        &mut typed,
        production::TerminalSourceCompilation {
            supplies: &supplies,
            ..request(&[])
        },
        None,
        None,
        &mut accounting,
    )
    .unwrap();
    let packet = typed.data_definition_by_name_str("Packet").unwrap();
    let Some([DataMember::Field(bytes)]) = typed.data_members(packet).as_array() else {
        panic!("the array parameter's field")
    };
    assert!(matches!(
        typed
            .type_reference_table
            .type_reference(bytes.type_reference),
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(4),
            ..
        }
    ));
    assert_eq!(accounting.receipts().len(), 1);
}

#[test]
fn failed_invocations_and_observation_decoding_retain_the_actual_charge() {
    use crate::compile_time_invocations::{InvocationTarget, PreparedBuildMachineProgram};
    use build_time_values::BuildTimeValue;

    let source = typed_source(
        "machine compute(flag: bool) -> u32 crashes none { 17 }
         machine fail() crashes Trap { crash Trap; }",
    );
    let symbol = source.realized_machine_named("compute").unwrap().symbol;
    let prepared = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
    let entry = prepared
        .entry(symbol, symbol, &[BuildTimeValue::Bool(true)])
        .unwrap();
    let produced = Arc::new(entry.produce_invocation().unwrap());
    let module = produced.view().module();
    let Some(semantic_vocabulary::TargetMachineCallable::Machine(caller)) = module.entry else {
        panic!("the generated invocation has a machine entry");
    };
    let call = module
        .machine(caller)
        .unwrap()
        .module_invocations()
        .next()
        .unwrap();
    // The interpreter starts the call, then refuses to capture the boolean.
    // Without the observation the same program returns its ordinary integer.
    let observe_flag = [terminal_interpreter::TerminalCallObservation {
        caller,
        site: call.site,
        callee: call.inputs.callee,
        argument: 0,
    }];
    let target = InvocationTarget {
        name: None,
        layout_target: None,
        carrier_layout: None,
        address_model: None,
    };
    let fail = source.realized_machine_named("fail").unwrap().symbol;
    let mut before_execution = SourceCompletionAccounting::default();
    let refusal = prepared
        .entry(fail, fail, &[])
        .unwrap()
        .invoke_recorded(target, &mut before_execution)
        .unwrap_err();
    assert!(
        refusal.contains("no crash route can occur before execution"),
        "{refusal}"
    );
    assert!(before_execution.receipts().is_empty());

    for observations in [&[][..], &observe_flag[..]] {
        let mut accounting = SourceCompletionAccounting::default();
        let mut decoded = false;
        let refusal = entry
            .invoke_with_observations(
                Arc::clone(&produced),
                target,
                &mut accounting,
                observations,
                |value, _| {
                    decoded = true;
                    assert_eq!(value, BuildTimeValue::Int(17));
                    Err::<(), _>("test decoder refused the observations".to_owned())
                },
            )
            .unwrap_err();
        assert_eq!(decoded, observations.is_empty(), "{refusal}");
        if decoded {
            assert_eq!(refusal, "test decoder refused the observations");
        } else {
            assert!(
                refusal.contains("source invocation execution")
                    && refusal.contains("compiler call observation requires"),
                "{refusal}"
            );
        }
        let [PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
            panic!("expected the actual invocation receipt: {refusal}");
        };
        receipt.require_program(&produced).unwrap();
        assert!(
            matches!(receipt.outcome(), PureTerminalEvaluationOutcome::Failed { reason } if *reason == refusal)
        );
        assert!(receipt.usage().total_units() > 0);
        accounting.validate_charges().unwrap();
    }
}

#[test]
fn completed_invocations_release_the_generated_caller_and_program() {
    use crate::compile_time_invocations::{InvocationTarget, PreparedBuildMachineProgram};
    use build_time_values::BuildTimeValue;

    let typed = typed_source("machine compute(value: u32) -> u32 crashes none { value }");
    let symbol = typed.realized_machine_named("compute").unwrap().symbol;
    let prepared = PreparedBuildMachineProgram::prepare(&typed, None).unwrap();
    let mut accounting = SourceCompletionAccounting::default();
    let mut owners = Vec::new();
    let target = InvocationTarget {
        name: None,
        layout_target: None,
        carrier_layout: None,
        address_model: None,
    };
    for number in 0..16 {
        let value = BuildTimeValue::Int(number);
        let entry = prepared
            .entry(symbol, symbol, std::slice::from_ref(&value))
            .unwrap();
        let program = Arc::new(entry.produce_invocation().unwrap());
        owners.push((
            Arc::downgrade(entry.accepted().source()),
            Arc::downgrade(&program),
        ));
        if let Some(PureEvaluationRecord::Invocation(previous)) = accounting.receipts().last() {
            let refusal = previous
                .require_site(&entry.source_invocation_inputs())
                .unwrap_err();
            assert_eq!(
                refusal,
                "evaluation receipt belongs to another source invocation"
            );
        }
        let returned = entry
            .invoke_with_observations(program, target, &mut accounting, &[], |value, _| Ok(value))
            .unwrap();
        assert_eq!(returned, value);
    }
    assert_eq!(accounting.receipts().len(), owners.len());
    accounting.validate_charges().unwrap();
    let callers = owners
        .iter()
        .filter(|(source, _)| source.strong_count() != 0)
        .count();
    let programs = owners
        .iter()
        .filter(|(_, code)| code.strong_count() != 0)
        .count();
    assert_eq!(
        (callers, programs),
        (0, 0),
        "objects retained after completing 16 calls"
    );
}
