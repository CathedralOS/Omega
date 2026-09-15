//! Tests for physical evidence derivation.
#[test]
fn reference_projection_identity_is_distinct_from_owned_paths() {
    use terminal_psi::StructuralPathSegment;
    let identity = |path: &[StructuralPathSegment]| {
        let mut digest = sha2::Sha256::default();
        crate::physical::derivation::hashing::hash_structural_path(&mut digest, path);
        sha2::Digest::finalize(digest)
    };
    let reference = identity(&[StructuralPathSegment::Referent]);
    assert_ne!(reference, identity(&[]));
    assert_ne!(
        reference,
        identity(&[StructuralPathSegment::Field("Referent".into())])
    );
    assert_ne!(reference, identity(&[StructuralPathSegment::FixedIndex(3)]));
}
use crate::NativeSelectedProviderPlanDigest;
use crate::physical::derivation::evidence::PhysicalChildCoordinate;
use crate::physical::derivation::evidence::boundary_occurrence_identity;
use crate::physical::derivation::evidence::operator_occurrence_identity;
use crate::physical::derivation::evidence::validate_exact_physical_child_coordinates;
use crate::physical::derivation::settlement_identity::admitted_provider_settlement_identity;
use crate::physical::derivation::settlement_identity::builtin_structural_boundary_trait_settlement_identity;
use crate::physical::model::NativeOptimizationProjection;
use crate::physical::model::NativePhysicalOccurrence;
use crate::physical::model::native_optimization_projection;
use crate::physical::model::optimized_boundary_occurrence;
use crate::physical::model::optimized_operator_occurrence;
use optimization_core::NativeOptimizationProjectionIdentity;
use optimization_core::OptimizedBoundaryOccurrenceIdentity;
use optimization_core::OptimizedOperatorOccurrenceIdentity;
use semantic_vocabulary::IntegerSign;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::ScalarType;
use target::NativeTarget;
use terminal_psi::{SemanticFingerprint, VocabularyMarker};

fn physical_projection() -> NativeOptimizationProjection {
    let terminal = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([19; 32]),
    };
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine");
    let operator = optimized_operator_occurrence(
        terminal,
        machine,
        semantic_vocabulary::OperationId::new(2).expect("operator"),
        0,
        OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"operator survivor"),
    );
    let boundary = optimized_boundary_occurrence(
        terminal,
        machine,
        semantic_vocabulary::OperationId::new(3).expect("boundary operation"),
        semantic_vocabulary::BoundaryMachineId::new(4).expect("boundary"),
        1,
        OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(b"boundary survivor"),
    );
    native_optimization_projection(
        terminal,
        vec![operator],
        vec![boundary],
        NativeOptimizationProjectionIdentity::from_canonical_bytes(b"physical projection"),
    )
}

fn exact_coordinates(projection: &NativeOptimizationProjection) -> [PhysicalChildCoordinate; 2] {
    [
        PhysicalChildCoordinate {
            projection: projection.identity(),
            occurrence: NativePhysicalOccurrence::Operator(
                projection.operator_occurrences()[0].identity(),
            ),
            parent_role: 1,
        },
        PhysicalChildCoordinate {
            projection: projection.identity(),
            occurrence: NativePhysicalOccurrence::Boundary(
                projection.boundary_occurrences()[0].identity(),
            ),
            parent_role: 2,
        },
    ]
}

#[test]
fn equal_boundary_requirements_at_distinct_operations_have_distinct_occurrences() {
    let terminal = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
    };
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine");
    let boundary = semantic_vocabulary::BoundaryMachineId::new(2).expect("boundary");
    let first = boundary_occurrence_identity(
        terminal,
        machine,
        semantic_vocabulary::OperationId::new(3).expect("first operation"),
        boundary,
        0,
    );
    let second = boundary_occurrence_identity(
        terminal,
        machine,
        semantic_vocabulary::OperationId::new(4).expect("second operation"),
        boundary,
        1,
    );

    assert_ne!(first, second);
}

