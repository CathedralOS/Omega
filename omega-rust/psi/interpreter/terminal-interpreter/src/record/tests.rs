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
fn owned_byte_copy_and_record_execution_publish_complete_snapshots() {
    let mut module = record_module();
    let carrier = StructuralTypeId::new(2).unwrap();
    let record = StructuralTypeId::new(3).unwrap();
    let domain = StructuralDomainId::new(1).unwrap();
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: carrier,
            identity: "bytes8".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 8,
            }),
        },
        StructuralTypeDeclaration {
            id: record,
            identity: "named-count".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "name".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::PredicateByteSequence {
                            carrier: ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                            predicates,
                        },
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "count".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(unsigned(0).scalar_type()),
                    },
                ],
            },
        },
    ]);
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain,
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: "Utf8".into(),
        carrier,
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    });
    let (mut execution, _) = run(&module, &[unsigned(7)]);
    // This execution-unit control supplies a completed activation value.
    // Source-free verifier tests and the corpus cover admission of its producer.
    let source = PlaceId::new(20).unwrap();
    let copied = PlaceId::new(21).unwrap();
    let destination = PlaceId::new(22).unwrap();
    execution.structural_values.insert(
        source,
        crate::values::TerminalStructuralValue {
            opaque_identity: 9000,
            structural_type: carrier,
            qualifications: vec![domain],
            path: Vec::new(),
        },
    );
    execution.byte_sequence_values.insert(
        source,
        crate::byte_sequences::binding::ByteSequenceBinding::Immutable(
            crate::byte_sequences::ByteSequenceView::new(b"bat".to_vec()),
        ),
    );
    let result = StructuralOperationResult {
        place: copied,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![domain],
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    execution
        .execute_structural_leaf_copy(&Operation {
            id: OperationId::new(50).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            kind: OperationKind::StructuralLeafCopy {
                source,
                path: Vec::new(),
            },
            result: OperationResult::Structural(result),
        })
        .expect("copy snapshots existing byte value");
    let crate::byte_sequences::binding::ByteSequenceBinding::Immutable(bytes) =
        execution.byte_sequence_values.get_mut(&source).unwrap()
    else {
        panic!("owned bytes");
    };
    bytes.replace_byte(0, b'r').unwrap();
    let result = StructuralOperationResult {
        place: destination,
        structural_type: record,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let mut fields = vec![
        RecordFieldInitializer {
            field: StructuralFieldId::new(1).unwrap(),
            value: RecordFieldValue::Structural(StructuralArgument {
                place: copied,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }),
        },
        RecordFieldInitializer {
            field: StructuralFieldId::new(2).unwrap(),
            value: RecordFieldValue::Scalar {
                value: ValueId::new(99).unwrap(),
                range_obligation: None,
            },
        },
    ];
    let snapshot = |execution: &TerminalExecution| {
        execution
            .structural_byte_sequence_fields
            .iter()
            .map(|(field, bytes)| (field.clone(), bytes.bytes().to_vec()))
            .collect::<Vec<_>>()
    };
    let before = snapshot(&execution);
    execution
        .establish_record(&result, &fields)
        .expect_err("unavailable later operand cannot publish earlier byte field");
    assert!(!execution.structural_values.contains_key(&destination));
    assert_eq!(snapshot(&execution), before);
    execution
        .values
        .insert(ValueId::new(99).unwrap(), unsigned(7));
    execution
        .establish_record(&result, &fields)
        .expect("all complete operands publish together");
    let value = &execution.structural_values[&destination];
    assert_eq!(
        execution.structural_byte_sequence_field(
            value.opaque_identity,
            &[],
            StructuralFieldId::new(1).unwrap()
        ),
        Some(b"bat".as_slice())
    );
    assert_eq!(
        execution.byte_sequence_values[&source]
            .immutable()
            .unwrap()
            .bytes(),
        b"rat"
    );
    fields[0].value = RecordFieldValue::Structural(StructuralArgument {
        place: source,
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    });
    execution
        .establish_record(&result, &fields)
        .expect_err("constructor needs owned child access");
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

#[test]
fn zeroed_mixed_fields_keep_common_and_payload_namespaces_separate() {
    let mut module = record_module();
    module.machines[0].parameters.clear();
    module.machines[0].blocks[0].operations[0].kind = OperationKind::EstablishZeroedStructural {};
    let common = StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "count".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(unsigned(0).scalar_type()),
    };
    let mut payload = common.clone();
    payload.identity = "payload".into();
    module.structural_types[0].shape = StructuralTypeShape::Mixed {
        fields: vec![common],
        cases: vec![StructuralCaseDeclaration {
            id: StructuralCaseId::new(1).unwrap(),
            identity: "First".into(),
            fields: vec![payload],
        }],
    };

    let (execution, result) = run(&module, &[]);
    assert_eq!(result, TerminalExecutionResult::Unit);
    let cells = &execution.structural_scalar_fields;
    assert_eq!(cells.len(), 2);
    let parent = cells.keys().next().unwrap().parent.clone();
    let field = StructuralFieldId::new(1).unwrap();
    assert_eq!(
        cells.get(&crate::values::StructuralScalarRuntimeField::member(
            parent.clone(),
            field
        )),
        Some(&unsigned(0)),
    );
    assert_eq!(
        cells.get(&crate::values::StructuralScalarRuntimeField::case_payload(
            parent, field
        )),
        Some(&unsigned(0)),
    );
}

#[test]
fn zeroed_affine_record_retains_its_disposal_until_the_exit() {
    let mut module = record_module();
    let machine = &mut module.machines[0];
    machine.parameters.clear();
    let operation = &mut machine.blocks[0].operations[0];
    operation.kind = OperationKind::EstablishZeroedStructural {};
    let OperationResult::Structural(result) = &mut operation.result else {
        panic!("record result");
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    let disposal = StructuralAffineDiscard {
        place: result.place,
        path: Vec::new(),
        structural_type: result.structural_type,
    };
    machine.blocks[0].operations.extend([
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Scalar(scalar(2)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(42),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(3).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: disposal.place,
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
                value: ValueId::new(2).unwrap(),
                range_obligation: None,
            },
        },
    ]);
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        panic!("Unit exit");
    };
    trivial_affine_discards.push(disposal.place);
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(1);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution
            .live_affine_frontier()
            .cloned()
            .collect::<Vec<_>>(),
        [disposal.clone()]
    );
    assert_eq!(
        execution
            .structural_scalar_fields
            .values()
            .copied()
            .collect::<Vec<_>>(),
        [unsigned(0)]
    );
    meter.replenish(2).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution
            .structural_scalar_fields
            .values()
            .copied()
            .collect::<Vec<_>>(),
        [unsigned(42)]
    );
    assert_eq!(
        execution
            .live_affine_frontier()
            .cloned()
            .collect::<Vec<_>>(),
        [disposal]
    );
    meter.replenish(100).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.live_affine_frontier().count(), 0);
}

