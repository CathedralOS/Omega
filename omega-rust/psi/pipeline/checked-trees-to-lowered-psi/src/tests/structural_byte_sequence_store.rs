//! Source admission for whole bounded byte-field replacement.

use super::*;

#[test]
fn bounded_byte_field_literal_replacement_publishes_terminal() {
    for literal in ["XXX", "X", ""] {
        let checked = checked_source(&format!(
            r#"
            domain [u8; 3]::Utf8 requires valid_utf8(self);
            data Record {{ out: [u8; 3] in Utf8; }}
            machine Record::replace(&mut self) {{ self.out = "{literal}"; }}
            "#
        ));
        let artifact = produce_terminal_artifact(&checked, "Record::replace")
            .expect("checked literal replacement publishes verified Terminal");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        terminal_verifier::validate_module(&module).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let OperationKind::StructuralByteSequenceFieldStore { path, field, .. } =
            &entry.blocks[0].operations.last().unwrap().kind
        else {
            panic!("source replacement retains its byte-field store");
        };
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
        let mut completed = false;
        for _ in 0..20 {
            match execution.resume(&mut fuel).unwrap() {
                terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_) => {
                    fuel.replenish(1).unwrap()
                }
                terminal_interpreter::TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, terminal_interpreter::TerminalExecutionResult::Unit);
                    completed = true;
                    break;
                }
                status => panic!("unexpected byte-field execution status: {status:?}"),
            }
        }
        assert!(completed);
        assert_eq!(
            execution.structural_byte_sequence_field(73, path, *field),
            Some(literal.as_bytes())
        );
    }
}

#[test]
fn byte_field_store_receiving_rejects_literal_path_access_and_omission_drift() {
    let source = r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; other: [u8;3] in Utf8; }
        machine Record::replace(&mut self) { self.out = "XXX"; }
    "#;
    let checked = checked_source(source);
    lower_machine(&checked, "Record::replace").expect("untampered byte replacement lowers");
    for mutation in 0..5 {
        let mut changed = checked.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                    )
                })
            })
            .unwrap();
        if mutation == 3 {
            plan.structural_parameters[0].access =
                checked_trees::CheckedStructuralAccess::SharedBorrow;
        } else if mutation == 4 {
            plan.operations.remove(0);
        } else {
            let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) =
                &mut plan.operations[0]
            else {
                panic!("byte store");
            };
            match mutation {
                0 => store.bytes = b"YYY".to_vec(),
                1 => store.field_identity = "Record::other".into(),
                2 => store
                    .carrier_path
                    .push(CheckedUnitStructuralPathSegment::FixedIndex(0)),
                _ => unreachable!(),
            }
        }
        assert!(
            lower_machine(&changed, "Record::replace").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn byte_replacements_preserve_sibling_and_call_order_at_each_fuel_pause() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; other: [u8;3] in Utf8; flag: bool; }
        machine Record::shrink(&mut self) { self.out = "X"; }
        machine Record::run(&mut self) {
            self.other = "QQ";
            self.out = "XXX";
            self.flag = true;
            self.shrink();
            self.out = "";
        }
    "#,
    );
    let artifact = produce_terminal_artifact(&checked, "Record::run")
        .expect("mixed stores and receiver call publish together");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let stores = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            if let OperationKind::StructuralByteSequenceFieldStore { path, field, .. } =
                &operation.kind
            {
                Some((path.clone(), *field))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 3);
    let (sibling_path, field) = &stores[0];
    let (destination_path, destination_field) = &stores[1];
    assert_ne!(field, destination_field);
    assert!(sibling_path.is_empty());
    assert!(destination_path.is_empty());
    let mut execution =
        terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 91,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .unwrap();
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut snapshots = Vec::new();
    let mut completed = false;
    for _ in 0..100 {
        let status = execution.resume(&mut fuel).unwrap();
        let snapshot = (
            execution
                .structural_byte_sequence_field(91, sibling_path, *field)
                .map(<[u8]>::to_vec),
            execution
                .structural_byte_sequence_field(91, destination_path, *destination_field)
                .map(<[u8]>::to_vec),
        );
        if snapshots.last() != Some(&snapshot) {
            snapshots.push(snapshot);
        }
        match status {
            terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_) => {
                // A second attempt with unchanged allowance must commit nothing.
                assert!(matches!(
                    execution.resume(&mut fuel).unwrap(),
                    terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(
                    execution.structural_byte_sequence_field(91, sibling_path, *field),
                    snapshots.last().unwrap().0.as_deref()
                );
                assert_eq!(
                    execution.structural_byte_sequence_field(
                        91,
                        destination_path,
                        *destination_field
                    ),
                    snapshots.last().unwrap().1.as_deref()
                );
                fuel.replenish(1).unwrap();
            }
            terminal_interpreter::TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, terminal_interpreter::TerminalExecutionResult::Unit);
                completed = true;
                break;
            }
            status => panic!("unexpected replacement status: {status:?}"),
        }
    }
    assert!(completed);
    assert_eq!(
        snapshots,
        vec![
            (None, None),
            (Some(b"QQ".to_vec()), None),
            (Some(b"QQ".to_vec()), Some(b"XXX".to_vec())),
            (Some(b"QQ".to_vec()), Some(b"X".to_vec())),
            (Some(b"QQ".to_vec()), Some(Vec::new())),
        ]
    );
}

