//! Source-backed indexed replacement within a bounded byte field's live prefix.

use super::*;

#[test]
fn initialized_byte_field_runtime_index_replacement_publishes_terminal() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::replace(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {
            self.out = "XXX";
            self.out[position] = byte;
        }
        "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Record::replace")
        .produce_artifact()
        .expect("checked indexed byte replacement publishes verified Terminal");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::validate_module(&module).unwrap();
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_parameter_ranges(machine);
    run_stores(&artifact, &[2, 65], &[b"", b"XXX", b"XXA"]);
}

#[test]
fn nested_byte_field_runtime_index_preserves_the_static_carrier_path() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        data Envelope { inner: Record; }
        machine Envelope::replace(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {
            self.inner.out = "XXX";
            self.inner.out[position] = byte;
        }
        "#,
    );
    let artifact =
        terminal_production::TerminalProductionRequest::new(&checked, "Envelope::replace")
            .produce_artifact()
            .expect("nested runtime byte replacement publishes verified Terminal");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let path = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldByteStore { path, .. } => Some(path),
            _ => None,
        })
        .unwrap();
    assert!(matches!(path.as_slice(), [StructuralPathSegment::Field(_)]));
    run_stores(&artifact, &[1, 65], &[b"", b"XXX", b"XAX"]);
}

fn assert_parameter_ranges(machine: &terminal_psi::TerminalMachine) {
    assert_eq!(machine.contract.requires.len(), 2);
    for (parameter, maximum) in machine.parameters.iter().zip([2, 127]) {
        let ScalarType::Integer(integer_type) = parameter.scalar_type else {
            panic!("integer parameter");
        };
        let subject = ScalarTerm::Value {
            id: parameter.id,
            scalar_type: parameter.scalar_type,
        };
        let bounds = [
            Proposition::LessOrEqual(
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(0)).unwrap(),
                subject.clone(),
            ),
            Proposition::LessOrEqual(
                subject,
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(maximum)).unwrap(),
            ),
        ];
        assert!(machine.contract.requires.iter().any(|requirement| {
            let Proposition::Conjunction(retained) = requirement else {
                return false;
            };
            retained.len() == 2 && bounds.iter().all(|bound| retained.contains(bound))
        }));
    }
}

#[test]
fn caller_byte_index_range_is_checked_against_the_evaluated_argument() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::replace(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {
            self.out = "XXX"; self.out[position] = byte;
        }
        machine Record::run(&mut self) { self.replace(2, 65); }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Record::run")
        .produce_artifact()
        .expect("bounded caller argument");
    run_stores(&artifact, &[], &[b"", b"XXX", b"XXA"]);
    let mut lowered = lower_machine(&checked, "Record::run").unwrap();
    let caller = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let constant = caller
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(2)
                }
            )
        })
        .unwrap();
    constant.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(3),
    };
    lowered.proof_bundle.evidence.clear();
    assert!(matches!(
        crate::operation_emission::finalize_operation_proofs(&mut lowered),
        Err(LoweringError::OperationProofUnavailable(_))
    ));
}

#[test]
fn indexed_byte_store_rejects_same_type_substitution_duplicate_and_reordering() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::replace(&mut self) {
            self.out = "XXX"; self.out[0] = 65; self.out[2] = 66;
        }
    "#,
    );
    lower_machine(&checked, "Record::replace").expect("untampered ordered stores");
    for mutation in 0..3 {
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
                        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                    )
                })
            })
            .unwrap();
        let positions = plan
            .operations
            .iter()
            .enumerate()
            .filter_map(|(position, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                )
                .then_some(position)
            })
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 2);
        match mutation {
            0 => {
                let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(second) =
                    &plan.operations[positions[1]]
                else {
                    unreachable!()
                };
                let index = second.index.clone();
                let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(first) =
                    &mut plan.operations[positions[0]]
                else {
                    unreachable!()
                };
                first.index = index;
            }
            1 => plan
                .operations
                .insert(positions[0], plan.operations[positions[0]].clone()),
            2 => plan.operations.swap(positions[0], positions[1]),
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, "Record::replace").is_err(),
            "mutation {mutation}"
        );
    }
}

