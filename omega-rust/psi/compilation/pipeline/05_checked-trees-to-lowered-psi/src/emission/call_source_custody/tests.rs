//! Source custody for a scalar call result the same statement stores.
//!
//! `self.field = Host::close(..)` binds no authored local, so the boundary
//! call's result carries no `CheckedScalarExpressionRole::LocalInitializer`
//! and no `Return` role either. Custody rejoins the authored assignment's
//! right-hand side instead, and the consuming `StructuralScalarFieldStore`
//! reads the call's own SSA value. Without that arm the whole body refuses
//! with "scalar source custody disagrees with its authored destination role",
//! which is what kept `tests/omega/pass/filesystem/native_close` out of
//! lowering.
//!
//! `wiki/spec/terminal-psi/structural_access.md` gives
//! `StructuralScalarFieldStore` an "already-defined, exactly typed SSA value";
//! a completed call result is such a value, so no authored local has to name
//! it.

use crate::TerminalMachineSelection;
use crate::lower_machine;
use terminal_psi::{OperationKind, OperationResult, StructuralPathSegment, TerminalModule};

#[test]
fn nested_selected_named_operator_keeps_qualifier_and_ordered_value_custody() {
    use checked_trees::expression::ExpressionNode;
    use checked_trees::statement::StatementNode;

    let checked = checked_binding_context(
        "data ScalarMath {}
         boundary operator ScalarMath::keep(value: u32) -> u32
             requires value == value ensures result == value;
         data OtherMath {}
         boundary operator OtherMath::keep(value: u32) -> u32
             requires value == value ensures result == value;
         pub boundary trait Host { machine mark(value: u32); }
         machine run(left: u32, right: u32) reaches Host {
             Host::mark(ScalarMath::keep(left) ^ right);
         }",
    );
    let run = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let state = &checked.typed.machine_states(run)[0];
    let parameters = checked.typed.state_parameters(state);
    let left = parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == "left")
        .unwrap()
        .symbol;
    let right = parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == "right")
        .unwrap()
        .symbol;
    let StatementNode::Call(outer) = &checked
        .typed
        .statement_table
        .statements(state.statement_nodes)[0]
    else {
        panic!("outer observation call");
    };
    let arguments = checked
        .typed
        .statement_table
        .expression_handles(outer.arguments);
    let host = checked
        .typed
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Host")
        .unwrap();
    let mark = checked.typed.trait_machine_signatures(host)[0].symbol;
    let call = checked
        .facts
        .borrow
        .calls
        .iter()
        .find(|(_, call)| call.target_symbol == mark)
        .unwrap()
        .1
        .clone();
    let replay = |source: &checked_trees::CheckedTrees| {
        crate::expression_preparation::source_custody::computation_calls::replay_call_accesses(
            source,
            run.symbol,
            state.symbol,
            0,
            &call,
            arguments,
        )
    };
    replay(&checked).expect("selected qualifier lookup preserves the ordered operand observations");
    let accesses = checked
        .facts
        .borrow
        .argument_accesses
        .span(call.accesses)
        .unwrap();
    assert_eq!(
        accesses
            .iter()
            .map(|access| access.root_symbol)
            .collect::<Vec<_>>(),
        [left, right]
    );

    let (expression, operand) = checked
        .typed
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Call(call) if call.target.as_str() == "keep" => Some((
                handle,
                checked
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)[0],
            )),
            _ => None,
        })
        .expect("exact nested named operator occurrence");
    let mut forged_receiver = checked.clone();
    let ExpressionNode::Call(inner) = forged_receiver
        .typed
        .expression_table
        .expression_mut(expression)
    else {
        panic!("operator call");
    };
    inner.receiver = operand;
    assert!(
        replay(&forged_receiver).is_err(),
        "a runtime operand cannot replace the static qualifier"
    );

    let other = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked.typed.operator_path_members(operator.name)[0].as_str() == "OtherMath"
        })
        .unwrap()
        .symbol;
    let mut forged_target = checked.clone();
    let ExpressionNode::Call(inner) = forged_target
        .typed
        .expression_table
        .expression_mut(expression)
    else {
        panic!("operator call");
    };
    inner.target_symbol = other;
    assert!(
        replay(&forged_target).is_err(),
        "equal runtime signatures cannot substitute the selected qualifier/target pair"
    );

    let mut forged_observation = checked.clone();
    let accesses = forged_observation
        .facts
        .borrow
        .argument_accesses
        .span_mut(call.accesses)
        .unwrap();
    accesses[0].root_symbol = right;
    assert!(
        replay(&forged_observation).is_err(),
        "static lookup cannot erase or substitute an actual operand's captured read"
    );
}

