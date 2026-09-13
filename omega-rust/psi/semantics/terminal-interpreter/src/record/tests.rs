use crate::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::*;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::*;
use terminal_verifier::ProofBundle;

mod owned_successor;
fn unit_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(900).unwrap(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(900).unwrap(),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(900).unwrap(),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: BlockId::new(900).unwrap(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(900).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(900).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn scalar(identity: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(identity).unwrap(),
        scalar_type: unsigned(0).scalar_type(),
        qualifications: Default::default(),
    }
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn record_module() -> TerminalModule {
    let mut module = unit_module();
    let record = StructuralTypeId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: record,
        identity: "RuntimeRecord".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: "count".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(unsigned(0).scalar_type()),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine.parameters = vec![scalar(1)];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: PlaceId::new(1).unwrap(),
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(1).unwrap(),
            structural_type: record,
        },
    });
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Structural(StructuralOperationResult {
            place: PlaceId::new(1).unwrap(),
            structural_type: record,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishRecord {
            fields: vec![RecordFieldInitializer {
                field: StructuralFieldId::new(1).unwrap(),
                value: terminal_psi::RecordFieldValue::Scalar {
                    value: ValueId::new(1).unwrap(),
                    range_obligation: None,
                },
            }],
        },
    });
    module
}

fn getter() -> TerminalMachine {
    let mut getter = unit_module().machines.remove(0);
    getter.id = MachineId::new(901).unwrap();
    getter.contract.id = ContractId::new(901).unwrap();
    getter.entry = BlockId::new(901).unwrap();
    getter.blocks[0].id = getter.entry;
    getter.attachment = Some(StructuralTypeId::new(1).unwrap());
    getter.structural_parameters = vec![StructuralParameterDeclaration {
        place: PlaceId::new(11).unwrap(),
        position: 0,
        is_self: true,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    getter.structural_places = vec![StructuralPlaceDeclaration {
        id: PlaceId::new(11).unwrap(),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        },
    }];
    getter.result = TerminalMachineResult::Scalar(scalar(14));
    getter.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: OperationId::new(12).unwrap(),
        result: OperationResult::Scalar(scalar(13)),
        kind: OperationKind::IntegerStructuralField {
            path: Vec::new(),
            source: PlaceId::new(11).unwrap(),
            field: StructuralFieldId::new(1).unwrap(),
        },
    }];
    getter.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(901).unwrap(),
        value: ValueId::new(13).unwrap(),
        cleanup_actions: Vec::new(),
    };
    getter
}

fn getter_call(identity: u64, source: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Scalar(scalar(identity)),
        kind: OperationKind::CallStructuralScalar {
            callee: MachineId::new(901).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: PlaceId::new(source).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }
}

fn run(
    module: &TerminalModule,
    arguments: &[TerminalScalarValue],
) -> (TerminalExecution, TerminalExecutionResult) {
    let semantic = encode_module(module).expect("canonical semantic bytes");
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        arguments,
    )
    .expect("independent decoded Terminal verification");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    loop {
        match execution.resume(&mut meter).expect("record execution") {
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(result) => return (execution, result),
            other => panic!("unexpected record outcome {other:?}"),
        }
    }
}

#[test]
fn constructed_runtime_record_shared_getter_reads_initialized_value() {
    let mut module = record_module();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.push(getter_call(3, 1));
    machine.result = TerminalMachineResult::Scalar(scalar(4));
    machine.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(900).unwrap(),
        value: ValueId::new(3).unwrap(),
        cleanup_actions: Vec::new(),
    };
    module.machines.push(getter());
    assert_eq!(
        run(&module, &[unsigned(41)]).1,
        TerminalExecutionResult::Scalar(unsigned(41))
    );
}

#[test]
fn ordinary_scalar_calls_reenter_constructor_with_fresh_record_identities() {
    let mut module = record_module();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.push(getter_call(3, 1));
    machine.result = TerminalMachineResult::Scalar(scalar(4));
    machine.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(900).unwrap(),
        value: ValueId::new(3).unwrap(),
        cleanup_actions: Vec::new(),
    };
    let mut outer = unit_module().machines.remove(0);
    outer.id = MachineId::new(902).unwrap();
    outer.contract.id = ContractId::new(902).unwrap();
    outer.entry = BlockId::new(902).unwrap();
    outer.blocks[0].id = outer.entry;
    outer.parameters = vec![scalar(91), scalar(92)];
    outer.result = TerminalMachineResult::Scalar(scalar(95));
    outer.blocks[0].operations = [(93, 91), (94, 92)]
        .into_iter()
        .map(|(identity, argument)| Operation {
            static_reach_binding: None,
            id: OperationId::new(identity).unwrap(),
            result: OperationResult::Scalar(scalar(identity)),
            kind: OperationKind::Call {
                callee: MachineId::new(900).unwrap(),
                arguments: vec![ValueId::new(argument).unwrap()],
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        })
        .collect();
    outer.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(902).unwrap(),
        value: ValueId::new(93).unwrap(),
        cleanup_actions: Vec::new(),
    };
    module.entry = outer.id;
    module.machines.extend([getter(), outer]);
    let (execution, result) = run(&module, &[unsigned(41), unsigned(99)]);
    assert_eq!(result, TerminalExecutionResult::Scalar(unsigned(41)));
    let initialized = execution
        .structural_scalar_fields
        .iter()
        .filter(|(field, _)| field.field == StructuralFieldId::new(1).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        initialized.len(),
        2,
        "each invocation constructed a separate referent"
    );
    assert_ne!(initialized[0].0.parent, initialized[1].0.parent);
    assert!(initialized.iter().any(|(_, value)| **value == unsigned(41)));
    assert!(initialized.iter().any(|(_, value)| **value == unsigned(99)));
}