fn run_stores(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    arguments: &[u128],
    expected: &[&[u8]],
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let (path, field) = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldByteStore { path, field, .. } => {
                Some((path.clone(), *field))
            }
            _ => None,
        })
        .expect("retained byte store");
    let sibling = module.structural_types.iter().find_map(|declaration| {
        let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return None;
        };
        fields
            .iter()
            .find(|field| field.identity == "Record::other")
            .map(|field| field.id)
    });
    let arguments = entry
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, value)| {
            let ScalarType::Integer(scalar_type) = parameter.scalar_type else {
                panic!("integer input");
            };
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(*value),
            }
        })
        .collect::<Vec<_>>();
    let mut execution =
        terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &arguments,
            &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 73,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .unwrap();
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observed = Vec::<Vec<u8>>::new();
    let mut complete = false;
    for _ in 0..100 {
        let status = execution.resume(&mut fuel).unwrap();
        let bytes = execution
            .structural_byte_sequence_field(73, &path, field)
            .unwrap_or(&[])
            .to_vec();
        if observed.last() != Some(&bytes) {
            observed.push(bytes.clone());
        }
        if let Some(sibling) = sibling
            && !bytes.is_empty()
        {
            assert_eq!(
                execution.structural_byte_sequence_field(73, &path, sibling),
                Some(b"XXX".as_slice())
            );
        }
        match status {
            terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_) => {
                let sibling_before = sibling.map(|field| {
                    execution
                        .structural_byte_sequence_field(73, &path, field)
                        .unwrap_or(&[])
                        .to_vec()
                });
                assert!(matches!(
                    execution.resume(&mut fuel).unwrap(),
                    terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(
                    execution
                        .structural_byte_sequence_field(73, &path, field)
                        .unwrap_or(&[]),
                    bytes
                );
                assert_eq!(
                    sibling.map(|field| execution
                        .structural_byte_sequence_field(73, &path, field)
                        .unwrap_or(&[])
                        .to_vec()),
                    sibling_before
                );
                fuel.replenish(1).unwrap();
            }
            terminal_interpreter::TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, terminal_interpreter::TerminalExecutionResult::Unit);
                complete = true;
                break;
            }
            status => panic!("unexpected execution status: {status:?}"),
        }
    }
    assert!(complete);
    assert_eq!(
        observed.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn indexed_byte_replacement_preserves_a_separately_initialized_sibling() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; other: [u8;3] in Utf8; }
        machine Record::replace(&mut self) {
            self.other = "XXX"; self.out = "XXX";
            self.out[0] = 65; self.out[2] = 66;
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Record::replace")
        .produce_artifact()
        .expect("sibling byte-field isolation");
    run_stores(&artifact, &[], &[b"", b"XXX", b"AXX", b"AXB"]);
}

#[test]
fn caller_observes_ordered_byte_writes_without_changing_live_length() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::edit(&mut self) { self.out = "XXX"; self.out[0] = 65; self.out[2] = 66; }
        machine Record::run(&mut self) { self.edit(); self.out = "X"; self.out[0] = 67; }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Record::run")
        .produce_artifact()
        .expect("ordered caller byte replacement");
    run_stores(&artifact, &[], &[b"", b"XXX", b"AXX", b"AXB", b"X", b"C"]);
}

#[test]
fn indexed_byte_store_rejoins_index_value_field_access_and_complete_roster() {
    let checked = checked_source(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; other: [u8;3] in Utf8; }
        machine Record::replace(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {
            self.out = "XXX"; self.out[position] = byte;
        }
    "#,
    );
    lower_machine(&checked, "Record::replace").expect("untampered indexed store");
    for mutation in 0..6 {
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
                        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                    )
                })
            })
            .unwrap();
        let position = plan
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                )
            })
            .unwrap();
        if mutation == 4 {
            plan.operations.remove(position);
        } else if mutation == 5 {
            plan.structural_parameters[0].access =
                checked_trees::CheckedStructuralAccess::SharedBorrow;
        } else {
            let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) =
                &mut plan.operations[position]
            else {
                unreachable!()
            };
            match mutation {
                0 => store.index = store.value.clone(),
                1 => store.value = store.index.clone(),
                2 => store.field_identity = "Record::other".into(),
                3 => store
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
