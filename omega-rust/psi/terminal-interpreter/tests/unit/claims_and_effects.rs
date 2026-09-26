use super::{
    RecordingHandler, RejectingHandler, claim_id, effect_artifact_sections, effect_module,
    operation_id, structural_type_id, structural_value,
};
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInterpretError, TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    BindingRelevance, ClaimTransfer, CompletionReceipt, EntryClaim, OperationKind,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeDeclaration, StructuralTypeShape,
};
use terminal_verifier::ProofBundle;

#[test]
fn unit_calls_transfer_and_settle_nested_record_field_claims() {
    let mut module = effect_module();
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "test::Pocket".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
                    identity: "#9".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(3)),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "test::Token".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    ]);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("expected record shape")
    };
    fields.push(StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
        identity: "#7".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims[0].path = vec!["#7".into(), "#9".into()];
    }

    let semantic = encode_module(&module).expect("nested field-custody module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = structural_value(47);
    let mut handler = RecordingHandler::default();
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
        &mut handler,
    )
    .expect("verified nested field custody should execute");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert!(matches!(
        &handler.effects[0],
        TerminalEffect::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } if structural_arguments == &[argument]
            && completion_receipts == &[
                CompletionReceipt { claim: claim_id(1), argument_index: 0 },
            ]
    ));
}

#[test]
fn unit_calls_transfer_and_settle_both_sibling_field_claims() {
    let mut module = effect_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "test::Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("expected record shape")
    };
    fields.extend([
        StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
            identity: "#7".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(2)),
        },
        StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(2).expect("field identity"),
            identity: "#9".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(2)),
        },
    ]);
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims[0].path = vec!["#7".into()];
        machine.entry_claims.push(EntryClaim {
            claim: claim_id(2),
            input: machine.structural_parameters[0].place,
            path: vec!["#9".into()],
        });
    }
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers.push(ClaimTransfer {
        claim: claim_id(2),
        argument_index: 0,
    });
    let OperationKind::BoundaryCall {
        completion_receipts,
        ..
    } = &mut module.machines[1].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    completion_receipts.push(CompletionReceipt {
        claim: claim_id(2),
        argument_index: 0,
    });

    let semantic = encode_module(&module).expect("sibling field-custody module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = structural_value(46);
    let mut handler = RecordingHandler::default();
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
        &mut handler,
    )
    .expect("verified sibling field custody should execute");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert!(matches!(
        &handler.effects[0],
        TerminalEffect::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } if structural_arguments == &[argument]
            && completion_receipts == &[
                CompletionReceipt { claim: claim_id(1), argument_index: 0 },
                CompletionReceipt { claim: claim_id(2), argument_index: 0 },
            ]
    ));
}

#[test]
fn sponsor_exhaustion_does_not_replay_unit_calls_or_accepted_effects() {
    let (semantic, proof) = effect_artifact_sections();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural_value(42)],
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(3);
    let mut handler = RecordingHandler::default();

    assert_eq!(
        execution.resume(&mut meter, &mut handler).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Operation(operation_id(2)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(handler.effects.len(), 1);
    assert!(matches!(
        handler.effects[0],
        TerminalEffect::BoundaryCall { .. }
    ));

    meter.replenish(2).unwrap();
    assert_eq!(
        execution.resume(&mut meter, &mut handler).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(handler.effects.len(), 2);
    assert_eq!(execution.effects(), handler.effects);
    assert_eq!(meter.usage().total_units(), 5);
}

#[test]
fn structural_runtime_mismatches_and_effect_rejection_fail_closed() {
    let (semantic, proof) = effect_artifact_sections();
    let missing_qualification = TerminalStructuralValue {
        opaque_identity: 43,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    assert!(matches!(
        TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[missing_qualification],
                ..Default::default()
            }
        ),
        Err(
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralQualificationMissing(_)
            )
        )
    ));

    let mut rejecting = RejectingHandler;
    assert!(matches!(
        interpret_terminal_artifact_measured(&semantic, &proof, &AdmissionProfile::default(), &[], TerminalStructuralInputs { arguments: &[structural_value(44)], ..Default::default() }, &mut rejecting),
        Err(terminal_interpreter::TerminalArtifactInterpretError::Execution(
            TerminalInterpretError::EffectRejected { operation, .. }
        )) if operation == operation_id(3)
    ));

    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural_value(44)],
            ..Default::default()
        },
    )
    .expect("verified effect artifact should start");
    let mut meter = TerminalFuelMeter::unbounded();
    assert!(matches!(
        execution.resume(&mut meter, &mut rejecting),
        Err(TerminalInterpretError::EffectRejected { operation, .. })
            if operation == operation_id(3)
    ));
    assert!(execution.effects().is_empty());
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        [claim_id(1)]
    );
}