#[test]
fn record_returning_static_call_updates_local_scratch() {
    let checked = checked_binding_context(
        "pub boundary trait Host { machine mark(value: u64); machine byte(value: u8); }
         data FillReport [copy] { value: u64; }
         machine fill(bytes: &mut [u8; 4]) -> FillReport {
             bytes[0] = 7;
             FillReport { value: 9 }
         }
         machine run() reaches Host {
             let mut bytes: [u8; 4];
             let report: FillReport = fill(&mut bytes);
             Host::mark(report.value);
             Host::byte(bytes[0]);
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("record-returning call borrows local scratch");
    #[derive(Default)]
    struct Observations(Vec<u128>);
    impl terminal_interpreter::TerminalEffectHandler for Observations {
        fn handle_effect(
            &mut self,
            effect: &terminal_interpreter::TerminalEffect,
        ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
            let terminal_interpreter::TerminalEffect::BoundaryCall { arguments, .. } = effect
            else {
                panic!("expected observation: {effect:?}");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("expected unsigned observation: {arguments:?}");
            };
            self.0.push(*value);
            Ok(())
        }
    }
    let mut observations = Observations::default();
    let result = terminal_interpreter::interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).expect("encode module"),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof"),
        &proof_admission::AdmissionProfile::default(),
        &[],
        terminal_interpreter::TerminalStructuralInputs::default(),
        &mut observations,
    )
    .unwrap_or_else(|error| {
        panic!(
            "verified local-buffer execution failed: {error:?}; observed {:?}",
            observations.0
        )
    });
    assert_eq!(
        result.value(),
        terminal_interpreter::TerminalExecutionResult::Unit
    );
    assert_eq!(observations.0, [9, 7]);
}