#[test]
fn stored_descriptor_leaf_copy_keeps_its_backing_window_after_replacement() {
    use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
    use crate::values::{
        StructuralByteSequenceRuntimeField, StructuralRuntimePlace, TerminalStructuralValue,
    };
    let mut module = unit_module();
    let holder = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: holder,
            identity: "Holder".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "text".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::SharedBorrow),
                        },
                    ),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "[u8]".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        },
    ];
    let (mut execution, _) = run(&module, &[]);
    let source = PlaceId::new(20).unwrap();
    let destination = PlaceId::new(21).unwrap();
    let root = TerminalStructuralValue {
        opaque_identity: 9000,
        structural_type: holder,
        qualifications: vec![],
        path: vec![],
    };
    let cell = StructuralByteSequenceRuntimeField {
        parent: StructuralRuntimePlace::from(&root),
        field,
    };
    let backing = ByteSequenceView::new(b"xabcz".to_vec());
    let window = backing.subslice(1, 4).unwrap();
    execution.structural_values.insert(source, root);
    execution
        .structural_byte_sequence_fields
        .insert(cell.clone(), window.clone());
    let path = vec![StructuralPathSegment::Field("text".into())];
    assert!(
        crate::custody::resolve_structural_path_type(
            &execution.program.structural_types,
            holder,
            &path,
        )
        .is_err(),
        "ordinary storage projection must not invent a descriptor carrier"
    );
    // Full source-derived lowering/verification covers admission. This owning
    // execution control checks the actual descriptor payload and replacement.
    execution
        .execute_structural_leaf_copy(&Operation {
            id: OperationId::new(50).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(StructuralOperationResult {
                place: destination,
                structural_type: view,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![],
                projected_qualifications: vec![],
                qualification_establishments: vec![],
                claims: vec![],
            }),
            kind: OperationKind::StructuralLeafCopy { source, path },
        })
        .unwrap();
    let ByteSequenceBinding::Immutable(copied) = &execution.byte_sequence_values[&destination]
    else {
        panic!("shared descriptor")
    };
    assert_eq!(copied.bytes(), b"abc");
    assert_eq!(
        copied.bytes().as_ptr(),
        window.bytes().as_ptr(),
        "descriptor copy shares exact backing and offset"
    );
    execution
        .structural_byte_sequence_fields
        .insert(cell, ByteSequenceView::new(b"new".to_vec()));
    assert_eq!(
        execution.byte_sequence_values[&destination]
            .immutable()
            .unwrap()
            .bytes(),
        b"abc"
    );
}