#[test]
fn record_constructor_retains_exact_float_payload_bits() {
    for value in [
        IeeeFloatValue::Binary32(0x8000_0000),
        IeeeFloatValue::Binary32(0x7fc1_2345),
    ] {
        let mut module = record_module();
        let scalar_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        module.machines[0].parameters[0].scalar_type = scalar_type;
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
            panic!("record")
        };
        fields[0].field_type = StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32);
        let runtime = TerminalScalarValue::IeeeFloat(value);
        let (execution, result) = run(&module, &[runtime]);
        assert_eq!(result, TerminalExecutionResult::Unit);
        assert_eq!(
            execution
                .structural_scalar_fields
                .values()
                .copied()
                .collect::<Vec<_>>(),
            vec![runtime]
        );
    }
}

#[test]
fn owned_record_argument_mutation_does_not_change_the_callers_referent() {
    let mut module = record_module();
    let mut mutator = getter();
    mutator.structural_parameters[0].access = StructuralAccess::Owned;
    let mut writer = unit_module().machines.remove(0);
    writer.id = MachineId::new(902).unwrap();
    writer.contract.id = ContractId::new(902).unwrap();
    writer.entry = BlockId::new(902).unwrap();
    writer.blocks[0].id = writer.entry;
    writer.structural_parameters = vec![StructuralParameterDeclaration {
        place: PlaceId::new(21).unwrap(),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    writer.structural_places = vec![StructuralPlaceDeclaration {
        id: PlaceId::new(21).unwrap(),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    writer.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: OperationId::new(21).unwrap(),
            result: OperationResult::Scalar(scalar(21)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(99),
            },
        },
        Operation {
            static_reach_binding: None,
            id: OperationId::new(22).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: PlaceId::new(21).unwrap(),
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
                value: ValueId::new(21).unwrap(),
            },
        },
    ];
    writer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(902).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    mutator.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            id: OperationId::new(11).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: writer.id,
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: PlaceId::new(11).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::WriteOnlyBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    );
    let machine = &mut module.machines[0];
    let mut call = getter_call(3, 1);
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut call.kind
    else {
        panic!("call")
    };
    structural_arguments[0].access = StructuralAccess::Owned;
    machine.blocks[0].operations.push(call);
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(4).unwrap(),
        result: OperationResult::Scalar(scalar(4)),
        kind: OperationKind::IntegerStructuralField {
            path: Vec::new(),
            source: PlaceId::new(1).unwrap(),
            field: StructuralFieldId::new(1).unwrap(),
        },
    });
    machine.result = TerminalMachineResult::Scalar(scalar(5));
    machine.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(900).unwrap(),
        value: ValueId::new(4).unwrap(),
        cleanup_actions: Vec::new(),
    };
    module.machines.extend([mutator, writer]);
    let (execution, result) = run(&module, &[unsigned(41)]);
    assert_eq!(result, TerminalExecutionResult::Scalar(unsigned(41)));
    assert_eq!(
        execution.values[&ValueId::new(3).unwrap()],
        unsigned(99),
        "callee read its changed copy"
    );
}

#[test]
fn scalar_return_nominal_cleanup_preserves_record_locals_until_fuel_is_paid() {
    let mut module = record_module();
    let token = StructuralTypeId::new(2).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: token,
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut cleanup = unit_module().machines.remove(0);
    cleanup.id = MachineId::new(901).unwrap();
    cleanup.attachment = Some(token);
    cleanup.contract.id = ContractId::new(901).unwrap();
    cleanup.entry = BlockId::new(901).unwrap();
    cleanup.blocks[0].id = cleanup.entry;
    cleanup.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(901).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: PlaceId::new(2).unwrap(),
            position: 0,
            is_self: false,
            structural_type: token,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: PlaceId::new(2).unwrap(),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    machine.result = TerminalMachineResult::Scalar(scalar(4));
    machine.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(900).unwrap(),
        value: ValueId::new(1).unwrap(),
        cleanup_actions: vec![TerminalAffineCleanupAction::InvokeNominal(
            NominalAffineCleanup {
                place: PlaceId::new(2).unwrap(),
                structural_type: token,
                cleanup_machine: cleanup.id,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        )],
    };
    module.machines.push(cleanup);
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[unsigned(41)],
        &[crate::TerminalStructuralValue {
            opaque_identity: 77,
            structural_type: token,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .expect("independently verified nominal cleanup and ordinary local");
    let mut meter = TerminalFuelMeter::with_allowance(1);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert!(
        execution
            .structural_values
            .contains_key(&PlaceId::new(1).unwrap()),
        "unpaid edge preserves record binding"
    );
    assert!(
        execution
            .structural_values
            .contains_key(&PlaceId::new(2).unwrap()),
        "unpaid edge preserves nominal owner"
    );
    assert_eq!(execution.live_affine_frontier.len(), 1);
    loop {
        meter.replenish(1).unwrap();
        match execution.resume(&mut meter).unwrap() {
            TerminalExecutionStatus::SponsorExhausted(_) => {}
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Scalar(unsigned(41)));
                break;
            }
            other => panic!("unexpected nominal cleanup result {other:?}"),
        }
    }
    assert!(execution.structural_values.is_empty());
    assert!(execution.live_affine_frontier.is_empty());
}
