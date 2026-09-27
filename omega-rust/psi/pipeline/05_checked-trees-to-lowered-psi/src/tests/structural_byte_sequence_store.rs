//! Source admission for whole bounded byte-field replacement.

use super::lower_machine;
use crate::TerminalMachineSelection;
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
use semantic_vocabulary::IntegerValue;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;
#[test]
fn replaced_byte_field_length_reaches_canonical_interpretation() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        boundary trait Output { machine size(value: u64) reaches Output; }
        data Payload { out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }
        data Record { payload: Payload; }
        machine Record::shrink(&mut self) { self.payload.out = "X"; }
        machine Record::measure(&mut self) reaches Output {
            self.payload.other = "QQ";
            self.payload.out = "XXX";
            Output::size(self.payload.out.len);
            self.shrink();
            Output::size(self.payload.out.len);
            self.payload.out = "";
            Output::size(self.payload.out.len);
        }
        "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::measure"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("live field length publishes canonical Terminal after replacement")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
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
    #[derive(Default)]
    struct LengthTrace(Vec<u128>);
    impl terminal_interpreter::TerminalEffectHandler for LengthTrace {
        fn handle_effect(
            &mut self,
            effect: &terminal_interpreter::TerminalEffect,
        ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
            let terminal_interpreter::TerminalEffect::BoundaryCall { arguments, .. } = effect
            else {
                panic!("only length boundaries are observable");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(length),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("exact unsigned length argument");
            };
            self.0.push(*length);
            Ok(())
        }
    }
    let mut trace = LengthTrace::default();
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(100);
    let result = execution.resume(&mut fuel, &mut trace).unwrap();
    assert_eq!(
        result,
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
    assert_eq!(trace.0, [3, 1, 0]);
    let (path, sibling) = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldStore { path, field, .. } => {
                Some((path, *field))
            }
            _ => None,
        })
        .expect("first replacement initializes the sibling");
    assert_eq!(
        execution.structural_byte_sequence_field(73, path, sibling),
        Some(b"QQ".as_slice())
    );
}

#[test]
fn byte_field_store_receiving_rejects_literal_path_access_and_omission_drift() {
    let source = r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; other: [u8;3] in Utf8; }
        machine Record::replace(&mut self) { self.out = "XXX"; }
    "#;
    let checked = crate::front_end::checked_program(source);
    lower_machine(&checked, TerminalMachineSelection::Name("Record::replace"))
        .expect("untampered byte replacement lowers");
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
            lower_machine(&changed, TerminalMachineSelection::Name("Record::replace")).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn byte_replacements_preserve_sibling_and_call_order_at_each_fuel_pause() {
    let checked = crate::front_end::checked_program(
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
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("mixed stores and receiver call publish together")
    .into_artifact();
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
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 91,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut snapshots = Vec::new();
    let mut completed = false;
    for _ in 0..100 {
        let status = execution
            .resume(&mut fuel, &mut AcceptTerminalEffects)
            .unwrap();
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
                    execution
                        .resume(&mut fuel, &mut AcceptTerminalEffects)
                        .unwrap(),
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