#[test]
fn operator_occurrence_identity_binds_exact_terminal_operation() {
    let terminal = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
    };
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine");
    let first = operator_occurrence_identity(
        terminal,
        machine,
        semantic_vocabulary::OperationId::new(3).expect("first operation"),
        0,
    );
    let second = operator_occurrence_identity(
        terminal,
        machine,
        semantic_vocabulary::OperationId::new(4).expect("second operation"),
        1,
    );

    assert_ne!(first, second);
}

#[test]
fn physical_children_require_an_exact_survivor_bijection() {
    let projection = physical_projection();
    let [operator, boundary] = exact_coordinates(&projection);
    assert!(validate_exact_physical_child_coordinates(&projection, [operator, boundary]).is_ok());

    assert_eq!(
        validate_exact_physical_child_coordinates(&projection, [operator]),
        Err("native physical evidence does not cover the exact surviving occurrence set")
    );
    assert_eq!(
        validate_exact_physical_child_coordinates(&projection, [operator, operator, boundary]),
        Err("native physical evidence contains duplicate optimized occurrences")
    );

    let padded = PhysicalChildCoordinate {
        projection: projection.identity(),
        occurrence: NativePhysicalOccurrence::Operator(
            OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"stale occurrence"),
        ),
        parent_role: 1,
    };
    assert_eq!(
        validate_exact_physical_child_coordinates(&projection, [operator, boundary, padded]),
        Err("native physical child swapped or substituted its semantic parent role")
    );

    let detached = PhysicalChildCoordinate {
        projection: NativeOptimizationProjectionIdentity::from_canonical_bytes(
            b"detached projection",
        ),
        ..operator
    };
    assert_eq!(
        validate_exact_physical_child_coordinates(&projection, [detached, boundary]),
        Err("native physical child is detached from its optimized projection")
    );

    let role_swapped = PhysicalChildCoordinate {
        parent_role: 2,
        ..operator
    };
    assert_eq!(
        validate_exact_physical_child_coordinates(&projection, [role_swapped, boundary]),
        Err("native physical child swapped or substituted its semantic parent role")
    );
}

#[test]
fn structural_boundary_settlement_identity_binds_the_complete_result_declaration() {
    use semantic_vocabulary::{
        BoundedIntegerType, IntegerValue, OperationId, PlaceId, StructuralCaseId,
        StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let original = machine_code::BoundaryStructuralResultRecord {
        defining_operation: OperationId::new(3).unwrap(),
        result: StructuralOperationResult {
            place: PlaceId::new(1).unwrap(),
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        declaration: StructuralTypeDeclaration {
            id: structural_type,
            identity: "InputResult".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: "End".into(),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).unwrap(),
                        identity: "Value".into(),
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "value".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::BoundedInteger(
                                BoundedIntegerType::new(
                                    integer,
                                    IntegerValue::Signed(0),
                                    IntegerValue::Signed(255),
                                )
                                .unwrap(),
                            ),
                        }],
                    },
                ],
            },
        },
        layout: calling_conventions::evaluate_conventional_sum_layout(
            &[],
            &[
                Vec::new(),
                vec![calling_conventions::ValueShape::integer(4, 4)],
            ],
        )
        .unwrap(),
        home_byte_offset: 16,
    };
    let projection = physical_projection();
    let identity = |result: &machine_code::BoundaryStructuralResultRecord| {
        builtin_structural_boundary_trait_settlement_identity(
            &projection.boundary_occurrences()[0],
            "Input::read",
            NativeSelectedProviderPlanDigest::from_digest([7; 32]),
            NativeTarget::linux_x64(),
            result,
        )
        .unwrap()
    };
    let expected = identity(&original);
    for mutation in 0..7 {
        let mut changed = original.clone();
        let StructuralTypeShape::Sum { cases } = &mut changed.declaration.shape else {
            panic!("sum")
        };
        match mutation {
            0 => changed.declaration.identity.push_str("Other"),
            1 => cases[1].identity.push_str("Other"),
            2 => cases[1].id = StructuralCaseId::new(3).unwrap(),
            3 => cases[1].fields[0].identity.push_str("Other"),
            4 => cases[1].fields[0].id = StructuralFieldId::new(2).unwrap(),
            5 => {
                cases[1].fields[0].field_type = StructuralFieldType::BoundedInteger(
                    BoundedIntegerType::new(
                        integer,
                        IntegerValue::Signed(-1),
                        IntegerValue::Signed(256),
                    )
                    .unwrap(),
                )
            }
            _ => {
                cases[1].fields[0].field_type =
                    StructuralFieldType::Scalar(ScalarType::Integer(integer))
            }
        }
        assert_eq!(
            changed.layout, original.layout,
            "unchanged layout cannot mask semantic drift"
        );
        assert_ne!(
            identity(&changed),
            expected,
            "declaration mutation {mutation}"
        );
    }
}