#[test]
fn admitted_case_descriptor_copy_uses_the_current_payload_cell() {
    use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
    use crate::structural_inputs::case_membership::StructuralCaseContents;
    use crate::values::{
        StructuralByteSequenceRuntimeField, StructuralRuntimePlace, TerminalStructuralValue,
    };
    let mut module = unit_module();
    let sum = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let case = StructuralCaseId::new(1).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    let source = PlaceId::new(20).unwrap();
    let destination = PlaceId::new(21).unwrap();
    let producer = OperationId::new(50).unwrap();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: sum,
            identity: "Envelope".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![StructuralCaseDeclaration {
                    id: case,
                    identity: "Text".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: "payload".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(view),
                    }],
                }],
            },
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "[u8]".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        },
    ];
    let operation = Operation {
        id: producer,
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: destination,
            structural_type: view,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
            qualification_establishments: vec![],
            claims: vec![],
        }),
        kind: OperationKind::StructuralCaseLeafCopy {
            source,
            path: vec![
                CanonicalStructuralPathSegment::Case(case),
                CanonicalStructuralPathSegment::Field(field),
            ],
        },
    };
    // Establish that this is an admitted structural-child case projection,
    // not a new inline-byte case grammar invented by the execution path.
    let mut admitted = module.clone();
    let machine = &mut admitted.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 0,
            is_self: false,
            structural_type: sum,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: vec![],
            projected_qualifications: vec![],
        });
    machine.structural_places.extend([
        StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: destination,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: view,
            },
        },
    ]);
    machine
        .contract
        .requires
        .push(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(source, vec![]),
            case,
        });
    machine.blocks[0].operations.push(operation.clone());
    terminal_verifier::validate_module(&admitted)
        .expect("structural descriptor child is an admitted case-copy endpoint");

    let (mut execution, _) = run(&module, &[]);
    let root = TerminalStructuralValue {
        opaque_identity: 9100,
        structural_type: sum,
        qualifications: vec![],
        path: vec![],
    };
    let storage = StructuralRuntimePlace::from(&root);
    execution.structural_values.insert(source, root);
    execution.structural_cases.insert(
        storage.clone(),
        StructuralCaseContents {
            structural_type: sum,
            case,
        },
    );
    execution.structural_byte_sequence_fields.insert(
        StructuralByteSequenceRuntimeField {
            parent: storage.clone(),
            field,
        },
        ByteSequenceView::new(b"case".to_vec()),
    );
    execution.execute_structural_leaf_copy(&operation).unwrap();
    let ByteSequenceBinding::Immutable(bytes) = &execution.byte_sequence_values[&destination]
    else {
        panic!("shared descriptor")
    };
    assert_eq!(bytes.bytes(), b"case");
    execution.structural_values.remove(&destination);
    execution.byte_sequence_values.remove(&destination);
    execution.structural_cases.get_mut(&storage).unwrap().case = StructuralCaseId::new(2).unwrap();
    assert!(
        execution.execute_structural_leaf_copy(&operation).is_err(),
        "another discriminator cannot select the same-spelled payload cell"
    );
}

