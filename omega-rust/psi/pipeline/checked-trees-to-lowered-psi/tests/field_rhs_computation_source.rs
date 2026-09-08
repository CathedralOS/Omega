//! Call-bearing receiver assignments through source-produced Terminal artifacts.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralValue,
};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize field RHS source");
    let syntax = parse_syntax_trees(&tokens).expect("parse field RHS source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve field RHS source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type field RHS source");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check field RHS source")
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn unsigned_wrapping_conversion_helper_calls_preserve_modular_boundaries() {
    for (source_bits, target_bits) in [(16, 8), (32, 8), (32, 16), (64, 32), (8, 8), (8, 16)] {
        let source = format!(
            "boundary trait Trace {{ machine observe(value: u{target_bits}) reaches Trace; }}
             data Main {{ value: u{target_bits}; }}
             machine convert(value: u{source_bits}) -> u{target_bits} {{ (value as u{target_bits} in Wrapping) as u{target_bits} }}
             machine Main::main(&mut self, input: u{source_bits}) reaches Trace {{
                 self.value = convert(input);
                 Trace::observe(self.value);
             }}"
        );
        let source_maximum = (1_u128 << source_bits) - 1;
        let target_maximum = (1_u128 << target_bits) - 1;
        let mut inputs = vec![0, 1, source_maximum];
        if target_bits < source_bits {
            inputs.extend([target_maximum - 1, target_maximum, target_maximum + 1]);
        }
        for input in inputs {
            execute(
                &source,
                &[TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, source_bits).unwrap(),
                    value: IntegerValue::Unsigned(input),
                }],
                &[TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, target_bits).unwrap(),
                    value: IntegerValue::Unsigned(input & target_maximum),
                }],
                false,
                1,
            );
        }
    }
}

#[derive(Default)]
struct FieldTrace(Vec<TerminalScalarValue>);

impl TerminalEffectHandler for FieldTrace {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("only authored field observations are expected")
        };
        assert_eq!(arguments.len(), 1);
        self.0.push(arguments[0]);
        Ok(())
    }
}

fn execute(
    source: &str,
    arguments: &[TerminalScalarValue],
    expected_trace: &[TerminalScalarValue],
    crashes: bool,
    expected_stores: u64,
) {
    let checked = checked(source);
    let artifact = terminal_production::produce_terminal_artifact(&checked, "Main::main")
        .expect("field RHS computations publish");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let receiver = &entry.structural_parameters[0];
    let receiver_type = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == receiver.structural_type)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields } = &receiver_type.shape else {
        panic!("field RHS fixture receiver is a record")
    };
    let boolean_fields: Vec<_> = fields
        .iter()
        .filter(|field| {
            field.field_type
                == terminal_psi::StructuralFieldType::Scalar(
                    semantic_vocabulary::ScalarType::Boolean,
                )
        })
        .map(
            |field| terminal_interpreter::TerminalStructuralBooleanFieldValue {
                argument_index: 0,
                path: Vec::new(),
                field: field.id,
                value: false,
            },
        )
        .collect();
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_boolean_fields(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            arguments,
            &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &boolean_fields,
        )
        .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut trace = FieldTrace::default();
    let status = loop {
        let status = execution
            .resume_with_effect_handler(&mut meter, &mut trace)
            .unwrap();
        if !matches!(status, TerminalExecutionStatus::SponsorExhausted(_)) {
            break status;
        }
        assert!(
            expected_trace.starts_with(&trace.0),
            "observations preserve the executed prefix"
        );
        let prefix = trace.0.clone();
        let usage = meter.usage().clone();
        for _ in 0..2 {
            assert!(matches!(
                execution
                    .resume_with_effect_handler(&mut meter, &mut trace)
                    .unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert_eq!(trace.0, prefix);
            assert_eq!(meter.usage(), &usage);
        }
        assert!(
            meter.usage().total_units() < 1000,
            "bounded field RHS fixture completes"
        );
        meter.replenish(1).unwrap();
    };
    if crashes {
        assert!(
            matches!(status, TerminalExecutionStatus::Crashed(crash) if crash.cause == terminal_psi::CrashCause::Abort)
        );
    } else {
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
    }
    assert_eq!(trace.0, expected_trace);
    let stores: u64 = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::StructuralScalarFieldStore { .. }
            )
        })
        .filter_map(|operation| {
            meter
                .usage()
                .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
        })
        .map(|charge| charge.executions())
        .sum();
    assert_eq!(
        stores, expected_stores,
        "a field store commits only after its complete RHS"
    );
}

#[test]
fn nested_and_cast_wrapped_field_rhs_reads_prewrite_value() {
    execute(
        r#"
        boundary trait Trace { machine observe(value: u16) reaches Trace; }
        data Main { value: u16; }
        machine identity(value: u16) -> u16 { value }
        machine replacement() -> u16 { 19 }
        machine Main::main(&mut self) reaches Trace {
            self.value = 17;
            self.value = identity(identity(self.value)) as u16;
            Trace::observe(self.value);
            self.value = replacement();
            Trace::observe(self.value);
        }
    "#,
        &[],
        &[unsigned(17), unsigned(19)],
        false,
        3,
    );
}

#[test]
fn pure_and_computed_field_rhs_observe_mutated_primitive_local() {
    execute(
        r#"
        boundary trait Trace { machine observe(value: u16) reaches Trace; }
        data Main { value: u16; }
        machine identity(value: u16) -> u16 { value }
        machine reset(value: &mut u16) -> u16 { value = 19; 19 }
        machine Main::main(&mut self) reaches Trace {
            let mut scratch: u16 = 17;
            self.value = identity(scratch);
            Trace::observe(self.value);
            let returned: u16 = reset(&mut scratch);
            self.value = scratch;
            Trace::observe(self.value);
            self.value = identity(scratch);
            Trace::observe(self.value);
        }
        "#,
        &[],
        &[unsigned(17), unsigned(19), unsigned(19)],
        false,
        3,
    );
}

