//! Source-backed indexed replacement within a bounded byte field's live prefix.

use super::{ScalarType, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
use semantic_vocabulary::IntegerValue;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, StructuralTypeShape};

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
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &arguments,
        TerminalStructuralInputs {
            arguments: &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 73,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observed = Vec::<Vec<u8>>::new();
    let mut complete = false;
    for _ in 0..100 {
        let status = execution
            .resume(&mut fuel, &mut AcceptTerminalEffects)
            .unwrap();
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
                    execution
                        .resume(&mut fuel, &mut AcceptTerminalEffects)
                        .unwrap(),
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
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; other: [u8;3] in Utf8; }
        machine Record::replace(&mut self) {
            self.other = "XXX"; self.out = "XXX";
            self.out[0] = 65; self.out[2] = 66;
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::replace"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("sibling byte-field isolation")
    .into_artifact();
    run_stores(&artifact, &[], &[b"", b"XXX", b"AXX", b"AXB"]);
}

#[test]
fn joined_field_guards_preserve_the_stored_bytes_bound() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; byte: u8; }
        machine Record::replace(&mut self, choice: u8, byte: u8) {
            self.out = "XXX";
            transition choice == 0 { true -> supplied(byte) _ -> space() }
            state supplied(&mut self, byte: u8) {
                self.byte = byte;
                transition self.byte < 128 { true -> write() _ -> done() }
            }
            state space(&mut self) {
                self.byte = 32;
                transition self.byte < 128 { true -> write() _ -> done() }
            }
            state write(&mut self) { self.out[1] = self.byte; }
            state done(&mut self) { }
        }
        "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::replace"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("both arrivals prove the observed field is ASCII")
    .into_artifact();
    terminal_codec::verify_terminal_artifact_proof(
        &artifact,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the receiver checks both incoming paths without source");
    run_stores(&artifact, &[0, 65], &[b"", b"XXX", b"XAX"]);
    run_stores(&artifact, &[1, 200], &[b"", b"XXX", b"X X"]);
    run_stores(&artifact, &[0, 200], &[b"", b"XXX"]);

    let mut changed =
        lower_machine(&checked, TerminalMachineSelection::Name("Record::replace")).unwrap();
    let mut changed_guards = 0;
    for operation in changed
        .semantic_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::IntegerConstant { value } = &mut operation.kind
            && *value == IntegerValue::Unsigned(128)
        {
            *value = IntegerValue::Unsigned(255);
            changed_guards += 1;
        }
    }
    assert_eq!(changed_guards, 2, "both authored guards remain explicit");
    assert!(
        terminal_verifier::verify_module(
            &changed.semantic_module,
            &changed.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "a wider guard cannot reuse the original arrival certificates"
    );
    changed.semantic_module.scalar_block_invariants.clear();
    changed.proof_bundle = terminal_verifier::ProofBundle::default();
    assert!(
        crate::proofs::operation_proofs::finalize_operation_proofs(&mut changed).is_err(),
        "candidate discovery must reject an arrival that permits a non-ASCII byte"
    );
}

const REPLACE: &str = r#"
    domain [u8;3]::Utf8 requires valid_utf8(self);
    data Record { out: [u8;3] in Utf8; other: [u8;3] in Utf8; }
    machine Record::replace(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {
        self.out = "XXX"; self.out[position] = byte;
    }
"#;

#[test]
fn indexed_byte_store_rejoins_index_value_field_access_and_complete_roster() {
    let checked = crate::front_end::checked_program(REPLACE);
    lower_machine(&checked, TerminalMachineSelection::Name("Record::replace"))
        .expect("untampered indexed store");
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
                0 => {
                    store.index = store
                        .value
                        .as_pure()
                        .expect("authored store keeps a pure value")
                        .clone()
                }
                1 => store.value = checked_trees::CheckedStoreValue::Pure(store.index.clone()),
                2 => store.field_identity = "Record::other".into(),
                3 => store
                    .carrier_path
                    .push(CheckedUnitStructuralPathSegment::FixedIndex(0)),
                _ => unreachable!(),
            }
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Record::replace")).is_err(),
            "mutation {mutation}"
        );
    }
}

