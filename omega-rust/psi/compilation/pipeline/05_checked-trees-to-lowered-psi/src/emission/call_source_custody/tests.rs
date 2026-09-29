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
        crate::LoweringError::Unsupported("owned call argument changed its authored parameter")
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