#[test]
fn stored_element_descriptor_keeps_window_across_borrow_retirement_and_owner_disposal() {
    use crate::element_views::ElementView;
    use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
    let mut module = unit_module();
    let holder = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let element = StructuralTypeId::new(3).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: holder,
            identity: "Holder".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "values".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(view),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "view".into(),
            shape: StructuralTypeShape::ElementView { element },
        },
        StructuralTypeDeclaration {
            id: element,
            identity: "element".into(),
            shape: StructuralTypeShape::PrimitiveScalar(unsigned(0).scalar_type()),
        },
    ];
    let (mut execution, _) = run(&module, &[]);
    let source = PlaceId::new(20).unwrap();
    let owner = PlaceId::new(21).unwrap();
    let borrowed = PlaceId::new(22).unwrap();
    let window = ElementView::new(vec![unsigned(7), unsigned(11), unsigned(13), unsigned(17)])
        .subslice(1, 3, StructuralAccess::SharedBorrow)
        .unwrap();
    execution.structural_values.insert(
        source,
        TerminalStructuralValue {
            opaque_identity: 9000,
            structural_type: view,
            qualifications: vec![],
            path: vec![],
        },
    );
    execution.element_view_values.insert(source, window.clone());
    execution
        .establish_record(
            &StructuralOperationResult {
                place: owner,
                structural_type: holder,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: vec![],
                projected_qualifications: vec![],
                qualification_establishments: vec![],
                claims: vec![],
            },
            &[RecordFieldInitializer {
                field,
                value: RecordFieldValue::Structural(StructuralArgument {
                    place: source,
                    path: vec![],
                    access: StructuralAccess::Owned,
                }),
            }],
        )
        .unwrap();
    let argument = StructuralArgument {
        place: owner,
        path: vec![StructuralPathSegment::Field("values".into())],
        access: StructuralAccess::SharedBorrow,
    };
    let resolved = execution
        .resolve_reference_call_arguments(std::slice::from_ref(&argument))
        .unwrap();
    let parameter = StructuralParameterDeclaration {
        place: borrowed,
        position: 0,
        is_self: false,
        structural_type: view,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    let bindings = execution
        .bind_element_view_arguments(
            std::slice::from_ref(&parameter),
            std::slice::from_ref(&argument),
            &resolved,
        )
        .unwrap();
    assert_eq!(bindings[&borrowed].len(), 2);
    assert_eq!(bindings[&borrowed].get(0), Some(&unsigned(11)));
    assert_eq!(bindings[&borrowed].get(1), Some(&unsigned(13)));
    assert!(
        std::ptr::eq(bindings[&borrowed].get(0).unwrap(), window.get(0).unwrap()),
        "field and call transport copy descriptors, not their backing"
    );
    let mut exclusive_parameter = parameter.clone();
    exclusive_parameter.access = StructuralAccess::MutableBorrow;
    let mut exclusive_argument = argument.clone();
    exclusive_argument.access = StructuralAccess::MutableBorrow;
    assert!(
        execution
            .bind_element_view_arguments(&[exclusive_parameter], &[exclusive_argument], &resolved,)
            .is_err(),
        "a stored shared descriptor cannot acquire exclusive access"
    );

    let home = StructuralRuntimePlace::from(&resolved[0]);
    execution
        .structural_values
        .insert(borrowed, resolved[0].clone());
    execution.element_view_values.extend(bindings);
    execution.retire_unrestricted_views();
    assert!(
        execution.structural_element_view_fields.contains_key(&home),
        "retiring a borrowed frame binding leaves the owner's field intact"
    );
    crate::reference::discard_structural_value(
        &mut execution.structural_values,
        &mut execution.reference_referents,
        &mut execution.structural_element_view_fields,
        owner,
    )
    .unwrap();
    assert!(!execution.structural_element_view_fields.contains_key(&home));
    assert_eq!(
        window.get(0),
        Some(&unsigned(11)),
        "carrier disposal never disposes backing"
    );
}