/// Byte leaves share the scalar store vocabulary but lower no computation:
/// replay refuses an indexed byte replacement whose value names one, in the
/// leaf's own wording, instead of treating it as a pure byte.
#[test]
fn indexed_byte_store_refuses_a_computation_value() {
    let mut checked = crate::front_end::checked_program(REPLACE);
    let store = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|plan| &mut plan.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                Some(store)
            }
            _ => None,
        })
        .expect("indexed byte store");
    store.value = checked_trees::CheckedStoreValue::Computation(Default::default());
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Record::replace"))
        .expect_err("a byte replacement has no computation lowering");
    assert!(
        matches!(
            error,
            super::LoweringError::Unsupported("byte replacement kept a computation value")
        ),
        "{error:?}"
    );
}

/// A byte-field element has no local-rooted lowering: replay refuses an
/// indexed byte replacement whose shared store root names a local.
#[test]
fn indexed_byte_store_refuses_a_local_root() {
    let mut checked = crate::front_end::checked_program(REPLACE);
    let store = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|plan| &mut plan.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                Some(store)
            }
            _ => None,
        })
        .expect("indexed byte store");
    store.destination = checked_trees::CheckedStoreRoot::Local {
        symbol: symbols::SymbolHandle::invalid(),
    };
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Record::replace"))
        .expect_err("a byte-field element has no local root");
    assert!(
        matches!(
            error,
            super::LoweringError::Unsupported("byte replacement has no parameter root")
        ),
        "{error:?}"
    );
}

/// The serialized artifact carries the domain-bearing store's evidence: the
/// receiver reconstructs the ASCII leg from the field declaration and
/// verifies the certificate the producer attached. Tampering with the
/// serialized module -- a non-ASCII stored byte, a UTF-8 carrier that is not
/// all-ASCII, or a stripped certificate -- rejects on the receiving side.
#[test]
fn utf8_field_store_verifies_from_its_serialized_artifact_and_rejects_tampering() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::replace(&mut self) {
            self.out = "XXX"; self.out[1] = 65;
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::replace"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("an ASCII byte over an ASCII literal publishes")
    .into_artifact();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_codec::verify_terminal_artifact_proof(&artifact, &profile)
        .expect("the receiver replays the serialized store evidence");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_section_for(&module, artifact.proof_bytes()).unwrap();
    let store = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldByteStore {
                value, obligation, ..
            } => Some((*value, *obligation)),
            _ => None,
        })
        .expect("retained byte store");
    let byte_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap();
    let ascii_leg = semantic_vocabulary::Proposition::LessOrEqual(
        semantic_vocabulary::ScalarTerm::value(store.0, ScalarType::Integer(byte_type)),
        semantic_vocabulary::ScalarTerm::integer(byte_type, IntegerValue::Unsigned(0x7f)).unwrap(),
    );
    let goal = terminal_verifier::reconstruct_operation_obligations(&module)
        .unwrap()
        .into_iter()
        .find(|question| question.obligation.id == store.1)
        .expect("store goal")
        .obligation
        .proposition;
    assert!(
        matches!(&goal, semantic_vocabulary::Proposition::Conjunction(legs)
            if legs.len() == 2 && legs[1] == ascii_leg),
        "{goal:?}"
    );
    for tampering in 0..3 {
        let mut changed = module.clone();
        let mut evidence = proof.clone();
        let operations = changed
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations);
        match tampering {
            0 => {
                for operation in operations {
                    if operation.result.scalar().map(|result| result.id) == Some(store.0)
                        && let OperationKind::IntegerConstant { value } = &mut operation.kind
                    {
                        *value = IntegerValue::Unsigned(200);
                    }
                }
            }
            1 => {
                for operation in operations {
                    if let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
                        &mut operation.kind
                    {
                        *bytes = "\u{e9}X".as_bytes().to_vec();
                    }
                }
            }
            _ => evidence
                .evidence
                .retain(|candidate| candidate.obligation != store.1),
        }
        // Module validation guards both encoding and decoding: a carrier that
        // is not all-ASCII is already an invalid module. The other
        // tamperings serialize and then fail proof replay.
        match terminal_codec::encode_module(&changed)
            .and_then(|bytes| terminal_codec::decode_module(&bytes))
        {
            Ok(decoded) => {
                assert_ne!(tampering, 1, "a non-ASCII UTF-8 carrier decoded");
                assert!(
                    terminal_verifier::verify_module(&decoded, &evidence, &profile).is_err(),
                    "tampering {tampering}"
                );
            }
            Err(error) => {
                assert_eq!(tampering, 1, "{error:?}");
                assert!(format!("{error:?}").contains("InvalidStructuralByteSequenceFieldAccess"));
            }
        }
    }
}