#[test]
fn bounded_byte_replacements_require_capacity_and_exact_domain_predicate() {
    for (domain, predicate, literal) in [
        ("Utf8", "valid_utf8", "XXXX"),
        ("Utf8", "valid_utf8", r"\xFF"),
        ("Ascii", "ascii_only", "é"),
    ] {
        let source = format!(
            r#"
            domain [u8;3]::{domain} requires {predicate}(self);
            data Record {{ out: [u8;3] in {domain}; }}
            machine Record::replace(&mut self) {{ self.out = "{literal}"; }}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = parse_syntax_trees(&tokens).unwrap();
        let resolved = lower_syntax_trees(&syntax).unwrap();
        let typed = lower_symbol_resolved_trees(&resolved).unwrap();
        assert!(
            lower_typed_trees(typed).is_err(),
            "{domain}: {literal:?} must fail source checking"
        );
    }
}

#[test]
fn nested_record_byte_field_store_retains_its_exact_carrier_path() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        data Holder { record: Record; }
        machine Holder::replace(&mut self) { self.record.out = "XY"; }
    "#,
    );
    let artifact = produce_terminal_artifact(&checked, "Holder::replace").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let OperationKind::StructuralByteSequenceFieldStore { path, field, .. } =
        &entry.blocks[0].operations.last().unwrap().kind
    else {
        panic!("byte store");
    };
    assert!(matches!(path.as_slice(), [StructuralPathSegment::Field(_)]));
    let mut execution =
        terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 101,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .unwrap();
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(20);
    assert!(matches!(
        execution.resume(&mut fuel).unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    ));
    assert_eq!(
        execution.structural_byte_sequence_field(101, path, *field),
        Some(b"XY".as_slice())
    );
    assert_eq!(
        execution.structural_byte_sequence_field(101, &[], *field),
        None
    );
}

#[test]
fn literal_reestablishment_in_a_cycle_consumes_fuel_without_losing_field_bytes() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::replace(&mut self) { self.out = "XXX"; }
    "#,
    );
    let mut lowered = lower_machine(&checked, "Record::replace").unwrap();
    // The store is source-produced. This control-flow rewrite tests repeated
    // Terminal execution, not source correspondence for a fabricated loop.
    let machine = &mut lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 1);
    let condition = value_id(
        machine.blocks[0]
            .operations
            .iter()
            .filter_map(|operation| operation.result.scalar().map(|value| value.id.get()))
            .max()
            .unwrap()
            + 1,
    );
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: condition,
        scalar_type: ScalarType::Boolean,
    });
    let loop_block = block_id(machine.blocks[0].id.get() + 1);
    let exit_block = block_id(loop_block.get() + 1);
    let operations = std::mem::take(&mut machine.blocks[0].operations);
    let terminator = machine.blocks[0].terminator.clone();
    let Terminator::ReturnUnit { edge, .. } = terminator else {
        panic!("source Unit return");
    };
    let next_edge = edge.get() + 1;
    let OperationKind::StructuralByteSequenceFieldStore { field, .. } =
        operations.last().unwrap().kind
    else {
        panic!("byte store");
    };
    machine.blocks[0].terminator = Terminator::Jump {
        edge: edge_id(next_edge),
        target: loop_block,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        id: loop_block,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator: Terminator::Conditional {
            condition,
            when_true: SuccessorEdge {
                edge: edge_id(next_edge + 1),
                target: loop_block,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: SuccessorEdge {
                edge: edge_id(next_edge + 2),
                target: exit_block,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        },
    });
    machine.blocks.push(Block {
        id: exit_block,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    });
    let structural_type = machine.structural_parameters[0].structural_type;
    lowered.proof_bundle.evidence.clear();
    crate::operation_emission::finalize_operation_proofs(&mut lowered).unwrap();
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let mut execution =
        terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &[terminal_interpreter::TerminalScalarValue::Boolean(true)],
            &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 97,
                structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .unwrap();
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observed_write = false;
    for allowance in 0..40 {
        assert!(matches!(
            execution.resume(&mut fuel).unwrap(),
            terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(fuel.usage().total_units(), allowance);
        let bytes = execution.structural_byte_sequence_field(97, &[], field);
        if bytes.is_some() {
            observed_write = true;
        }
        if observed_write {
            assert_eq!(bytes, Some(b"XXX".as_slice()));
        }
        assert!(matches!(
            execution.resume(&mut fuel).unwrap(),
            terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(fuel.usage().total_units(), allowance);
        fuel.replenish(1).unwrap();
    }
    assert!(
        observed_write,
        "repeated literal execution must reach its stores"
    );
}