#[test]
fn qualified_shared_byte_field_rejects_invalid_contents_before_constructor_commit() {
    use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
    use crate::values::TerminalStructuralValue;
    let mut module = unit_module();
    let carrier = StructuralTypeId::new(1).unwrap();
    let record = StructuralTypeId::new(2).unwrap();
    let domain = StructuralDomainId::new(1).unwrap();
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: carrier,
            identity: "view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        },
        StructuralTypeDeclaration {
            id: record,
            identity: "text".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "count".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(unsigned(0).scalar_type()),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "text".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::PredicateByteSequence {
                            carrier: ByteSequenceCarrier::BorrowedView {
                                access: Some(StructuralAccess::SharedBorrow),
                            },
                            predicates,
                        },
                    },
                ],
            },
        },
    ];
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain,
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: "Utf8".into(),
        carrier,
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: vec![],
    });
    let (mut execution, _) = run(&module, &[]);
    let source = PlaceId::new(20).unwrap();
    let destination = PlaceId::new(21).unwrap();
    execution
        .values
        .insert(ValueId::new(99).unwrap(), unsigned(7));
    execution.structural_values.insert(
        source,
        TerminalStructuralValue {
            opaque_identity: 9000,
            structural_type: carrier,
            qualifications: vec![],
            path: vec![],
        },
    );
    execution.byte_sequence_values.insert(
        source,
        ByteSequenceBinding::Immutable(ByteSequenceView::new(vec![0xff])),
    );
    execution
        .live_affine_frontier
        .insert(StructuralAffineDiscard {
            place: source,
            path: vec![],
            structural_type: carrier,
        });
    let result = StructuralOperationResult {
        place: destination,
        structural_type: record,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        qualification_establishments: vec![],
        claims: vec![],
    };
    let fields = [
        RecordFieldInitializer {
            field: StructuralFieldId::new(1).unwrap(),
            value: RecordFieldValue::Scalar {
                value: ValueId::new(99).unwrap(),
                range_obligation: None,
            },
        },
        RecordFieldInitializer {
            field: StructuralFieldId::new(2).unwrap(),
            value: RecordFieldValue::Structural(StructuralArgument {
                place: source,
                path: vec![],
                access: StructuralAccess::Owned,
            }),
        },
    ];
    let values = execution.structural_values.clone();
    let frontier = execution.live_affine_frontier.clone();
    let scalars = execution.structural_scalar_fields.clone();
    execution
        .establish_record(&result, &fields)
        .expect_err("invalid UTF8 cannot inhabit the qualified shared field");
    assert_eq!(execution.structural_values, values);
    assert_eq!(execution.live_affine_frontier, frontier);
    assert_eq!(execution.structural_scalar_fields, scalars);
    assert!(execution.structural_byte_sequence_fields.is_empty());
    assert_eq!(
        execution.byte_sequence_values[&source]
            .immutable()
            .unwrap()
            .bytes(),
        &[0xff]
    );
    execution
        .structural_values
        .get_mut(&source)
        .unwrap()
        .qualifications = vec![domain];
    execution.byte_sequence_values.insert(
        source,
        ByteSequenceBinding::Immutable(ByteSequenceView::new(b"ok".to_vec())),
    );
    execution
        .establish_record(&result, &fields)
        .expect("valid qualified descriptor transfers only after every field validates");
    assert!(!execution.structural_values.contains_key(&source));
    assert!(execution.structural_values.contains_key(&destination));
    assert_eq!(
        execution
            .structural_byte_sequence_fields
            .values()
            .next()
            .unwrap()
            .bytes(),
        b"ok"
    );
}