#[test]
fn backedge_field_rhs_calls_read_each_previous_replacement() {
    execute(
        r#"
        boundary trait Trace { machine observe(value: u16) reaches Trace; }
        data Main { value: u16 in Wrapping; }
        machine identity(value: u16) -> u16 { value }
        machine Main::main(&mut self) reaches Trace {
            self.value = 0;
            transition { _ -> step() }
            state step(&mut self) {
                self.value = identity(self.value + 1) as u16 in Wrapping;
                Trace::observe(self.value);
                transition self.value < 3 { true -> step() _ -> done() }
            }
            state done(&mut self) {}
        }
    "#,
        &[],
        &[unsigned(1), unsigned(2), unsigned(3)],
        false,
        4,
    );
}

#[test]
fn wrapping_narrowing_field_rhs_retains_input_width_and_output_policy() {
    let source = r#"
        boundary trait Trace { machine observe(value: u8) reaches Trace; }
        data Main { ch: u32 in Wrapping; byte: u8 in Wrapping; }
        machine narrow_u32_to_u8_wrapping(value: u32) -> u8 {
            (value as u8 in Wrapping) as u8
        }
        machine Main::main(&mut self, offset: u32) reaches Trace {
            self.ch = offset;
            self.ch = 48 + self.ch;
            self.byte = narrow_u32_to_u8_wrapping(self.ch as u32) as u8 in Wrapping;
            Trace::observe(self.byte as u8);
        }
    "#;
    // The helper body is the ordinary core numeric conversion implementation.
    for (offset, expected) in [(5, 53), (260, 52), (u32::MAX, 47)] {
        execute(
            source,
            &[TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                value: IntegerValue::Unsigned(u128::from(offset)),
            }],
            &[TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(expected),
            }],
            false,
            3,
        );
    }
}

#[test]
fn selected_crashing_rhs_never_commits_its_field_store() {
    let source = r#"
        boundary trait Trace { machine observe(value: bool) reaches Trace; }
        data Main { value: bool; }
        machine abort() -> bool crashes Abort { crash Abort; }
        machine Main::main(&mut self, fail: bool) reaches Trace crashes Abort {
            self.value = true;
            Trace::observe(self.value);
            self.value = fail && abort();
            Trace::observe(self.value);
        }
    "#;
    execute(
        source,
        &[TerminalScalarValue::Boolean(false)],
        &[
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
        ],
        false,
        2,
    );
    execute(
        source,
        &[TerminalScalarValue::Boolean(true)],
        &[TerminalScalarValue::Boolean(true)],
        true,
        1,
    );
}

#[test]
fn field_rhs_computation_custody_rejects_destination_root_and_call_substitution() {
    use checked_trees::{
        CheckedScalarComputationKind, CheckedScalarExpressionRole,
        CheckedStructuralScalarFieldStoreValue, CheckedUnitEffectOperationPlan,
    };
    let checked = checked(
        r#"
        data Main { value: u16; other: u16; }
        machine identity(value: u16) -> u16 { value }
        machine Main::main(&mut self) {
            self.value = 17;
            self.other = 19;
            self.value = identity(self.value);
            self.other = identity(self.other);
        }
    "#,
    );
    checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect("unmodified source computation custody is valid");
    for mutation in 0..4 {
        let mut changed = checked.clone();
        let stores: Vec<_> = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .flat_map(|machine| &machine.operations)
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                    match store.value {
                        CheckedStructuralScalarFieldStoreValue::Computation(handle) => Some(handle),
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect();
        assert_eq!(stores.len(), 2);
        match mutation {
            0 | 1 => {
                let store = changed.facts.flow.terminal_unit_effects.machines.iter_mut()
                    .flat_map(|machine| &mut machine.operations)
                    .find_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)
                            if matches!(store.value, CheckedStructuralScalarFieldStoreValue::Computation(handle) if handle == stores[0]) => Some(store),
                        _ => None,
                    }).unwrap();
                if mutation == 0 {
                    store.field_identity = "other".into();
                } else {
                    store.value = CheckedStructuralScalarFieldStoreValue::Computation(stores[1]);
                }
            }
            2 => {
                let plans = &mut changed.facts.values.scalar_computations;
                let root = plans
                    .roots
                    .iter()
                    .find(|(_, root)| root.root == stores[0])
                    .unwrap()
                    .0;
                plans.roots.get_mut(root).role = CheckedScalarExpressionRole::Return;
            }
            3 => {
                let node = changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(stores[0]);
                let CheckedScalarComputationKind::Call { call_ordinal, .. } = &mut node.kind else {
                    panic!("direct call is the assignment computation root")
                };
                *call_ordinal += 1;
            }
            _ => unreachable!(),
        }
        let result =
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").map(|_| ());
        assert!(
            result.is_err(),
            "field RHS custody mutation {mutation} must reject: {result:?}"
        );
    }
}

#[test]
fn receiver_field_rhs_call_reaches_canonical_terminal() {
    let source = r#"
        data Main { value: u16; }
        machine identity(value: u16) -> u16 { value }
        machine Main::main(&mut self) {
            self.value = 17;
            self.value = identity(self.value);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize field RHS call");
    let syntax = parse_syntax_trees(&tokens).expect("parse field RHS call");
    let resolved = lower_syntax_trees(&syntax).expect("resolve field RHS call");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type field RHS call");
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check field RHS call");
    let artifact = terminal_production::produce_terminal_artifact(&checked, "Main::main")
        .expect("call-bearing field RHS must reach canonical Terminal");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("reloaded field RHS artifact verifies independently");
}