#[test]
fn copy_local_call_argument_survives_state_graph() {
    let checked = checked_binding_context(
        "data Value [copy] { count: u32; }
         machine read(value: Value) -> u32 { value.count }
         machine run() {
             let value: Value = Value { count: 7 };
             let result: u32 = read(value);
             transition result == 7 {
                 true -> done(result)
                 false -> done(0)
             }
             state done(result: u32) {}
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("copyable local is an ordinary owned call argument");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("copied argument retains independently checked source custody");
}

#[test]
fn qualified_static_binding_context_crosses_state_transition() {
    let checked = checked_binding_context(
        "pub boundary trait Host { machine ticks() -> u64; machine mark(value: u64); }
         pub data Clock { host: Binding<Host>; }
         machine Clock::read(clock: &mut Clock) reaches Host {
             let ticks: u64 = clock.host.ticks();
             transition ticks == 0 {
                 true -> report(ticks)
                 false -> report(ticks)
             }
             state report(ticks: u64) { Host::mark(ticks); }
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Clock::read"))
        .expect("qualified static state graph lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("qualified static state graph is independently verified");
}

#[test]
fn copy_local_argument_crosses_effectful_call_before_state_transition() {
    let checked = checked_binding_context(
        "pub boundary trait Host { machine mark(value: u32); }
         data Value [copy] { count: u32; }
         machine consume(value: Value) reaches Host { Host::mark(value.count); }
         machine run() reaches Host {
             let value: Value = Value { count: 7 };
             consume(value);
             consume(value);
             transition true { true -> done() false -> done() }
             state done() {}
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("local record is an ordinary argument to an effectful call");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("local argument custody is independently verified");

    #[derive(Default)]
    struct Observations(Vec<u128>);
    impl terminal_interpreter::TerminalEffectHandler for Observations {
        fn handle_effect(
            &mut self,
            effect: &terminal_interpreter::TerminalEffect,
        ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
            let terminal_interpreter::TerminalEffect::BoundaryCall { arguments, .. } = effect
            else {
                panic!("expected an observation call: {effect:?}");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("expected one unsigned argument: {arguments:?}");
            };
            self.0.push(*value);
            Ok(())
        }
    }
    let mut observations = Observations::default();
    let result = terminal_interpreter::interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).expect("encode module"),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof"),
        &proof_admission::AdmissionProfile::default(),
        &[],
        terminal_interpreter::TerminalStructuralInputs::default(),
        &mut observations,
    )
    .expect("checked copy-local program executes");
    assert_eq!(
        result.value(),
        terminal_interpreter::TerminalExecutionResult::Unit
    );
    assert_eq!(observations.0, [7, 7]);
}

#[test]
fn qualified_static_binding_context_lowers_without_attachment() {
    let checked = checked_binding_context(
        "pub boundary trait Host { machine ticks() -> u64; machine mark(value: u64); }
         pub data Clock { host: Binding<Host>; }
         machine Clock::read(clock: &mut Clock) reaches Host {
             let ticks: u64 = clock.host.ticks();
             Host::mark(ticks);
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Clock::read"))
        .expect("qualified static context lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("qualified static context is independently verified");
}

#[test]
fn qualified_static_binding_context_returns_a_record() {
    let checked = checked_binding_context(
        "pub boundary trait Host { machine ticks() -> u64; }
         pub data Clock { host: Binding<Host>; }
         pub data Reading [copy] { ticks: u64; }
         machine Clock::read(clock: &mut Clock) -> Reading reaches Host {
             let ticks: u64 = clock.host.ticks();
             Reading { ticks: ticks }
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Clock::read"))
        .expect("qualified static context can produce a record");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("record construction retains independently checked boundary custody");
}

#[test]
fn explicit_context_and_case_payload_cross_state_transition() {
    let checked = checked_binding_context(
        "pub data ReadingResult [copy] { case Empty; case Value(total: u64); }
         pub boundary trait Host {
             machine read() -> ReadingResult;
             machine write(total: u64);
         }
         pub data Context { host: Binding<Host>; }
         machine run(context: &mut Context) reaches Host {
             let result: ReadingResult = context.host.read();
             transition result {
                 ReadingResult::Value { total } -> report(context, total)
                 ReadingResult::Empty -> report(context, 0)
             }
             state report(context: &mut Context, total: u64) {
                 context.host.write(total);
             }
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("explicit context and payload are ordinary state arguments");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("case payload transfer is independently verified");
}

#[test]
fn explicit_binding_context_preserves_boundary_source_custody() {
    for projection in ["clock.host", "context.clock.host"] {
        let parameter = if projection.starts_with("context") {
            "context: &mut Context"
        } else {
            "clock: &mut Clock"
        };
        let checked = checked_binding_context(&format!(
            "pub boundary trait Host {{ machine ticks() -> u64; machine mark(value: u64); }}
             pub data Clock {{ host: Binding<Host>; }}
             pub data Context {{ clock: Clock; }}
             machine read({parameter}) reaches Host {{
                 let ticks: u64 = {projection}.ticks();
                 {projection}.mark(ticks);
             }}"
        ));
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("read"))
            .unwrap_or_else(|error| panic!("{projection}: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("explicit context preserves the boundary call");

        let mut changed = checked.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| !plan.structural_parameters.is_empty())
            .expect("ordinary context parameter");
        plan.structural_parameters.clear();
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("read")).is_err(),
            "{projection}: missing context custody must reject"
        );
    }
}

fn checked_binding_context(source: &str) -> checked_trees::CheckedTrees {
    use source::{SourceMap, SourceOrigin};
    use std::path::PathBuf;

    let binding = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/binding.omg"
    ));
    let mut sources = SourceMap::default();
    let binding_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/binding.omg"),
            binding.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let program_source = sources
        .add(PathBuf::from("tests/main.omg"), source.to_owned())
        .source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[(binding_source, binding), (program_source, source)],
    );
    let authorizations = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(
            |definition| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: definition.symbol,
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("fixture binding authorization");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("checked binding context")
}

#[test]
fn explicit_binding_source_replay_rejects_missing_parameter_and_erasure() {
    let checked = checked_binding_context(
        "pub boundary trait Host { machine ticks() -> u64; machine mark(value: u64); }
         pub data Clock { host: Binding<Host>; }
         machine read(clock: &mut Clock) reaches Host {
             let ticks: u64 = clock.host.ticks();
             Host::mark(ticks);
         }",
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| !plan.structural_parameters.is_empty())
        .expect("context parameter plan");
    let operation = plan
        .operations
        .iter()
        .find(|operation| operation.call().is_some())
        .expect("projected boundary call");
    super::validate_operation(
        &checked,
        plan.machine,
        plan.state,
        operation,
        &plan.structural_parameters,
    )
    .expect("source replay accepts exact authorized context");
    let error = super::validate_operation(&checked, plan.machine, plan.state, operation, &[])
        .expect_err("missing parameter custody");
    assert!(format!("{error:?}").contains("lost its retained structural parameter"));

    let mut changed = checked.clone();
    let field = changed
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter_mut()
        .filter_map(|shape| match &mut shape.shape {
            checked_trees::CheckedUnitStructuralTypeShape::Record { fields } => Some(fields),
            _ => None,
        })
        .flatten()
        .find(|field| {
            matches!(
                field.field_type,
                checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
            )
        })
        .expect("binding field receipt");
    if let checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked { erasure, .. } =
        &mut field.field_type
    {
        erasure.requirement = symbols::SymbolHandle::invalid();
    }
    let error = super::validate_operation(
        &changed,
        plan.machine,
        plan.state,
        operation,
        &plan.structural_parameters,
    )
    .expect_err("forged field erasure receipt");
    assert!(format!("{error:?}").contains("lost its exact Fused field erasure receipt"));
}

#[test]
fn boundary_local_buffer_preserves_backing_and_loan() {
    for (formal, borrow) in [
        ("&mut [u8]", "&mut"),
        ("&[u8]", "&"),
        ("&mut [u8; 4]", "&mut"),
    ] {
        let checked = crate::front_end::checked_program(&format!(
            "boundary trait Input {{ machine fill(buffer: {formal}) reaches Input; }}
             machine run() reaches Input {{
                 let mut buffer: [u8; 4];
                 let mut other: [u8; 4];
                 Input::fill({borrow} buffer);
                 Input::fill({borrow} buffer);
             }}"
        ));
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
            .unwrap_or_else(|error| panic!("{formal}: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("local buffer loans retain independently checked custody");

        for mutation in ["binding", "path", "access", "captured root", "mutability"] {
            if mutation == "mutability" && borrow == "&" {
                continue;
            }
            let mut changed = checked.clone();
            match mutation {
                "binding" | "path" | "access" => {
                    let other_binding = changed.facts.flow.terminal_unit_effects.machines.iter()
                        .flat_map(|machine| &machine.operations)
                        .find_map(|operation| match operation {
                            checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                            | checked_trees::CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
                                if result.statement_index == 1 => Some(result.binding_ordinal),
                            _ => None,
                        }).expect("same-typed other buffer producer");
                    let argument = changed
                        .facts
                        .flow
                        .terminal_unit_effects
                        .machines
                        .iter_mut()
                        .flat_map(|machine| &mut machine.operations)
                        .filter_map(checked_trees::CheckedUnitEffectOperationPlan::call_mut)
                        .filter(|call| {
                            call.is_boundary()
                                && *call.result() == checked_trees::CheckedUnitCallResult::Unit
                        })
                        .find_map(|call| call.structural_arguments.first_mut())
                        .expect("local buffer boundary argument");
                    if mutation == "binding" {
                        argument.source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal: other_binding };
                    } else if mutation == "path" {
                        argument
                            .path
                            .push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                    } else {
                        argument.access = checked_trees::CheckedStructuralAccess::Owned;
                    }
                }
                "captured root" => {
                    let access_handle = changed
                        .facts
                        .borrow
                        .argument_accesses
                        .iter()
                        .next()
                        .expect("captured buffer access")
                        .0;
                    changed
                        .facts
                        .borrow
                        .argument_accesses
                        .get_mut(access_handle)
                        .root_symbol = symbols::SymbolHandle::invalid();
                }
                "mutability" => {
                    let machine = changed
                        .machines()
                        .iter()
                        .find(|machine| machine.name.as_str() == "run")
                        .expect("run machine");
                    let statements = changed.machine_states(machine)[0].statement_nodes;
                    let local = changed
                        .typed
                        .statement_table
                        .statements_mut(statements)
                        .iter_mut()
                        .find_map(|node| match node {
                            checked_trees::statement::StatementNode::LocalData(local) => {
                                Some(local)
                            }
                            _ => None,
                        })
                        .expect("buffer declaration");
                    local.is_mutable = false;
                }
                _ => unreachable!(),
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("run")).is_err(),
                "{formal}: forged {mutation}"
            );
        }
    }
}

#[test]
fn boundary_record_result_can_replace_a_field() {
    let checked = crate::front_end::checked_program(
        r#"
            data Triple { first: u64; second: u64; third: u64; }
            boundary trait Host { machine receive(seed: u64) -> Triple reaches Host; }
            data Main { result: Triple; seed: u64; }
            machine Main::main(&mut self) reaches Host {
                self.seed = 7;
                self.result = Host::receive(self.seed);
                self.seed = 9;
                self.result = Host::receive(self.seed);
                transition { _ -> done() }
                state done(&mut self) { }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a boundary record result replaces the declared field");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the boundary call and destination retain independent verification");

    for mutation in [
        "result type",
        "result multiplicity",
        "call target",
        "statement",
    ] {
        let mut changed = checked.clone();
        let call = changed
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .flat_map(|machine| &mut machine.states)
            .flat_map(|state| &mut state.operations)
            .filter_map(checked_trees::CheckedUnitEffectOperationPlan::call_mut)
            .find(|call| call.is_boundary() && call.structural_result().is_some())
            .expect("boundary call in the assignment's ordered effects");
        match mutation {
            "result type" => {
                let (result, _) = call.structural_result_mut().expect("structural result");
                result.type_identity = "wrong record".to_owned();
            }
            "result multiplicity" => {
                let (result, _) = call.structural_result_mut().expect("structural result");
                result.multiplicity = language_semantics::Multiplicity::Linear;
            }
            "call target" => call.target.state = symbols::SymbolHandle::invalid(),
            "statement" => call.coordinate.statement_index += 1,
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
            "forged {mutation} must not pass result custody"
        );
    }
}

#[test]
fn explicit_owned_self_call_rejoins_only_its_authored_receiver() {
    let mut checked = crate::front_end::checked_program(
        "data Token [linear] { code: i32; }
         boundary requirement Token::finish(self);
         machine Token::settle(self, other: Token) {
             Token::finish(self);
             Token::finish(other);
         }",
    );
    lower_machine(&checked, TerminalMachineSelection::Name("Token::settle"))
        .expect("both owned parameters explicitly complete their boundary disposition");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Token::settle")
        .expect("settle machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("owned-self body has a Unit plan");
    let argument = plan
        .operations
        .iter_mut()
        .filter_map(checked_trees::CheckedUnitEffectOperationPlan::call_mut)
        .filter(|call| {
            call.is_boundary() && *call.result() == checked_trees::CheckedUnitCallResult::Unit
        })
        .find_map(|call| call.structural_arguments.first_mut())
        .expect("first boundary call consumes self");
    assert_eq!(argument.source_parameter_index(), Some(0));
    argument.source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert_eq!(
        lower_machine(&checked, TerminalMachineSelection::Name("Token::settle"))
            .expect_err("a same-typed parameter cannot replace the authored receiver"),
        crate::LoweringError::Unsupported("call argument changed its authored parameter place")
    );
}

/// The carrier path of the one field store that reads the one emitted call's
/// scalar result. Stores of other values, such as the preceding literal
/// assignment, are not this consumer.
fn carrier_path_storing_the_call_result(module: &TerminalModule) -> Vec<StructuralPathSegment> {
    let operations = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let calls = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::Call { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | OperationKind::BoundaryCall { .. }
            )
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one emitted call per authored call site: {calls:#?}");
    };
    let OperationResult::Scalar(result) = &call.result else {
        panic!("a scalar boundary call declares a scalar result: {call:#?}");
    };
    let consumers = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::StructuralScalarFieldStore { value, path, .. }
                if *value == result.id =>
            {
                Some(path.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [path] = consumers.as_slice() else {
        panic!("exactly one store reads the call result: {consumers:#?}");
    };
    path.clone()
}

/// The carrier path is the only difference from `DIRECT_FIELD`: the same
/// statement shape must compose with an ordered path to the carrier record
/// rather than depending on the field sitting directly on the root.
const CARRIER_PATH_FIELD: &str = r#"
    data Status { rc: i32; }
    data Main { fd_in: i32; status: Status; }
    boundary trait Host {
        machine close(fd: i32) -> i32 reaches Host;
    }
    machine Main::main(&mut self) reaches Host {
        self.fd_in = 5;
        self.status.rc = Host::close(self.fd_in);
    }
"#;

/// The same store into a bounded destination field. The call result carries no
/// range evidence, so the destination-range check must still refuse it: the
/// custody arm admits a source, not an unproved value.
const BOUNDED_DESTINATION: &str = r#"
    data Main { fd_in: i32; rc: i32 [0..=16]; }
    boundary trait Host {
        machine close(fd: i32) -> i32 reaches Host;
    }
    machine Main::main(&mut self) reaches Host {
        self.fd_in = 5;
        self.rc = Host::close(self.fd_in);
    }
"#;

#[test]
fn same_statement_field_store_composes_with_a_carrier_path() {
    let checked = crate::front_end::checked_program(CARRIER_PATH_FIELD);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .unwrap_or_else(|error| panic!("{CARRIER_PATH_FIELD}: {error:#?}"));
    assert_eq!(
        carrier_path_storing_the_call_result(&lowered.semantic_module),
        vec![StructuralPathSegment::Field("status".into())],
        "the ordered path reaches the carrier record holding `rc`"
    );
}

/// `cells[i].get()` lowers the runtime-selected borrowed element receiver to
/// a `RuntimeIndex` structural argument: the selector is the calling state's
/// own immutable scalar operand, the index bound is emitted as a call
/// obligation, and terminal verification discharges it both from the
/// retained-range spelling (`i: u64 [0..=1]`) and from the authored
/// `requires` conjunct.
#[test]
fn dynamic_indexed_shared_receiver_emits_a_runtime_index_argument() {
    for (index_decl, requires) in [("i: u64 [0..=1]", ""), ("i: u64", "requires i <= 1")] {
        let source = format!(
            "data Cell {{ value: u64; }}
             machine Cell::get(&self) -> u64 {{ self.value }}
             machine run(cells: &[Cell; 2], {index_decl}) -> u64
             {requires} {{ cells[i].get() }}"
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
            .unwrap_or_else(|error| panic!("{index_decl} {requires}: {error:#?}"));
        let runtime_indexed = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .flat_map(|operation| match &operation.kind {
                OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => structural_arguments.clone(),
                _ => Vec::new(),
            })
            .flat_map(|argument| argument.path)
            .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }));
        assert!(
            runtime_indexed,
            "{index_decl} {requires}: the borrowed element receiver lowers to a RuntimeIndex segment"
        );
        let _ = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("run"),
        )
        .produce(
            terminal_production::TerminalProductionCustody::artifact_only(
                &mut terminal_production::TerminalProductionTimings::default(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{index_decl} {requires}: terminal production rejects the runtime-indexed receiver: {error:#?}"
            )
        });
    }
}

#[test]
fn same_statement_field_store_still_refuses_an_unproved_bounded_destination() {
    let Err(diagnostics) = crate::front_end::checked_program_result(BOUNDED_DESTINATION) else {
        panic!("a call result with no range evidence cannot land in a bounded field");
    };
    assert!(
        !diagnostics.is_empty(),
        "the refusal names at least one diagnostic: {diagnostics:#?}"
    );
}

const INDEXED_LOCAL_CALL_LOANS: &str = r#"
    data Tally { hits: i32 in Wrapping; }
    data Board { tallies: [Tally; 2]; }
    data Report { total: i32 in Wrapping; }
    machine bump_hit(hits: &mut i32 in Wrapping) { hits = hits + 1; }
    machine Tally::absorb(into: &mut Tally, from: &Tally) -> Report {
        transition { _ -> apply(into, from) }
        state apply(into: &mut Tally, from: &Tally) {
            into.hits = into.hits + from.hits;
            transition { _ -> (Report { total: into.hits }) }
        }
    }
    machine run() {
        let mut board: Board;
        let mut other: Board;
        bump_hit(&mut board.tallies[1].hits);
        let report: Report = Tally::absorb(&mut board.tallies[1], &other.tallies[1]);
    }
"#;

#[test]
fn indexed_local_call_loans_reject_substituted_checked_custody() {
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    };
    let checked = crate::front_end::checked_program(INDEXED_LOCAL_CALL_LOANS);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("indexed scalar and state-forwarded record loans lower");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("both call families retain independently verified custody");

    let run = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let state = &checked.machine_states(run)[0];
    let statements = checked.statement_table.statements(state.statement_nodes);
    let local_symbol = |index| match &statements[index] {
        checked_trees::statement::StatementNode::LocalData(local) => local.symbol,
        _ => panic!("authored Board declaration"),
    };
    let board = local_symbol(0);
    let other = local_symbol(1);
    let operations = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .filter(|plan| plan.machine == run.symbol && plan.state == state.symbol)
        .flat_map(|plan| &plan.operations)
        .chain(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter()
                .filter(|plan| plan.machine == run.symbol)
                .flat_map(|plan| &plan.states)
                .filter(|plan| plan.state == state.symbol)
                .flat_map(|plan| &plan.operations),
        )
        .collect::<Vec<_>>();
    let other_binding = operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                if result.statement_index == 1 =>
            {
                Some(result.binding_ordinal)
            }
            _ => None,
        })
        .expect("same-typed other Board producer");
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|borrow| borrow.machine_symbol == run.symbol && borrow.state_symbol == state.symbol)
        .expect("run borrow state");

    for target_name in ["bump_hit", "Tally::absorb"] {
        let target = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == target_name)
            .unwrap()
            .symbol;
        let calls = operations
            .iter()
            .filter_map(|operation| operation.call())
            .filter(|call| call.target.machine == target)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1, "one exact {target_name} occurrence");
        let call = calls[0];
        assert_eq!(
            call.structural_result().is_some(),
            target_name == "Tally::absorb"
        );
        let coordinate = call.coordinate;
        let target_state = call.target.state;
        let captured = checked
            .facts
            .borrow
            .calls
            .span(borrow_state.calls)
            .unwrap()
            .iter()
            .filter(|call| {
                call.statement_index == coordinate.statement_index as usize
                    && call.call_ordinal == coordinate.call_ordinal as usize
                    && call.target_symbol == target_state
            })
            .collect::<Vec<_>>();
        assert_eq!(captured.len(), 1, "one exact captured {target_name} call");
        let access_span = captured[0].accesses;
        let positions = checked
            .facts
            .borrow
            .argument_accesses
            .span(access_span)
            .unwrap()
            .iter()
            .enumerate()
            .filter_map(|(position, access)| (access.root_symbol == board).then_some(position))
            .collect::<Vec<_>>();
        assert_eq!(
            positions.len(),
            1,
            "one exclusive Board access at {target_name}"
        );
        for mutation in [
            "root",
            "index",
            "access",
            "captured root",
            "captured path",
            "captured access",
            "captured lifetime",
        ] {
            let mut changed = checked.clone();
            if mutation.starts_with("captured ") {
                // Change only this occurrence's evidence; authored source and
                // the argument plan remain intact in each independent control.
                let row = &mut changed
                    .facts
                    .borrow
                    .argument_accesses
                    .span_mut(access_span)
                    .unwrap()[positions[0]];
                match mutation {
                    "captured root" => row.root_symbol = other,
                    "captured path" => {
                        let segments = row.segments;
                        let index = changed
                            .facts
                            .borrow
                            .access_segments
                            .span_mut(segments)
                            .unwrap()
                            .iter_mut()
                            .find(|segment| {
                                matches!(segment, facts::PlaceSegment::FixedIndex { index: 1 })
                            })
                            .expect("captured second element");
                        *index = facts::PlaceSegment::FixedIndex { index: 0 };
                    }
                    "captured access" => {
                        assert_eq!(row.kind, checked_trees::BorrowAccessKind::Mutable);
                        row.kind = checked_trees::BorrowAccessKind::Read;
                    }
                    "captured lifetime" => {
                        assert_eq!(row.end_before_access, None);
                        row.end_before_access = Some(positions[0] + 1);
                    }
                    _ => unreachable!(),
                }
            } else {
                let plans = &mut changed.facts.flow.terminal_unit_effects;
                let mut calls = plans
                    .machines
                    .iter_mut()
                    .filter(|plan| plan.machine == run.symbol && plan.state == state.symbol)
                    .flat_map(|plan| &mut plan.operations)
                    .chain(
                        plans
                            .composed_machines
                            .iter_mut()
                            .filter(|plan| plan.machine == run.symbol)
                            .flat_map(|plan| &mut plan.states)
                            .filter(|plan| plan.state == state.symbol)
                            .flat_map(|plan| &mut plan.operations),
                    )
                    .filter_map(CheckedUnitEffectOperationPlan::call_mut)
                    .filter(|call| call.target.machine == target && call.coordinate == coordinate);
                let call = calls.next().expect("retained call to mutate");
                assert!(calls.next().is_none());
                let argument = &mut call.structural_arguments[0];
                match mutation {
                    "root" => {
                        argument.source =
                            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: other_binding,
                            }
                    }
                    "index" => {
                        let index = argument
                            .path
                            .iter_mut()
                            .find(|segment| {
                                matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(1))
                            })
                            .expect("authored second element");
                        // Both elements have the same type and valid bounds;
                        // geometry alone cannot distinguish this substitution.
                        *index = CheckedUnitStructuralPathSegment::FixedIndex(0);
                    }
                    "access" => {
                        assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
                        argument.access = CheckedStructuralAccess::SharedBorrow;
                    }
                    _ => unreachable!(),
                }
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("run")).is_err(),
                "{target_name} accepted substituted {mutation}"
            );
        }
    }
}

#[test]
fn indexed_record_call_loans_preserve_disjoint_siblings_and_reject_overlap() {
    let disjoint = INDEXED_LOCAL_CALL_LOANS.replace("&other.tallies[1]", "&board.tallies[0]");
    let checked = crate::front_end::checked_program(&disjoint);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("one owner can lend disjoint record elements to a state-forwarded call");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("disjoint sibling loans retain independent verification");

    let overlapping = INDEXED_LOCAL_CALL_LOANS.replace("&other.tallies[1]", "&board.tallies[1]");
    let Err(diagnostics) = crate::front_end::checked_program_result(&overlapping) else {
        panic!("exclusive and shared loans of the same record element must conflict");
    };
    assert!(
        !diagnostics.is_empty(),
        "overlapping loans must report a diagnostic"
    );
}

#[test]
fn constructor_member_call_replays_its_own_captured_loan() {
    let definitions = INDEXED_LOCAL_CALL_LOANS
        .split("machine run() {")
        .next()
        .unwrap();
    let source = format!(
        "{definitions}
        data Envelope {{ report: Report; }}
        machine run(board: &mut Board, other: &Board) {{
            let envelope: Envelope = Envelope {{
                report: Tally::absorb(&mut board.tallies[1], &other.tallies[1])
            }};
        }}"
    );
    let mut checked = crate::front_end::checked_program(&source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("constructor member call keeps its nested source occurrence");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("constructor member loan verifies independently");
    let run = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let state = &checked.machine_states(run)[0];
    let other = checked.state_parameters(state)[1].symbol;
    let absorb = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Tally::absorb")
        .unwrap();
    let absorb_entry = checked.machine_states(absorb)[0].symbol;
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, row)| row)
        .find(|row| row.machine_symbol == run.symbol && row.state_symbol == state.symbol)
        .unwrap();
    let calls = checked
        .facts
        .borrow
        .calls
        .span(borrow_state.calls)
        .unwrap()
        .iter()
        .filter(|call| call.target_symbol == absorb_entry)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one constructor member invocation");
    let accesses = calls[0].accesses;
    assert_eq!(
        checked
            .facts
            .borrow
            .argument_accesses
            .span(accesses)
            .unwrap()
            .len(),
        2
    );
    let loan = &mut checked
        .facts
        .borrow
        .argument_accesses
        .span_mut(accesses)
        .unwrap()[0];
    assert_eq!(loan.kind, checked_trees::BorrowAccessKind::Mutable);
    assert_ne!(loan.root_symbol, other);
    loan.root_symbol = other;
    assert!(
        lower_machine(&checked, TerminalMachineSelection::Name("run")).is_err(),
        "a constructor's member call cannot substitute its captured exclusive root"
    );
}