#[test]
fn record_erased_declarations_have_no_runtime_initializers_or_storage() {
    let mut module = record_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!();
    };
    let scalar_type = unsigned(0).scalar_type();
    *fields = (1..=5)
        .map(|ordinal| StructuralFieldDeclaration {
            id: StructuralFieldId::new(ordinal).unwrap(),
            identity: format!("member{ordinal}"),
            relevance: if ordinal % 2 == 0 {
                BindingRelevance::Relevant
            } else {
                BindingRelevance::Erased
            },
            field_type: if ordinal % 2 == 0 {
                StructuralFieldType::Scalar(scalar_type)
            } else {
                StructuralFieldType::Erased {
                    type_identity: "EvidenceWithNominalCleanup".into(),
                }
            },
        })
        .collect();
    let operation = &mut module.machines[0].blocks[0].operations[0];
    let OperationKind::EstablishRecord { fields } = &mut operation.kind else {
        unreachable!();
    };
    let mut initializer = fields[0].clone();
    initializer.field = StructuralFieldId::new(2).unwrap();
    *fields = vec![initializer.clone()];
    initializer.field = StructuralFieldId::new(4).unwrap();
    fields.push(initializer);
    let (mut execution, result) = run(&module, &[unsigned(37)]);
    assert_eq!(result, TerminalExecutionResult::Unit);
    assert_eq!(execution.structural_scalar_fields.len(), 2);
    assert!(
        execution
            .structural_scalar_fields
            .values()
            .all(|value| *value == unsigned(37))
    );
    assert!(execution.reference_referents.is_empty());
    assert!(execution.structural_element_view_fields.is_empty());
    assert!(execution.live_affine_frontier.is_empty());

    let operation = &module.machines[0].blocks[0].operations[0];
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        unreachable!();
    };
    let mut result = operation.result.structural().unwrap().clone();
    result.place = PlaceId::new(20).unwrap();
    for mutation in ["erased", "missing", "duplicate", "order"] {
        let mut changed = fields.clone();
        match mutation {
            "erased" => changed[0].field = StructuralFieldId::new(1).unwrap(),
            "missing" => {
                changed.pop();
            }
            "duplicate" => changed.push(changed[0].clone()),
            "order" => changed.reverse(),
            _ => unreachable!(),
        }
        let before = execution.structural_scalar_fields.clone();
        assert!(
            execution.establish_record(&result, &changed).is_err(),
            "{mutation}"
        );
        assert_eq!(execution.structural_scalar_fields, before);
        assert!(!execution.structural_values.contains_key(&result.place));
    }
}

