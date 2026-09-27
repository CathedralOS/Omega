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
fn indexed_byte_store_rejoins_index_value_field_access_and_complete_roster() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; other: [u8;3] in Utf8; }
        machine Record::replace(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {
            self.out = "XXX"; self.out[position] = byte;
        }
    "#,
    );
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
                1 => {
                    store.value =
                        checked_trees::CheckedByteSequenceStoreValue::Pure(store.index.clone())
                }
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