#[test]
fn admitted_provider_settlement_identity_binds_the_complete_retained_row() {
    use semantic_vocabulary::{
        BoundaryMachineId, ClaimId, EdgeId, IntegerValue, OperationId, PlaceId, ServiceId,
        StructuralCaseId, StructuralTypeId, ValueId,
    };
    use target_operations::{
        BoundaryScalarArgument, ClaimCompletionOnlyRealization, MetadataOnlyPortRealization,
    };
    use terminal_psi::{
        CompletionReceipt, EntryClaim, StructuralAccess, StructuralArgument,
        StructuralCaseDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    };

    let projection = physical_projection();
    let occurrence = &projection.boundary_occurrences()[0];
    let record = machine_code::ProviderExecutionRecord::new(7, 11, 13, 17, 19).unwrap();
    let execution = target_operations::ProviderExecutionBinding::from_execution_record(
        target_operations::ProviderPlanReportIdentity::new(7).unwrap(),
        11,
        13,
        17,
        19,
    )
    .unwrap();
    let claim = ClaimId::new(23).unwrap();
    let argument = StructuralArgument {
        place: PlaceId::new(29).unwrap(),
        path: vec![terminal_psi::StructuralPathSegment::Field("leaf".into())],
        access: StructuralAccess::Owned,
    };
    let source = target_operations::CompletionClaimSource {
        claim,
        entry: Some(EntryClaim {
            claim,
            input: argument.place,
            path: Vec::new(),
        }),
        content: None,
    };
    let receipt = CompletionReceipt {
        claim,
        argument_index: 0,
    };
    let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let byte_sequence = machine_code::BoundaryByteSequenceArgumentRecord {
        argument: argument.clone(),
        literal_operation: OperationId::new(47).unwrap(),
        structural_type: StructuralTypeDeclaration {
            id: StructuralTypeId::new(73).unwrap(),
            identity: "Bytes".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![StructuralCaseDeclaration {
                    id: StructuralCaseId::new(1).unwrap(),
                    identity: "Bytes".into(),
                    fields: Vec::new(),
                }],
            },
        },
        bytes: b"payload".to_vec(),
        code_offset: 9,
        code_byte_count: 3,
        data_offset: 4,
        data_byte_count: 7,
    };
    let scalar_result =
        machine_code::BoundaryResultRecord::Scalar(machine_code::BoundaryScalarResultRecord {
            value: ValueId::new(67).unwrap(),
            scalar_type: u8_type,
            placement: calling_conventions::ValuePlacement {
                shape: calling_conventions::ValueShape::integer(1, 1),
                locations: vec![calling_conventions::ValueLocation::Register {
                    register: calling_conventions::MachineRegister::X86Rax,
                    value_byte_offset: 0,
                    byte_size: 1,
                }],
            },
            return_edge: EdgeId::new(71).unwrap(),
        });
    let completion = machine_code::BoundarySettlementRecord {
        psi_operation: OperationId::new(41).unwrap(),
        boundary: BoundaryMachineId::new(43).unwrap(),
        execution: machine_code::BoundaryExecutionRecord::AdmittedProvider(record),
        realization: target_operations::BoundaryRealization::ClaimCompletionOnly(
            ClaimCompletionOnlyRealization,
        ),
        scalar_arguments: Vec::new(),
        runtime_scalar_arguments: Vec::new(),
        arguments: vec![argument.clone()],
        byte_sequence_arguments: vec![byte_sequence],
        completion_claim_sources: vec![source.clone()],
        completion_receipts: vec![receipt],
        completion_provider_custody: vec![machine_code::CompletionProviderCustodyBinding {
            source: source.clone(),
            receipt,
            provider_execution: record,
        }],
        native_result: machine_code::BoundaryResultRecord::Unit,
        operation_ordinal: 5,
        code_offset: 8,
        byte_count: 0,
    };
    let identity = |settlement: &machine_code::BoundarySettlementRecord,
                    port_effect: Option<&machine_code::PortEffectRecord>,
                    execution: target_operations::ProviderExecutionBinding| {
        admitted_provider_settlement_identity(
            occurrence,
            "Extent::complete",
            NativeSelectedProviderPlanDigest::from_digest([7; 32]),
            NativeTarget::linux_x64(),
            execution,
            settlement,
            port_effect,
        )
        .unwrap()
    };
    let expected = identity(&completion, None, execution);

    for mutation in 0..22 {
        let mut changed = completion.clone();
        match mutation {
            0 => changed.psi_operation = OperationId::new(53).unwrap(),
            1 => changed.boundary = BoundaryMachineId::new(59).unwrap(),
            2 => {
                changed.execution = machine_code::BoundaryExecutionRecord::AdmittedProvider(
                    machine_code::ProviderExecutionRecord::new(7, 11, 13, 17, 23).unwrap(),
                )
            }
            3 => {
                changed.execution = machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                )
            }
            4 => {
                changed.realization = target_operations::BoundaryRealization::MetadataOnlyPort(
                    MetadataOnlyPortRealization {
                        effect_operation: OperationId::new(40).unwrap(),
                        service: ServiceId::new(3).unwrap(),
                        port: 0x3f8,
                        value: 0x51,
                    },
                )
            }
            5 => changed.scalar_arguments.push(BoundaryScalarArgument {
                source_value: ValueId::new(31).unwrap(),
                scalar_type: u8_type,
                immediate: IntegerValue::Unsigned(7),
                destination: calling_conventions::MachineRegister::X86Rdi,
            }),
            6 => {
                changed
                    .runtime_scalar_arguments
                    .push(machine_code::ForeignCallScalarArgumentRecord {
                    parameter_index: 0,
                    source:
                        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                            source_value: ValueId::new(37).unwrap(),
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                            ),
                            instruction: selected_instructions::SelectedInstructionId(41),
                        },
                    placement: calling_conventions::ValuePlacement {
                        shape: calling_conventions::ValueShape::integer(4, 4),
                        locations: vec![calling_conventions::ValueLocation::Register {
                            register: calling_conventions::MachineRegister::X86Rax,
                            value_byte_offset: 0,
                            byte_size: 4,
                        }],
                    },
                    code_offset: 2,
                    byte_count: 4,
                })
            }
            7 => changed.arguments[0].access = StructuralAccess::SharedBorrow,
            8 => changed.arguments.push(argument.clone()),
            9 => changed.byte_sequence_arguments[0].bytes = b"other".to_vec(),
            10 => changed.byte_sequence_arguments[0].data_byte_count += 1,
            11 => changed.completion_claim_sources[0].entry = None,
            12 => changed.completion_claim_sources.push(source.clone()),
            13 => changed.completion_receipts[0].argument_index = 1,
            14 => changed.completion_receipts.push(CompletionReceipt {
                claim: ClaimId::new(61).unwrap(),
                argument_index: 1,
            }),
            15 => {
                changed.completion_provider_custody[0]
                    .provider_execution
                    .boundary_contract_report_fingerprint = 23
            }
            16 => {
                changed.completion_provider_custody[0]
                    .receipt
                    .argument_index = 7
            }
            17 => changed.native_result = scalar_result.clone(),
            18 => changed.operation_ordinal += 1,
            19 => changed.code_offset += 1,
            20 => changed.byte_count += 1,
            _ => changed.completion_provider_custody.clear(),
        }
        assert_ne!(
            identity(&changed, None, execution),
            expected,
            "settlement mutation {mutation}"
        );
    }

    for mutation in 0..5 {
        let changed = match mutation {
            0 => admitted_provider_settlement_identity(
                &optimized_boundary_occurrence(
                    occurrence.terminal(),
                    occurrence.machine(),
                    OperationId::new(9).unwrap(),
                    occurrence.boundary(),
                    occurrence.operation_ordinal(),
                    OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(b"other occurrence"),
                ),
                "Extent::complete",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                execution,
                &completion,
                None,
            )
            .unwrap(),
            1 => admitted_provider_settlement_identity(
                occurrence,
                "Extent::other",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                execution,
                &completion,
                None,
            )
            .unwrap(),
            2 => admitted_provider_settlement_identity(
                occurrence,
                "Extent::complete",
                NativeSelectedProviderPlanDigest::from_digest([8; 32]),
                NativeTarget::linux_x64(),
                execution,
                &completion,
                None,
            )
            .unwrap(),
            3 => admitted_provider_settlement_identity(
                occurrence,
                "Extent::complete",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::windows_x64(),
                execution,
                &completion,
                None,
            )
            .unwrap(),
            _ => identity(
                &completion,
                None,
                target_operations::ProviderExecutionBinding::from_execution_record(
                    target_operations::ProviderPlanReportIdentity::new(7).unwrap(),
                    11,
                    13,
                    17,
                    29,
                )
                .unwrap(),
            ),
        };
        assert_ne!(changed, expected, "binding input mutation {mutation}");
    }

    let port_effect = machine_code::PortEffectRecord {
        psi_operation: OperationId::new(40).unwrap(),
        service: ServiceId::new(3).unwrap(),
        port: 0x3f8,
        value: 0x51,
        operation_ordinal: 4,
        code_offset: 6,
        byte_count: 2,
    };
    let port = machine_code::BoundarySettlementRecord {
        realization: target_operations::BoundaryRealization::MetadataOnlyPort(
            MetadataOnlyPortRealization {
                effect_operation: OperationId::new(40).unwrap(),
                service: ServiceId::new(3).unwrap(),
                port: 0x3f8,
                value: 0x51,
            },
        ),
        scalar_arguments: Vec::new(),
        runtime_scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        byte_sequence_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
        completion_provider_custody: Vec::new(),
        ..completion.clone()
    };
    let port_expected = identity(&port, Some(&port_effect), execution);
    assert_ne!(port_expected, identity(&port, None, execution));
    for mutation in 0..7 {
        let mut changed = port_effect.clone();
        match mutation {
            0 => changed.psi_operation = OperationId::new(43).unwrap(),
            1 => changed.service = ServiceId::new(5).unwrap(),
            2 => changed.port += 1,
            3 => changed.value += 1,
            4 => changed.operation_ordinal += 1,
            5 => changed.code_offset += 1,
            _ => changed.byte_count += 1,
        }
        assert_ne!(
            identity(&port, Some(&changed), execution),
            port_expected,
            "port-effect mutation {mutation}"
        );
    }
    for mutation in 0..4 {
        let mut changed = port.clone();
        let target_operations::BoundaryRealization::MetadataOnlyPort(realization) =
            &mut changed.realization
        else {
            panic!("metadata-only port settlement")
        };
        match mutation {
            0 => realization.effect_operation = OperationId::new(43).unwrap(),
            1 => realization.service = ServiceId::new(5).unwrap(),
            2 => realization.port += 1,
            _ => realization.value += 1,
        }
        assert_ne!(
            identity(&changed, Some(&port_effect), execution),
            port_expected,
            "metadata-port realization mutation {mutation}"
        );
    }
}