#[test]
fn folded_local_selectors_keep_their_exact_captured_occurrences() {
    let mut checked = crate::front_end::checked_program(
        "machine write_marker(value: &mut u32) { value = 70; }
         machine run() {
             let values: [u32; 2] = [10, 20];
             let position: u64 = 1;
             let other_position: u64 = 1;
             write_marker(&mut values[position]);
             write_marker(&mut values[other_position]);
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("folded local selectors preserve their captured reads and loans");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("folded argument places verify independently");
    let run = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let state = &checked.machine_states(run)[0];
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, row)| row)
        .find(|row| row.machine_symbol == run.symbol && row.state_symbol == state.symbol)
        .unwrap();
    let calls = checked.facts.borrow.calls.span(borrow_state.calls).unwrap();
    assert_eq!(calls.len(), 2);
    let captured_segments = calls
        .iter()
        .map(|call| {
            let accesses = checked
                .facts
                .borrow
                .argument_accesses
                .span(call.accesses)
                .unwrap();
            let loans = accesses
                .iter()
                .filter(|access| access.kind == checked_trees::BorrowAccessKind::Mutable)
                .collect::<Vec<_>>();
            assert_eq!(loans.len(), 1);
            loans[0].segments
        })
        .collect::<Vec<_>>();
    let selector = |span| {
        let [facts::PlaceSegment::Index { expression }] =
            checked.facts.borrow.access_segments.span(span).unwrap()
        else {
            panic!("an immutable local selector retains its source occurrence")
        };
        *expression
    };
    let first = selector(captured_segments[0]);
    let second = selector(captured_segments[1]);
    assert_ne!(first, second);
    // Both selectors fold to the same legal index. Neither equal carrier
    // shape nor equal folded value can replace the captured source occurrence.
    checked
        .facts
        .borrow
        .access_segments
        .span_mut(captured_segments[0])
        .unwrap()[0] = facts::PlaceSegment::Index { expression: second };
    assert!(
        lower_machine(&checked, TerminalMachineSelection::Name("run")).is_err(),
        "a different equally valued local selector cannot replace this occurrence"
    );
}
