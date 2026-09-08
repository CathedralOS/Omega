//! Exact indexed mutation of the supplied mutable byte view.

use super::*;

pub(super) const PUT: &str = r#"
    machine put(out: &mut [u8], byte: u8) {
        transition out.len > 0 {
            true -> store(out, byte)
            false -> done()
        }
        state store(out: &mut [u8], byte: u8) { out[0] = byte; }
        state done() {}
    }
"#;

#[test]
fn fixed_byte_array_lends_mutable_view() {
    let checked = checked_source(&format!(
        "{PUT}\n machine run(out: &mut [u8; 3]) {{ put(out, 65); put(out, 0); }}"
    ));
    let _artifact = produce_terminal_artifact(&checked, "run")
        .expect("a raw fixed byte array lends its exact initialized writable range");
}

#[test]
fn guarded_mutable_byte_view_write_publishes_terminal() {
    let checked = checked_source(PUT);
    let artifact = produce_terminal_artifact(&checked, "put")
        .expect("guarded mutable byte-view write publishes verified Terminal");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::validate_module(&module).unwrap();
}

#[test]
fn byte_view_write_rejects_changed_source_operands_access_and_roster() {
    let checked = checked_source(
        r#"
        machine put(out: &mut [u8], other: &mut [u8], byte: u8) {
            transition out.len > 0 {
                true -> store(out, other, byte)
                false -> done()
            }
            state store(out: &mut [u8], other: &mut [u8], byte: u8) {
                out[0] = byte;
                out[0] = byte;
            }
            state done() {}
        }
    "#,
    );
    let _artifact = produce_terminal_artifact(&checked, "put")
        .expect("lawful two-write source publishes first");
    let plan_index = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .position(|plan| {
            plan.states.iter().any(|state| {
                state.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                    )
                })
            })
        })
        .unwrap();
    let state_index = checked.facts.flow.terminal_unit_effects.composed_machines[plan_index]
        .states
        .iter()
        .position(|state| state.operations.len() == 2)
        .unwrap();
    for mutation in 0..7 {
        let mut changed = checked.clone();
        let state = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index]
            .states[state_index];
        match mutation {
            0 => {
                let CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) =
                    &mut state.operations[0]
                else {
                    panic!("write")
                };
                write.destination_parameter_position = 1;
            }
            1 => {
                let CheckedUnitEffectOperationPlan::ByteSequenceWrite(first) =
                    &mut state.operations[0]
                else {
                    panic!("write")
                };
                first.index = CheckedScalarExpression::IntegerLiteral {
                    literal: numerics::literals::IntegerLiteral::from_value(1).with_landing(
                        numerics::literals::IntegerLanding {
                            landed_type: numerics::literals::LandedIntegerType::U64,
                            domain: numerics::arithmetic::ArithmeticDomain::Exact,
                        },
                    ),
                };
            }
            2 => {
                state.structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            3 => state.operations.push(state.operations[0].clone()),
            4 => {
                state.operations.remove(0);
            }
            5 => state.operations.swap(0, 1),
            6 => {
                let CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) =
                    &mut state.operations[0]
                else {
                    panic!("write")
                };
                write.value = CheckedScalarExpression::IntegerLiteral {
                    literal: numerics::literals::IntegerLiteral::from_value(0).with_landing(
                        numerics::literals::IntegerLanding {
                            landed_type: numerics::literals::LandedIntegerType::U8,
                            domain: numerics::arithmetic::ArithmeticDomain::Exact,
                        },
                    ),
                };
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, "put").is_err(),
            "source custody mutation {mutation}"
        );
    }
}

#[test]
fn guarded_mutable_byte_write_keeps_original_field_extent_and_tail() {
    // This fixture uses the existing checked UTF-8 literal initialization route;
    // it does not establish raw fixed-array or complete read_line support.
    for initial in ["old", ""] {
        let checked = checked_source(&format!(
            r#"
        {PUT}
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Record {{ out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }}
        machine Record::run(&mut self) {{
            self.out = "{initial}";
            self.other = "QQ";
            put(&mut self.out, 65);
            put(&mut self.out, 0);
        }}
    "#
        ));
        let artifact = produce_terminal_artifact(&checked, "Record::run")
            .expect("checked caller retains original borrowed field backing");
        let mut redirected = checked.clone();
        let caller = redirected
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations.iter().any(|operation| {
                    matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                })
            })
            .unwrap();
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        } = caller
            .operations
            .iter_mut()
            .find(|operation| matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. }))
            .unwrap()
        else {
            panic!("ordinary put")
        };
        structural_arguments[0].path =
            vec![CheckedUnitStructuralPathSegment::Field("other".into())];
        assert!(
            lower_machine(&redirected, "Record::run").is_err(),
            "same-typed sibling cannot replace authored output"
        );
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == entry.structural_parameters[0].structural_type)
            .unwrap();
        let StructuralTypeShape::Record {
            fields: declared_fields,
            ..
        } = &declaration.shape
        else {
            panic!("caller record")
        };
        let fields = ["out", "other"].map(|identity| {
            let field = declared_fields
                .iter()
                .find(|field| field.identity == identity)
                .unwrap();
            (Vec::<StructuralPathSegment>::new(), field.id)
        });
        assert_ne!(fields[0].1, fields[1].1);
        let mut execution =
            terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                &[terminal_interpreter::TerminalStructuralValue {
                    opaque_identity: 73,
                    structural_type: entry.structural_parameters[0].structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
            )
            .unwrap();
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
        let mut complete = false;
        for _ in 0..100 {
            let status = execution.resume(&mut fuel).unwrap();
            let before = execution
                .structural_byte_sequence_field(73, &fields[0].0, fields[0].1)
                .map(<[u8]>::to_vec);
            match status {
                terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert!(matches!(
                        execution.resume(&mut fuel).unwrap(),
                        terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
                    ));
                    assert_eq!(
                        execution.structural_byte_sequence_field(73, &fields[0].0, fields[0].1),
                        before.as_deref()
                    );
                    fuel.replenish(1).unwrap();
                }
                terminal_interpreter::TerminalExecutionStatus::Complete(_) => {
                    assert!(execution.effects().is_empty());
                    assert_eq!(
                        before.as_deref(),
                        Some(if initial.is_empty() {
                            initial.as_bytes()
                        } else {
                            b"\0ld".as_slice()
                        })
                    );
                    assert_eq!(
                        execution.structural_byte_sequence_field(73, &fields[1].0, fields[1].1),
                        Some(b"QQ".as_slice())
                    );
                    complete = true;
                    break;
                }
                terminal_interpreter::TerminalExecutionStatus::Crashed(crash) => {
                    panic!("unexpected crash: {crash:?}")
                }
            }
        }
        assert!(
            complete,
            "guarded writes did not complete with incremental fuel"
        );
    }
}
