use crate::effects::AcceptTerminalEffects;
use crate::execution::TerminalExecution;
use crate::results::{TerminalExecutionResult, TerminalExecutionStatus};
use crate::structural_inputs::TerminalStructuralInputs;
use crate::values::TerminalScalarValue;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::*;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::*;
use terminal_verifier::ProofBundle;

mod owned_successor;

fn unit_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
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
        suspension_crossing: None,
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
        suspension_crossing: None,
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Scalar(scalar(identity)),
        kind: OperationKind::CallStructuralScalar {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
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
    let proof = encode_proof_section(module, &ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        arguments,
        TerminalStructuralInputs::default(),
    )
    .expect("independent decoded Terminal verification");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    loop {
        match execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("record execution")
        {
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(result) => return (execution, result),
            other => panic!("unexpected record outcome {other:?}"),
        }
    }
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
            suspension_crossing: None,
            id: OperationId::new(identity).unwrap(),
            result: OperationResult::Scalar(scalar(identity)),
            kind: OperationKind::Call {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
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