fn projected_constructor_fixture(
    case: bool,
) -> (
    TerminalExecution,
    StructuralOperationResult,
    Vec<RecordFieldInitializer>,
) {
    let mut module = record_module();
    let member = |identity: u64, name: &str| StructuralFieldDeclaration {
        id: StructuralFieldId::new(identity).unwrap(),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(StructuralTypeId::new(1).unwrap()),
    };
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(2).unwrap(),
            identity: "Pair".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![member(1, "left"), member(2, "right")],
            },
        },
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(3).unwrap(),
            identity: "Selected".into(),
            shape: if case {
                StructuralTypeShape::Sum {
                    cases: vec![StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: "Value".into(),
                        fields: vec![member(1, "selected")],
                    }],
                }
            } else {
                StructuralTypeShape::Record {
                    fields: vec![member(1, "selected")],
                }
            },
        },
    ]);
    let (mut execution, _) = run(&module, &[unsigned(7)]);
    let source = PlaceId::new(20).unwrap();
    execution.structural_values.insert(
        source,
        crate::values::TerminalStructuralValue {
            opaque_identity: 9000,
            structural_type: StructuralTypeId::new(2).unwrap(),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    );
    execution
        .live_affine_frontier
        .insert(StructuralAffineDiscard {
            place: source,
            path: Vec::new(),
            structural_type: StructuralTypeId::new(2).unwrap(),
        });
    for (name, scalar) in [("left", 11), ("right", 29)] {
        execution.structural_scalar_fields.insert(
            crate::values::StructuralScalarRuntimeField::member(
                crate::values::StructuralRuntimePlace {
                    opaque_identity: 9000,
                    path: vec![StructuralPathSegment::Field(name.into())],
                },
                StructuralFieldId::new(1).unwrap(),
            ),
            unsigned(scalar),
        );
    }
    let result = StructuralOperationResult {
        place: PlaceId::new(21).unwrap(),
        structural_type: StructuralTypeId::new(3).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let fields = vec![RecordFieldInitializer {
        field: StructuralFieldId::new(1).unwrap(),
        value: RecordFieldValue::Structural(StructuralArgument {
            place: source,
            path: vec![StructuralPathSegment::Field("left".into())],
            access: StructuralAccess::Owned,
        }),
    }];
    (execution, result, fields)
}

fn execute_projected_constructor(
    execution: &mut TerminalExecution,
    result: &StructuralOperationResult,
    fields: &[RecordFieldInitializer],
    case: bool,
) -> Result<(), crate::errors::TerminalInterpretError> {
    if case {
        execution
            .execute_establish_structural_case(&Operation {
                id: OperationId::new(100).unwrap(),
                static_reach_binding: None,
                suspension_crossing: None,
                result: OperationResult::Structural(result.clone()),
                kind: OperationKind::EstablishStructuralCase {
                    result_case: StructuralCaseId::new(1).unwrap(),
                    fields: fields.to_vec(),
                },
            })
            .map(|_| ())
    } else {
        execution.establish_record(result, fields)
    }
}

#[test]
fn projected_constructor_operands_preserve_live_sibling_storage() {
    for case in [false, true] {
        let (mut execution, result, fields) = projected_constructor_fixture(case);
        execute_projected_constructor(&mut execution, &result, &fields, case).unwrap();
        let selected = &execution.structural_values[&result.place];
        let cell = |identity, name: &str| {
            crate::values::StructuralScalarRuntimeField::member(
                crate::values::StructuralRuntimePlace {
                    opaque_identity: identity,
                    path: vec![StructuralPathSegment::Field(name.into())],
                },
                StructuralFieldId::new(1).unwrap(),
            )
        };
        assert_eq!(
            execution
                .structural_scalar_fields
                .get(&cell(selected.opaque_identity, "selected")),
            Some(&unsigned(11))
        );
        assert_eq!(
            execution.structural_scalar_fields.get(&cell(9000, "right")),
            Some(&unsigned(29))
        );
        assert!(
            !execution
                .structural_scalar_fields
                .contains_key(&cell(9000, "left"))
        );
        assert!(
            execution
                .live_affine_frontier
                .contains(&StructuralAffineDiscard {
                    place: PlaceId::new(20).unwrap(),
                    path: vec![StructuralPathSegment::Field("right".into())],
                    structural_type: StructuralTypeId::new(1).unwrap(),
                })
        );
        let mut repeated = result.clone();
        repeated.place = PlaceId::new(22).unwrap();
        assert!(execute_projected_constructor(&mut execution, &repeated, &fields, case).is_err());
        assert_eq!(
            execution.structural_scalar_fields.get(&cell(9000, "right")),
            Some(&unsigned(29))
        );
    }
}

#[test]
fn projected_constructor_operands_refuse_unowned_or_unknown_paths_atomically() {
    for case in [false, true] {
        for mutation in 0..2 {
            let (mut execution, result, mut fields) = projected_constructor_fixture(case);
            if mutation == 0 {
                execution.live_affine_frontier.clear();
            } else {
                let RecordFieldValue::Structural(argument) = &mut fields[0].value else {
                    unreachable!()
                };
                argument.path = vec![StructuralPathSegment::Field("absent".into())];
            }
            let before = execution.structural_scalar_fields.clone();
            let frontier = execution.live_affine_frontier.clone();
            assert!(execute_projected_constructor(&mut execution, &result, &fields, case).is_err());
            assert_eq!(execution.structural_scalar_fields, before);
            assert_eq!(execution.live_affine_frontier, frontier);
            assert!(!execution.structural_values.contains_key(&result.place));
        }
    }
}
