use super::validate_constant_evaluations;
use crate::compile_time_invocations::{
    EvaluatedValues, InvocationSourceConsent, PreCheckEvaluation, SourceCompletionAccounting,
    ValueDemand, evaluate_demanded_values,
};
use crate::source_state::TypedTrees;
use crate::source_state::data::DataMember;
use crate::source_state::types::{FixedArrayLength, TypeReferenceNode};
use crate::test_support::frontend_fixture;
use checked_program_to_psi as production;
use source_completion_accounting::{PureEvaluationRecord, PureTerminalEvaluationOutcome};
use source_provenance::DependencyScope;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees as typing;
use syntax_trees_to_symbol_resolved_trees::resolution;
use typed_trees_to_prepared_typed_trees::{
    PreparedValue, TargetInputs, ValueInputs, prepare_typed_trees,
};

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

/// Evaluates everything `compilation` demands of `program`, the way source
/// preparation does, with no deployment and no package authority.
fn evaluate(
    program: &mut TypedTrees,
    compilation: production::TerminalSourceCompilation<'_>,
    source_consent: Option<Arc<dyn InvocationSourceConsent>>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<Vec<ValueInputs>, Vec<diagnostics::Diagnostic>> {
    evaluate_with_reaches(program, compilation, source_consent, accounting)
        .map(|evaluated| evaluated.inputs)
}

/// The same, keeping the reach the receipt join compares.
fn evaluate_with_reaches(
    program: &mut TypedTrees,
    compilation: production::TerminalSourceCompilation<'_>,
    source_consent: Option<Arc<dyn InvocationSourceConsent>>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<EvaluatedValues, Vec<diagnostics::Diagnostic>> {
    evaluate_demanded_values(
        program,
        &ValueDemand {
            compilation,
            layout_scopes: &[],
            authority: None,
            source_consent,
        },
        accounting,
    )
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
fn closed_domain_index_calls_use_their_evaluated_value() {
    let check = |family: &str, parameter: &str, output: &str, body: &str| {
        let text = format!(
            "const KEY: u8 = 4u8; {family} \
             machine identity(value: u8) -> u8 crashes none {{ value }} \
             machine main(value: {parameter}) -> u32 in At<{output}> {{ {body} }}"
        );
        let mut program = typed_source(&text);
        let entry = program.realized_machine_named("main").unwrap().symbol;
        let entries = [entry];
        let request = request(&entries);
        let mut accounting = SourceCompletionAccounting::default();
        let values = evaluate(&mut program, request, None, &mut accounting).unwrap();
        assert!(
            values
                .iter()
                .any(|value| matches!(value.value, PreparedValue::StaticArgument { .. })),
            "no evaluation requested the domain index"
        );
        let prepared = prepare_typed_trees(program, &values, &[]).unwrap();
        crate::test_support::judge_terminal_source(
            prepared,
            request,
            terminal_codec::TerminalPublication::Invocation,
        )
    };
    let fixed = "domain<const N: u8> u32::At<N>;";
    let generic = "domain<T, const N: u8> T::At<N>;";
    for (family, parameter, output, body) in [
        (fixed, "u32 in At<(identity(KEY))>", "4", "value"),
        (fixed, "u32 in At<4>", "(identity(KEY))", "value"),
        (
            fixed,
            "u32 in At<(identity(KEY))>",
            "(identity(4u8))",
            "value",
        ),
        (generic, "u32 in At<(identity(KEY))>", "4", "value"),
        (fixed, "u32", "4", "value as +At<(identity(KEY))>"),
    ] {
        if let Err(diagnostics) = check(family, parameter, output, body) {
            panic!("{parameter} to {output} with {body}: {diagnostics:?}");
        }
    }
    let errors = check(fixed, "u32 in At<(identity(KEY))>", "5", "value")
        .expect_err("different index values must refuse");
    assert!(
        diagnostics::join_messages(&errors)
            .contains("a type gains or loses a domain only through a cast"),
        "{errors:?}"
    );
}

#[test]
fn a_const_argument_erased_by_an_alias_evaluates_on_its_source_owner() {
    use crate::source_state::deferred_refusals::DeferredRequest;

    let source = "data Word { bits: u32; } data Plain<const N: u8> = Word;
        machine bound() -> u8 { 2u8 }
        machine bad() -> u8 { crash Trap; }
        machine good(x: Plain<bound()>) {}
        machine trapping(x: Plain<bad()>) {}
        machine restricted(v: u8, x: Plain<bound()>, y: Plain<v + 1u8>) {}";
    for (name, allowed) in [("good", true), ("restricted", false)] {
        let mut program = typed_source(source);
        let entry = program.realized_machine_named(name).unwrap().symbol;
        let entries = [entry];
        let request = request(&entries);
        let mut accounting = SourceCompletionAccounting::default();
        let values = evaluate(&mut program, request, None, &mut accounting).unwrap();
        assert_eq!(evaluations(&program, &accounting), [(name.to_owned(), 2)]);
        assert_eq!(
            program
                .deferred_refusals_of(entry)
                .any(|row| row.request == DeferredRequest::ConstArgumentAdmission),
            !allowed
        );
        let prepared = prepare_typed_trees(program, &values, &[]).unwrap();
        let judged = crate::test_support::judge_terminal_source(
            prepared,
            request,
            terminal_codec::TerminalPublication::Invocation,
        );
        assert_eq!(judged.is_ok(), allowed, "{name}");
    }
    let mut program = typed_source(source);
    let entry = program.realized_machine_named("trapping").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    let errors = evaluate(&mut program, request(&[entry]), None, &mut accounting).unwrap_err();
    assert!(
        errors.iter().any(|error| {
            error.message.contains("const argument for `N` of `Plain`")
                && error.message.contains("crash")
        }),
        "{errors:?}"
    );
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
    let evaluated =
        evaluate_with_reaches(&mut typed, request(&[entry]), None, &mut accounting).unwrap();
    let reaches = &evaluated.constant_reaches;
    let [input] = evaluated.inputs.as_slice() else {
        panic!("one demanded constant")
    };
    let PreparedValue::Constant(demanded) = input.value else {
        panic!("named initializer")
    };
    assert!(
        typed
            .const_declaration_by_symbol(demanded)
            .unwrap()
            .evaluated_value
            .is_some()
    );
    validate_constant_evaluations(&typed, &accounting, reaches).unwrap();
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
        validate_constant_evaluations(&typed, &Default::default(), reaches).unwrap_err(),
        "constant result has no accepted invocation receipt"
    );
    let original = Arc::downgrade(accounting.constant_input(receipt).unwrap().unwrap());
    let mut retry = SourceCompletionAccounting::default();
    retry.append(accounting.clone());
    accounting.release_constant_inputs();
    accounting.validate_charges().unwrap();
    validate_constant_evaluations(&typed, &retry, reaches).unwrap();
    retry.release_constant_inputs();
    assert_eq!(original.strong_count(), 0);
    retry.validate_charges().unwrap();
}

#[test]
fn a_closed_arithmetic_extent_settles_at_its_original_array() {
    let source = |expression: &str| {
        typed_source(&format!(
            "machine read(bytes: @[u8; {expression}]) -> u8 {{ bytes[3u64] }}"
        ))
    };
    let mut program = source("3u64 + 1u64");
    let reader = program.realized_machine_named("read").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    let values = evaluate(&mut program, request(&[reader]), None, &mut accounting).unwrap();
    let [input] = values.as_slice() else {
        panic!("the written addition is one input")
    };
    assert!(matches!(input.value, PreparedValue::StaticArgument { owner, .. } if owner == reader));
    assert_eq!(accounting.receipts().len(), 1);
    assert!(program.deferred_refusal(reader).is_none());
    assert!(
        program
            .type_reference_table
            .type_reference_rows()
            .any(|(_, ty)| matches!(
                ty,
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Literal(4),
                    ..
                }
            ))
    );
    let mut invalid = source("18446744073709551615u64 + 1u64");
    let reader = invalid.realized_machine_named("read").unwrap().symbol;
    let failure = evaluate(
        &mut invalid,
        request(&[reader]),
        None,
        &mut Default::default(),
    )
    .expect_err("an exact addition outside its u64 carrier cannot supply an extent");
    assert!(diagnostics::join_messages(&failure).contains("computed array extent"));
}

#[test]
fn only_a_reader_of_an_array_requests_its_extent() {
    let base = typed_source(
        "const WIDTH: u64 = 4u64; const UNUSED: u64 = 4u64;
        machine extent() -> u64 { WIDTH } data Packet { bytes: [u8; extent()]; }
        machine reader(packet: @Packet) -> u64 { 0u64 } machine unrelated() -> u64 { 0u64 }",
    );
    for (entry, computes_extent) in [("reader", true), ("unrelated", false)] {
        let mut typed = base.clone();
        let entry = typed.realized_machine_named(entry).unwrap().symbol;
        let mut accounting = SourceCompletionAccounting::default();
        let values = evaluate(&mut typed, request(&[entry]), None, &mut accounting).unwrap();
        let prepared = prepare_typed_trees(typed.clone(), &values, &[]).unwrap();
        // The reader demands the extent and the constant it reads. Nothing
        // demands either for the unrelated machine.
        assert_eq!(prepared.values().len(), values.len());
        assert_eq!(
            values
                .iter()
                .map(|input| match input.value {
                    PreparedValue::StaticArgument { .. } => "extent",
                    PreparedValue::Constant(_) => "constant",
                })
                .collect::<Vec<_>>(),
            if computes_extent {
                vec!["extent", "constant"]
            } else {
                Vec::new()
            }
        );
        assert_eq!(
            prepared
                .constants()
                .map(|constant| typed.symbols.name(constant.symbol))
                .collect::<Vec<_>>(),
            if computes_extent {
                vec!["WIDTH"]
            } else {
                Vec::new()
            }
        );
        if computes_extent {
            assert!(matches!(
                values[0].value,
                PreparedValue::StaticArgument { .. }
            ));
            assert_eq!(values[0].targets, TargetInputs::None);
            let other = typed_source("const WIDTH:u64 = 4u64; machine main() -> u64 { WIDTH }");
            assert!(
                prepare_typed_trees(other, &values, &[]).unwrap_err()[0]
                    .message
                    .contains("another source program")
            );
        }
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
        let result = evaluate(
            &mut candidate,
            production::TerminalSourceCompilation {
                purpose: DependencyScope::Build,
                ..request(&[entry])
            },
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
        let result = evaluate(
            &mut typed,
            production::TerminalSourceCompilation {
                subjects: &subjects,
                supplies: &supplies,
                ..request(entries)
            },
            None,
            &mut accounting,
        );
        if succeeds {
            let demanded = result.unwrap();
            assert_eq!(demanded.len(), 1);
            assert_eq!(
                demanded[0].value,
                typed_trees_to_prepared_typed_trees::PreparedValue::Constant(
                    typed
                        .const_declarations()
                        .iter()
                        .find(|row| typed.symbols.name(row.symbol) == "GOOD")
                        .unwrap()
                        .symbol
                )
            );
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
                diagnostics.iter().any(|error| error.message
                    == "constant `CYCLIC` needs its own value, so its evaluation has no first \
                        step"),
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
    evaluate(
        &mut typed,
        production::TerminalSourceCompilation {
            supplies: &supplies,
            ..request(&[])
        },
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

/// The requesting declaration and returned integer of every invocation, in
/// the order the invocations ran.
fn evaluations(typed: &TypedTrees, accounting: &SourceCompletionAccounting) -> Vec<(String, i128)> {
    accounting
        .receipts()
        .iter()
        .map(|record| {
            let PureEvaluationRecord::Invocation(receipt) = record else {
                panic!("a source invocation")
            };
            let PureTerminalEvaluationOutcome::Materialized {
                value: build_time_values::BuildTimeValue::Int(value),
                ..
            } = receipt.outcome()
            else {
                panic!("an integer result")
            };
            (
                typed.symbols.name(receipt.requester()).to_owned(),
                i128::from(*value),
            )
        })
        .collect()
}

/// Four values, each needing the next: the extent in `main` calls `outer`,
/// which reads `HELD`; `HELD` is computed through `Buffer<inner()>`; `inner`
/// reads `BASE`. A constant comes before an argument and an argument before
/// a constant, so neither kind can run first as a whole.
const CHAIN: &str = "data Buffer<const Capacity: u64> { slots: [u8; Capacity]; }
    const BASE: u64 = base();
    machine base() -> u64 crashes none { 2u64 }
    machine inner() -> u64 crashes none { BASE }
    const HELD: u64 = held();
    machine held() -> u64 crashes none { let buffer: Buffer<inner()>; inner() }
    machine outer() -> u64 crashes none { HELD }
    machine main(values: @[u8; outer()]) -> u64 { 0u64 }";

#[test]
fn each_value_is_evaluated_once_after_the_values_it_reads() {
    let mut typed = typed_source(CHAIN);
    let entry = typed.realized_machine_named("main").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    let EvaluatedValues {
        inputs: values,
        constant_reaches,
    } = evaluate_with_reaches(&mut typed, request(&[entry]), None, &mut accounting).unwrap();
    // One invocation per value, each named by the declaration that asked,
    // and none before a value it reads.
    assert_eq!(
        evaluations(&typed, &accounting),
        [("BASE", 2), ("held", 2), ("HELD", 2), ("main", 2)]
            .map(|(name, value)| (name.to_owned(), value)),
    );
    accounting.validate_charges().unwrap();
    validate_constant_evaluations(&typed, &accounting, &constant_reaches).unwrap();
    // Stage 04 receives the two arguments, then the two constants.
    assert_eq!(
        values
            .iter()
            .map(|input| match input.value {
                PreparedValue::StaticArgument { owner, .. } => typed.symbols.name(owner),
                PreparedValue::Constant(constant) => typed.symbols.name(constant),
            })
            .collect::<Vec<_>>(),
        ["held", "main", "BASE", "HELD"],
    );
    prepare_typed_trees(typed, &values, &[]).unwrap();
}

#[test]
fn a_constant_waits_for_the_constant_that_sizes_a_type_it_reads() {
    // `[u8; LEFT]` names `LEFT` in a type, not in an expression. `right`
    // holds that array, so `RIGHT` has no value before `LEFT` has one.
    let mut typed = typed_source(
        "const RIGHT: u64 = right();
        machine right() -> u64 crashes none { let held: [u8; LEFT]; held.length }
        const LEFT: u64 = left();
        machine left() -> u64 crashes none { 3u64 }
        machine main() -> u64 crashes none { RIGHT }",
    );
    let entry = typed.realized_machine_named("main").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    evaluate(&mut typed, request(&[entry]), None, &mut accounting).unwrap();
    assert_eq!(
        evaluations(&typed, &accounting),
        [("LEFT", 3), ("RIGHT", 3)].map(|(name, value)| (name.to_owned(), value)),
    );
}

#[test]
fn values_that_need_each_other_are_refused_by_name() {
    // `TOTAL` is computed through `Buffer<capacity()>`, and `capacity` reads
    // `TOTAL`. `FREE` needs neither, so it is evaluated before the refusal.
    let source = "data Buffer<const Capacity: u64> { slots: [u8; Capacity]; }
        const FREE: u64 = free();
        machine free() -> u64 crashes none { 1u64 }
        const TOTAL: u64 = total();
        machine capacity() -> u64 crashes none { TOTAL }
        machine total() -> u64 crashes none { let buffer: Buffer<capacity()>; 3u64 }
        machine main() -> u64 crashes none { TOTAL + FREE }";
    let mut typed = typed_source(source);
    let entry = typed.realized_machine_named("main").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    let refusal = evaluate(&mut typed, request(&[entry]), None, &mut accounting).unwrap_err();
    let [error, note] = refusal.as_slice() else {
        panic!("one refusal and one note: {refusal:?}")
    };
    assert_eq!(
        error.message,
        "the const argument for `Capacity` of `Buffer` in `total` needs constant `TOTAL`, which \
         needs the const argument for `Capacity` of `Buffer` in `total`: none of them can be \
         evaluated first"
    );
    let written = |diagnostic: &diagnostics::Diagnostic| {
        let span = diagnostic.source_span.unwrap().span;
        &source[span.start..span.end]
    };
    assert_eq!(written(error), "capacity()");
    assert_eq!(note.message, "constant `TOTAL` is part of that circle");
    assert_eq!(written(note), "TOTAL");
    assert_eq!(evaluations(&typed, &accounting), [("FREE".to_owned(), 1)]);

    // The same refusal for one value: `SIDE` is computed by a machine that
    // holds an array sized by `SIDE`.
    let mut typed = typed_source(
        "const SIDE: u64 = side();
        machine side() -> u64 crashes none { let held: [u8; SIDE]; 2u64 }
        machine main() -> u64 crashes none { SIDE }",
    );
    let entry = typed.realized_machine_named("main").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    let refusal = evaluate(&mut typed, request(&[entry]), None, &mut accounting).unwrap_err();
    assert_eq!(
        diagnostics::join_messages(&refusal),
        "constant `SIDE` needs its own value, so its evaluation has no first step"
    );
    assert!(accounting.receipts().is_empty());
}

/// `VALUE` is computed by `calc`, which builds a `Packet`. Nothing it runs
/// names `Spare`.
const READS_PACKET: &str = "data Packet { bytes: u8; }
    data Spare { bytes: u8; }
    const VALUE: u8 = calc();
    machine calc() -> u8 crashes none { let packet: Packet = Packet { bytes: 3u8 }; packet.bytes }
    machine spare(held: Spare) -> u8 crashes none { held.bytes }
    machine main() -> u8 crashes none { VALUE }";

/// Install a later plan at a real written type reference. The application
/// identifies the storage. Its semantic declaration can have other storage.
fn join_after_a_later_layout_of(data: &str, storage: &str) -> Result<(), String> {
    let mut typed = typed_source(READS_PACKET);
    let entry = typed.realized_machine_named("main").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    let evaluated =
        evaluate_with_reaches(&mut typed, request(&[entry]), None, &mut accounting).unwrap();
    validate_constant_evaluations(&typed, &accounting, &evaluated.constant_reaches).unwrap();
    let symbol = typed.data_definition_by_name_str(data).unwrap().symbol;
    let storage_symbol = typed.data_definition_by_name_str(storage).unwrap().symbol;
    let occurrences = crate::source_state::references::collect_program_type_occurrences(&typed);
    let written = |owner| {
        occurrences
            .iter()
            .map(|row| row.reference)
            .find(|&reference| typed.type_reference_symbol(reference) == owner)
            .expect("the fixture uses both types")
    };
    let application = written(storage_symbol);
    let semantic_type = written(symbol);
    typed
        .plan_laid_layouts
        .push(crate::source_state::PlanLaidLayout {
            application,
            semantic_type,
            target_inputs: Vec::new(),
            data_name: data.to_owned(),
            data_symbol: symbol,
            field_symbols: Vec::new(),
            schema_symbol: symbol,
            schema_field_symbols: Vec::new(),
            policy_symbol: symbols::SymbolHandle::invalid(),
            policy_plan_machine_symbol: symbols::SymbolHandle::invalid(),
            schema_argument: None,
            realizations: crate::source_state::PlanLaidRealizations::TargetInputs(
                arena::HandleSpan::empty(),
            ),
        });
    validate_constant_evaluations(&typed, &accounting, &evaluated.constant_reaches)
}

#[test]
fn a_later_layout_of_data_the_constant_never_reached_is_accepted() {
    join_after_a_later_layout_of("Spare", "Spare").unwrap();
}

#[test]
fn receipt_storage_inputs_include_a_type_inferred_from_a_call() {
    let typed = typed_source(
        "data Packet { byte: u8; }
         machine make_packet() -> Packet { Packet { byte: 3u8 } }
         machine read_packet() -> u8 { let packet: Packet = make_packet(); packet.byte }",
    );
    let reader = typed.realized_machine_named("read_packet").unwrap().symbol;
    let packet = typed.data_definition_by_name_str("Packet").unwrap().symbol;
    let reads = crate::source_state::references::selected_storage_type_references(
        &typed,
        &super::DeclarationSelection::from_symbols([reader]),
    );
    assert!(
        reads
            .into_iter()
            .any(|reference| typed.type_reference_symbol(reference) == packet)
    );
}

#[test]
fn another_storage_of_the_same_semantic_data_does_not_change_the_constant() {
    join_after_a_later_layout_of("Packet", "Spare").unwrap();
}

#[test]
fn a_later_layout_of_data_the_constant_reached_refuses_its_receipt() {
    assert_eq!(
        join_after_a_later_layout_of("Packet", "Packet").unwrap_err(),
        "constant receipt read a layout that changed after the constant was evaluated"
    );
}
