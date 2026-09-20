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
use crate::physical::derivation::settlement_identity::hosted_builtin_settlement_identity;
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
        hosted_builtin_settlement_identity(
            &projection.boundary_occurrences()[0],
            "Input::read",
            NativeSelectedProviderPlanDigest::from_digest([7; 32]),
            NativeTarget::linux_x64(),
            [3, 3],
            None,
            &crate::CompilerBuiltinResult::Structural(result.clone()),
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

/// The bounded structural lane admits only source-rooted borrowed
/// flat-record projections and rejects every missing, duplicate,
/// substituted, or role-swapped expected/observed row.
#[test]
fn normalized_foreign_structural_signature_requires_the_exact_admitted_lane() {
    use calling_conventions::{MachineRegister, ValueLocation, ValuePlacement, ValueShape};
    use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralTypeId};
    use terminal_psi::{
        BoundaryParameterKind, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathQualification, StructuralPathSegment,
    };

    use crate::physical::derivation::children::normalized_foreign_structural_parameter_shapes;

    let pointer = ValueShape::integer(8, 8);
    let parameter =
        |place: u64, position: u32, access: StructuralAccess| StructuralParameterDeclaration {
            place: PlaceId::new(place).unwrap(),
            position,
            is_self: false,
            structural_type: StructuralTypeId::new(40 + place).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
    let argument = |place: u64, field: &str, access: StructuralAccess| StructuralArgument {
        place: PlaceId::new(place).unwrap(),
        path: vec![StructuralPathSegment::Field(field.to_owned())],
        access,
    };
    let caller_parameters = vec![
        parameter(101, 0, StructuralAccess::SharedBorrow),
        parameter(102, 1, StructuralAccess::MutableBorrow),
    ];
    let parameters = vec![
        parameter(201, 0, StructuralAccess::SharedBorrow),
        parameter(202, 1, StructuralAccess::MutableBorrow),
    ];
    let arguments = vec![
        argument(101, "header", StructuralAccess::SharedBorrow),
        argument(102, "payload", StructuralAccess::MutableBorrow),
    ];
    let placement = |register: MachineRegister| ValuePlacement {
        shape: pointer,
        locations: vec![ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        }],
    };
    let placements = vec![
        placement(MachineRegister::X86Rdi),
        placement(MachineRegister::X86Rsi),
    ];

    let signature = |arguments: &[StructuralArgument],
                     parameters: &[StructuralParameterDeclaration],
                     placements: &[ValuePlacement]| {
        normalized_foreign_structural_parameter_shapes(
            arguments,
            parameters,
            &caller_parameters,
            &vec![BoundaryParameterKind::Structural; parameters.len()],
            false,
            placements,
            pointer,
        )
    };

    // The exact admitted lane: two source-rooted borrowed flat-record
    // projections, each placed as one pointer-width word in the observed plan.
    assert_eq!(
        signature(&arguments, &parameters, &placements),
        Ok(Some(vec![pointer, pointer]))
    );
    // An empty structural signature contributes no parameters.
    assert_eq!(signature(&[], &[], &[]), Ok(Some(Vec::new())));

    // Missing rows reject: a dropped argument cannot cover its declared
    // formal, and a dropped observed placement cannot realize it.
    assert_eq!(
        signature(&arguments[..1], &parameters, &placements),
        Err("normalized foreign D41 child changed its structural call occurrence")
    );
    assert_eq!(
        signature(&[], &parameters, &placements),
        Err("normalized foreign D41 child changed its structural call occurrence")
    );
    assert_eq!(
        signature(&arguments, &parameters, &placements[..1]),
        Err("normalized foreign D41 child changed its structural call custody")
    );

    // Duplicate rows reject: a duplicated formal position and an extra
    // observed parameter row both break the positional bijection.
    let mut duplicated = parameters.clone();
    duplicated[1].position = 0;
    assert_eq!(
        signature(&arguments, &duplicated, &placements),
        Err("normalized foreign D41 child changed its structural call occurrence")
    );
    let mut padded = placements.clone();
    padded.push(placement(MachineRegister::X86Rdx));
    assert_eq!(
        signature(&arguments, &parameters, &padded),
        Err("normalized foreign D41 child changed its structural call custody")
    );

    // Substituted rows reject: a changed argument access no longer matches
    // its formal, and a non-pointer-word observed placement no longer
    // realizes it.
    let mut substituted = arguments.clone();
    substituted[0].access = StructuralAccess::MutableBorrow;
    assert_eq!(
        signature(&substituted, &parameters, &placements),
        Err("normalized foreign D41 child changed its structural call occurrence")
    );
    let mut narrow = placements.clone();
    narrow[0] = ValuePlacement {
        shape: ValueShape::integer(4, 4),
        locations: vec![ValueLocation::Register {
            register: MachineRegister::X86Rdi,
            value_byte_offset: 0,
            byte_size: 4,
        }],
    };
    assert_eq!(
        signature(&arguments, &parameters, &narrow),
        Err("normalized foreign D41 child changed its structural call custody")
    );
    let mut indirect = placements.clone();
    indirect[0].locations = vec![ValueLocation::Indirect {
        pointer: calling_conventions::IndirectPointerLocation::Register(MachineRegister::X86Rdi),
        copy_stack_byte_offset: None,
        byte_size: 8,
        alignment: 8,
    }];
    assert_eq!(
        signature(&arguments, &parameters, &indirect),
        Err("normalized foreign D41 child changed its structural call custody")
    );

    // Role-swapped rows reject: formal positions or argument order that no
    // longer line up with the declared signature cannot replay.
    let mut swapped_positions = parameters.clone();
    swapped_positions[0].position = 1;
    swapped_positions[1].position = 0;
    assert_eq!(
        signature(&arguments, &swapped_positions, &placements),
        Err("normalized foreign D41 child changed its structural call occurrence")
    );
    let mut swapped_arguments = arguments.clone();
    swapped_arguments.swap(0, 1);
    assert_eq!(
        signature(&swapped_arguments, &parameters, &placements),
        Err("normalized foreign D41 child changed its structural call occurrence")
    );

    // Structural lane-local positions rejoin authored native positions even
    // when scalar parameters precede and separate the borrowed pointers.
    let interleaved_order = [
        BoundaryParameterKind::Scalar,
        BoundaryParameterKind::Structural,
        BoundaryParameterKind::Scalar,
        BoundaryParameterKind::Structural,
    ];
    let mut interleaved_placements = vec![
        narrow[0].clone(),
        placements[0].clone(),
        narrow[0].clone(),
        placements[1].clone(),
    ];
    assert_eq!(
        normalized_foreign_structural_parameter_shapes(
            &arguments,
            &parameters,
            &caller_parameters,
            &interleaved_order,
            false,
            &interleaved_placements,
            pointer,
        ),
        Ok(Some(vec![pointer, pointer]))
    );
    interleaved_placements.swap(0, 1);
    assert!(
        normalized_foreign_structural_parameter_shapes(
            &arguments,
            &parameters,
            &caller_parameters,
            &interleaved_order,
            false,
            &interleaved_placements,
            pointer,
        )
        .is_err()
    );

    // Valid but uncovered signature shapes retain no complete evidence: a
    // callback beside the structural formals, an owned
    // formal, a qualified or non-unrestricted formal, a non-field or empty
    // projection path, and an argument not rooted at a caller structural
    // parameter.
    assert_eq!(
        normalized_foreign_structural_parameter_shapes(
            &arguments,
            &parameters,
            &caller_parameters,
            &[BoundaryParameterKind::Structural; 2],
            true,
            &placements,
            pointer,
        ),
        Ok(None)
    );
    let owned_parameters = vec![
        parameter(201, 0, StructuralAccess::Owned),
        parameter(202, 1, StructuralAccess::Owned),
    ];
    let owned_arguments = vec![
        argument(101, "header", StructuralAccess::Owned),
        argument(102, "payload", StructuralAccess::Owned),
    ];
    assert_eq!(
        signature(&owned_arguments, &owned_parameters, &placements),
        Ok(None)
    );
    let mut affine = parameters.clone();
    affine[0].multiplicity = StructuralMultiplicity::Affine;
    assert_eq!(signature(&arguments, &affine, &placements), Ok(None));
    let mut qualified = parameters.clone();
    qualified[0]
        .qualifications
        .push(StructuralDomainId::new(3).unwrap());
    assert_eq!(signature(&arguments, &qualified, &placements), Ok(None));
    let mut projected_qualified = parameters.clone();
    projected_qualified[0]
        .projected_qualifications
        .push(StructuralPathQualification {
            path: vec![StructuralPathSegment::Field("leaf".into())],
            domain: StructuralDomainId::new(5).unwrap(),
        });
    assert_eq!(
        signature(&arguments, &projected_qualified, &placements),
        Ok(None)
    );
    let mut indexed = arguments.clone();
    indexed[0].path.push(StructuralPathSegment::FixedIndex(0));
    assert_eq!(signature(&indexed, &parameters, &placements), Ok(None));
    let mut root_path = arguments.clone();
    root_path[0].path.clear();
    assert_eq!(signature(&root_path, &parameters, &placements), Ok(None));
    let mut foreign_root = arguments.clone();
    foreign_root[0].place = PlaceId::new(999).unwrap();
    assert_eq!(signature(&foreign_root, &parameters, &placements), Ok(None));
}

/// The admitted-provider parent identity of a normalized foreign call binds
/// the complete expected structural custody — every authored argument row and
/// every declared formal row — so a missing, duplicate, substituted, or
/// role-swapped row cannot replay as the same child.
#[test]
fn normalized_foreign_parent_identity_binds_the_structural_call_custody() {
    use semantic_vocabulary::{OperationId, PlaceId, StructuralDomainId, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathQualification, StructuralPathSegment,
    };

    use crate::physical::derivation::settlement_identity::admitted_provider_boundary_trait_settlement_identity;

    let projection = physical_projection();
    let occurrence = &projection.boundary_occurrences()[0];
    let execution = target_operations::ProviderExecutionBinding::from_execution_record(
        target_operations::ProviderPlanReportIdentity::new(7).unwrap(),
        11,
        13,
        17,
        19,
    )
    .unwrap();
    let locator = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ReadFile".to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .expect("valid normalized Windows import");
    let parameter =
        |place: u64, position: u32, access: StructuralAccess| StructuralParameterDeclaration {
            place: PlaceId::new(place).unwrap(),
            position,
            is_self: false,
            structural_type: StructuralTypeId::new(40 + place).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
    let structural_parameters = vec![
        parameter(201, 0, StructuralAccess::SharedBorrow),
        parameter(202, 1, StructuralAccess::MutableBorrow),
    ];
    let structural_arguments = vec![
        StructuralArgument {
            place: PlaceId::new(101).unwrap(),
            path: vec![StructuralPathSegment::Field("header".into())],
            access: StructuralAccess::SharedBorrow,
        },
        StructuralArgument {
            place: PlaceId::new(102).unwrap(),
            path: vec![StructuralPathSegment::Field("payload".into())],
            access: StructuralAccess::MutableBorrow,
        },
    ];
    let identity = |arguments: &[StructuralArgument],
                    parameters: &[StructuralParameterDeclaration]| {
        admitted_provider_boundary_trait_settlement_identity(
            occurrence,
            "Input::read",
            NativeSelectedProviderPlanDigest::from_digest([7; 32]),
            NativeTarget::linux_x64(),
            execution,
            [9; 32],
            &locator,
            [11; 32],
            arguments,
            parameters,
        )
    };
    let expected = identity(&structural_arguments, &structural_parameters);
    // Empty structural custody is a distinct parent identity.
    assert_ne!(expected, identity(&[], &[]));

    for mutation in 0..6 {
        let mut changed = structural_arguments.clone();
        match mutation {
            0 => changed[0].place = PlaceId::new(103).unwrap(),
            1 => changed[0].path = vec![StructuralPathSegment::Field("other".into())],
            2 => changed[0].access = StructuralAccess::Owned,
            3 => changed[0].path.push(StructuralPathSegment::FixedIndex(0)),
            4 => changed.swap(0, 1),
            _ => {
                changed.pop();
            }
        }
        assert_ne!(
            identity(&changed, &structural_parameters),
            expected,
            "structural argument mutation {mutation}"
        );
    }

    for mutation in 0..8 {
        let mut changed = structural_parameters.clone();
        match mutation {
            0 => changed[0].position = 7,
            1 => changed[0].is_self = true,
            2 => changed[0].structural_type = StructuralTypeId::new(91).unwrap(),
            3 => changed[0].multiplicity = StructuralMultiplicity::Affine,
            4 => changed[0].access = StructuralAccess::Owned,
            5 => changed[0]
                .qualifications
                .push(StructuralDomainId::new(3).unwrap()),
            6 => changed[0]
                .projected_qualifications
                .push(StructuralPathQualification {
                    path: vec![StructuralPathSegment::Field("leaf".into())],
                    domain: StructuralDomainId::new(5).unwrap(),
                }),
            _ => changed[0].place = PlaceId::new(203).unwrap(),
        }
        assert_ne!(
            identity(&structural_arguments, &changed),
            expected,
            "structural parameter mutation {mutation}"
        );
    }

    // The same structural rows beneath a different occurrence, requirement,
    // plan, target, execution, plan commitment, locator, or same-stack
    // custody still produce a distinct parent identity.
    for mutation in 0..8 {
        let changed = match mutation {
            0 => admitted_provider_boundary_trait_settlement_identity(
                &optimized_boundary_occurrence(
                    occurrence.terminal(),
                    occurrence.machine(),
                    OperationId::new(9).unwrap(),
                    occurrence.boundary(),
                    occurrence.operation_ordinal(),
                    OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(b"other occurrence"),
                ),
                "Input::read",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                execution,
                [9; 32],
                &locator,
                [11; 32],
                &structural_arguments,
                &structural_parameters,
            ),
            1 => admitted_provider_boundary_trait_settlement_identity(
                occurrence,
                "Input::other",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                execution,
                [9; 32],
                &locator,
                [11; 32],
                &structural_arguments,
                &structural_parameters,
            ),
            2 => admitted_provider_boundary_trait_settlement_identity(
                occurrence,
                "Input::read",
                NativeSelectedProviderPlanDigest::from_digest([8; 32]),
                NativeTarget::linux_x64(),
                execution,
                [9; 32],
                &locator,
                [11; 32],
                &structural_arguments,
                &structural_parameters,
            ),
            3 => admitted_provider_boundary_trait_settlement_identity(
                occurrence,
                "Input::read",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::windows_x64(),
                execution,
                [9; 32],
                &locator,
                [11; 32],
                &structural_arguments,
                &structural_parameters,
            ),
            4 => admitted_provider_boundary_trait_settlement_identity(
                occurrence,
                "Input::read",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                target_operations::ProviderExecutionBinding::from_execution_record(
                    target_operations::ProviderPlanReportIdentity::new(7).unwrap(),
                    11,
                    13,
                    17,
                    29,
                )
                .unwrap(),
                [9; 32],
                &locator,
                [11; 32],
                &structural_arguments,
                &structural_parameters,
            ),
            5 => admitted_provider_boundary_trait_settlement_identity(
                occurrence,
                "Input::read",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                execution,
                [10; 32],
                &locator,
                [11; 32],
                &structural_arguments,
                &structural_parameters,
            ),
            6 => admitted_provider_boundary_trait_settlement_identity(
                occurrence,
                "Input::read",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                execution,
                [9; 32],
                &locator,
                [12; 32],
                &structural_arguments,
                &structural_parameters,
            ),
            _ => admitted_provider_boundary_trait_settlement_identity(
                occurrence,
                "Input::read",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                execution,
                [9; 32],
                &target::normalize_foreign_locator(
                    target::ForeignLocatorCandidate::PeByName {
                        library: b"kernel32.dll".to_vec(),
                        export: b"WriteFile".to_vec(),
                    },
                    target::TargetProfile::WindowsX64,
                )
                .expect("valid normalized Windows import"),
                [11; 32],
                &structural_arguments,
                &structural_parameters,
            ),
        };
        assert_ne!(changed, expected, "binding input mutation {mutation}");
    }
}

/// The committed child identity is the only byte string the evidence and
/// artifact identities retain about one physical child, and the
/// retained-vs-derived comparison is what rejects a missing, duplicate,
/// stale, substituted, padded, or role-swapped child on replay. Every
/// retained field — parent role and identity, projection, occurrence role
/// and identity, all three byte spans, all three byte digests, and the
/// relocation disposition — must move that identity or a substitution in
/// the unbound field would replay as the same child.
#[test]
fn physical_child_identity_binds_every_retained_field() {
    use boundary_applications::{
        BoundaryApplication, BoundaryApplicationRealization,
        BoundaryApplicationRealizationCompanion, BoundaryNominalIdentity,
        BoundaryOperatorRequirement, TerminalBoundaryApplicationDemand,
        TerminalBoundaryApplicationDemands, TerminalBoundaryApplicationRealizations,
    };
    use semantic_vocabulary::{IntegerValue, OperationId, ValueId};
    use target_operations::{
        BoundaryRealization, BoundaryScalarArgument, CompilerBuiltinExecution,
    };

    use crate::physical::derivation::evidence::physical_child_identity;
    use crate::physical::model::native_byte_span;
    use crate::{
        BoundaryTraitSettlementParts, BoundaryTraitSettlementRole, CompilerBuiltinResult,
        CompilerBuiltinScalarArgument, NativeCompilerBuiltinCatalogIdentity, PhysicalChildParent,
        PhysicalRelocationDisposition,
    };

    let terminal = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
    };
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine");
    let operation = OperationId::new(41).expect("operation");
    let demands = TerminalBoundaryApplicationDemands::new(
        terminal,
        vec![TerminalBoundaryApplicationDemand::new(
            operation,
            BoundaryOperatorRequirement::new(
                BoundaryNominalIdentity::new("package:operator".to_owned()).unwrap(),
                "operator::call()->u64".to_owned(),
            )
            .unwrap(),
            BoundaryApplication::Empty,
        )],
    )
    .unwrap();
    let coverage_parent = |selected_plan_digest: [u8; 32]| {
        let realizations = TerminalBoundaryApplicationRealizations::new(
            &demands,
            vec![
                BoundaryApplicationRealizationCompanion::new(
                    operation,
                    selected_plan_digest,
                    BoundaryApplicationRealization::NongenericCheckedBody {
                        realization_machine: BoundaryNominalIdentity::new("machine".to_owned())
                            .unwrap(),
                        realization_state: BoundaryNominalIdentity::new(
                            "machine::entry".to_owned(),
                        )
                        .unwrap(),
                        realization_contract_commitment: [3; 32],
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut references = realizations.coverage_references(&demands).unwrap();
        let [reference] = references.as_mut_slice() else {
            panic!("one D29 coverage reference")
        };
        PhysicalChildParent::OperatorApplicationCoverage(*reference)
    };
    let settlement_parent = |identity: [u8; 32]| {
        PhysicalChildParent::BoundaryTraitSettlement(
            BoundaryTraitSettlementParts {
                occurrence: optimized_boundary_occurrence(
                    terminal,
                    machine,
                    OperationId::new(43).expect("boundary operation"),
                    semantic_vocabulary::BoundaryMachineId::new(44).expect("boundary"),
                    0,
                    OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(
                        b"boundary settlement occurrence",
                    ),
                ),
                requirement_identity: "Process::exit".to_owned(),
                selected_plan_digest: NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                target: NativeTarget::linux_x64(),
                role: BoundaryTraitSettlementRole::CompilerBuiltin {
                    catalog: NativeCompilerBuiltinCatalogIdentity::HostedV1,
                    execution: CompilerBuiltinExecution::HostedExitProcessI32,
                    realization: BoundaryRealization::HostedExitProcessI32(Default::default()),
                    scalar_argument: Some(CompilerBuiltinScalarArgument::Immediate(
                        BoundaryScalarArgument {
                            source_value: ValueId::new(45).unwrap(),
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                            ),
                            immediate: IntegerValue::Signed(0),
                            destination: calling_conventions::MachineRegister::X86Rdi,
                        },
                    )),
                    result: CompilerBuiltinResult::Unit,
                },
                identity,
            }
            .into(),
        )
    };

    let parent = coverage_parent([5; 32]);
    let projection = physical_projection().identity();
    let occurrence = NativePhysicalOccurrence::Operator(
        physical_projection().operator_occurrences()[0].identity(),
    );
    let machine_span = native_byte_span(8, 5);
    let object_span = native_byte_span(104, 5);
    let final_image_span = native_byte_span(232, 5);
    let identity = |parent: &PhysicalChildParent,
                    projection: NativeOptimizationProjectionIdentity,
                    occurrence: NativePhysicalOccurrence,
                    machine_span: crate::NativeByteSpan,
                    object_span: crate::NativeByteSpan,
                    final_image_span: crate::NativeByteSpan,
                    machine_bytes_digest: [u8; 32],
                    object_bytes_digest: [u8; 32],
                    final_image_bytes_digest: [u8; 32],
                    relocation: PhysicalRelocationDisposition| {
        physical_child_identity(
            parent,
            projection,
            occurrence,
            machine_span,
            object_span,
            final_image_span,
            machine_bytes_digest,
            object_bytes_digest,
            final_image_bytes_digest,
            relocation,
        )
    };
    let expected = identity(
        &parent,
        projection,
        occurrence,
        machine_span,
        object_span,
        final_image_span,
        [0xA1; 32],
        [0xA2; 32],
        [0xA3; 32],
        PhysicalRelocationDisposition::ResolvedInternalCall,
    );

    for mutation in 0..15 {
        let changed = match mutation {
            // A substituted parent: the same operation beneath a different
            // selected-plan digest is a different D29 coverage parent.
            0 => identity(
                &coverage_parent([6; 32]),
                projection,
                occurrence,
                machine_span,
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            // A role-swapped parent: identical identity bytes beneath the
            // boundary-settlement role tag still cannot collide with the
            // D29 coverage parent.
            1 => identity(
                &settlement_parent(parent.identity()),
                projection,
                occurrence,
                machine_span,
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            // A detached projection cannot replay the same child.
            2 => identity(
                &parent,
                NativeOptimizationProjectionIdentity::from_canonical_bytes(b"detached projection"),
                occurrence,
                machine_span,
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            // Occurrence role swap: the same identity bytes beneath the
            // boundary tag are not the operator occurrence.
            3 => identity(
                &parent,
                projection,
                NativePhysicalOccurrence::Boundary(
                    OptimizedBoundaryOccurrenceIdentity::from_bytes(occurrence.identity()),
                ),
                machine_span,
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            // A substituted occurrence identity.
            4 => identity(
                &parent,
                projection,
                NativePhysicalOccurrence::Operator(
                    OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"stale occurrence"),
                ),
                machine_span,
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            5 => identity(
                &parent,
                projection,
                occurrence,
                native_byte_span(9, 5),
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            6 => identity(
                &parent,
                projection,
                occurrence,
                native_byte_span(8, 6),
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            7 => identity(
                &parent,
                projection,
                occurrence,
                machine_span,
                native_byte_span(105, 5),
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            8 => identity(
                &parent,
                projection,
                occurrence,
                machine_span,
                native_byte_span(104, 6),
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            9 => identity(
                &parent,
                projection,
                occurrence,
                machine_span,
                object_span,
                native_byte_span(233, 5),
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            10 => identity(
                &parent,
                projection,
                occurrence,
                machine_span,
                object_span,
                native_byte_span(232, 6),
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            11 => identity(
                &parent,
                projection,
                occurrence,
                machine_span,
                object_span,
                final_image_span,
                [0xB1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            12 => identity(
                &parent,
                projection,
                occurrence,
                machine_span,
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xB2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            13 => identity(
                &parent,
                projection,
                occurrence,
                machine_span,
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xB3; 32],
                PhysicalRelocationDisposition::ResolvedInternalCall,
            ),
            // A different relocation disposition on identical spans and
            // digests is a different child.
            _ => identity(
                &parent,
                projection,
                occurrence,
                machine_span,
                object_span,
                final_image_span,
                [0xA1; 32],
                [0xA2; 32],
                [0xA3; 32],
                PhysicalRelocationDisposition::DirectInstructionBytes,
            ),
        };
        assert_ne!(changed, expected, "child field mutation {mutation}");
    }
}

/// The normalized-foreign relocation is the one physical-child custody
/// record no emitted-code replay leg exercises: its locator, boundary
/// plan, object symbol, relocation origin, byte geometry, addend, kind,
/// callback custody, and final-image symbol identity must each move the
/// committed child identity or a substituted import site would replay as
/// the same child.
#[test]
fn physical_child_identity_binds_the_normalized_foreign_relocation() {
    use function_identity::{MachineFunctionIdentity, StateKey};
    use object_file::{ObjectSymbolHandle, RelocationKind, RelocationOrigin};
    use semantic_vocabulary::{IntegerValue, ValueId};
    use target_operations::{
        BoundaryRealization, BoundaryScalarArgument, CompilerBuiltinExecution,
    };

    use crate::physical::derivation::evidence::physical_child_identity;
    use crate::physical::model::{
        native_byte_span, normalized_foreign_call_relocation,
        normalized_foreign_callback_relocation,
    };
    use crate::{
        BoundaryTraitSettlementParts, BoundaryTraitSettlementRole, CompilerBuiltinResult,
        CompilerBuiltinScalarArgument, NativeCompilerBuiltinCatalogIdentity,
        NormalizedForeignCallbackRelocations, PhysicalChildParent, PhysicalRelocationDisposition,
    };

    let projection = physical_projection();
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: projection.boundary_occurrences()[0],
            requirement_identity: "Process::exit".to_owned(),
            selected_plan_digest: NativeSelectedProviderPlanDigest::from_digest([7; 32]),
            target: NativeTarget::linux_x64(),
            role: BoundaryTraitSettlementRole::CompilerBuiltin {
                catalog: NativeCompilerBuiltinCatalogIdentity::HostedV1,
                execution: CompilerBuiltinExecution::HostedExitProcessI32,
                realization: BoundaryRealization::HostedExitProcessI32(Default::default()),
                scalar_argument: Some(CompilerBuiltinScalarArgument::Immediate(
                    BoundaryScalarArgument {
                        source_value: ValueId::new(45).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        ),
                        immediate: IntegerValue::Signed(0),
                        destination: calling_conventions::MachineRegister::X86Rdi,
                    },
                )),
                result: CompilerBuiltinResult::Unit,
            },
            identity: [21; 32],
        }
        .into(),
    );
    let occurrence =
        NativePhysicalOccurrence::Boundary(projection.boundary_occurrences()[0].identity());
    let machine_span = native_byte_span(8, 5);
    let object_span = native_byte_span(104, 5);
    let final_image_span = native_byte_span(232, 5);
    let child_identity = |relocation: PhysicalRelocationDisposition| {
        physical_child_identity(
            &parent,
            projection.identity(),
            occurrence,
            machine_span,
            object_span,
            final_image_span,
            [0xA1; 32],
            [0xA2; 32],
            [0xA3; 32],
            relocation,
        )
    };

    let callback_relocation = |offset: usize| {
        normalized_foreign_callback_relocation(
            ObjectSymbolHandle::from_arena_index(17),
            RelocationOrigin::Materialization {
                object_symbol_handle: ObjectSymbolHandle::from_arena_index(19),
            },
            offset,
            4,
            0,
            RelocationKind::X86_64Relative32,
        )
    };
    let foreign = |locator_identity: [u8; 32],
                   boundary_plan_identity: [u8; 32],
                   object_symbol: ObjectSymbolHandle,
                   origin: RelocationOrigin,
                   offset: usize,
                   byte_width: usize,
                   addend: i64,
                   kind: RelocationKind,
                   callback: Option<NormalizedForeignCallbackRelocations>,
                   final_image_symbol_identity: [u8; 32]| {
        PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(
            normalized_foreign_call_relocation(
                locator_identity,
                boundary_plan_identity,
                object_symbol,
                origin,
                offset,
                byte_width,
                addend,
                kind,
                callback,
                final_image_symbol_identity,
            ),
        )
    };
    let origin = |operation_identity: u64| RelocationOrigin::SemanticOperation {
        function_symbol_handle: ObjectSymbolHandle::from_arena_index(5),
        operation_identity,
    };
    let callback = Some(NormalizedForeignCallbackRelocations::X86_64Relative32 {
        callback_function: MachineFunctionIdentity::default(),
        relocation: callback_relocation(23),
    });
    let expected = child_identity(foreign(
        [9; 32],
        [11; 32],
        ObjectSymbolHandle::from_arena_index(3),
        origin(7),
        13,
        4,
        -4,
        RelocationKind::X86_64Relative32,
        callback,
        [29; 32],
    ));

    for mutation in 0..15 {
        let changed = match mutation {
            // Substituted locator or boundary-plan custody.
            0 => foreign(
                [10; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            1 => foreign(
                [9; 32],
                [12; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            // Substituted or regenerated object symbol.
            2 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(4),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            3 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_parts(3, 2),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            // Origin drift: a different operation identity, a different
            // origin namespace carrying the same coordinate, or a
            // different owning symbol.
            4 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(8),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            5 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                RelocationOrigin::SemanticEdge {
                    function_symbol_handle: ObjectSymbolHandle::from_arena_index(5),
                    edge_identity: 7,
                },
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            6 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                RelocationOrigin::SemanticOperation {
                    function_symbol_handle: ObjectSymbolHandle::from_arena_index(6),
                    operation_identity: 7,
                },
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            // Byte geometry, addend, and encoding kind.
            7 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                14,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            8 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                8,
                -4,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            9 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                4,
                -8,
                RelocationKind::X86_64Relative32,
                callback,
                [29; 32],
            ),
            10 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::Aarch64Branch26,
                callback,
                [29; 32],
            ),
            // Callback custody: dropped, swapped encoding, a different
            // callback function, or a different callback relocation.
            11 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                None,
                [29; 32],
            ),
            12 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                Some(NormalizedForeignCallbackRelocations::Aarch64PageAddress {
                    callback_function: MachineFunctionIdentity::default(),
                    page: callback_relocation(23),
                    page_offset: callback_relocation(23),
                }),
                [29; 32],
            ),
            13 => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                Some(NormalizedForeignCallbackRelocations::X86_64Relative32 {
                    callback_function: MachineFunctionIdentity::source(StateKey {
                        segment_index: 1,
                        ..Default::default()
                    }),
                    relocation: callback_relocation(23),
                }),
                [29; 32],
            ),
            _ => foreign(
                [9; 32],
                [11; 32],
                ObjectSymbolHandle::from_arena_index(3),
                origin(7),
                13,
                4,
                -4,
                RelocationKind::X86_64Relative32,
                Some(NormalizedForeignCallbackRelocations::X86_64Relative32 {
                    callback_function: MachineFunctionIdentity::default(),
                    relocation: callback_relocation(24),
                }),
                [29; 32],
            ),
        };
        let changed = child_identity(changed);
        assert_ne!(changed, expected, "relocation custody mutation {mutation}");
    }

    // The final-image symbol identity is retained beside the relocation
    // rather than inside it.
    assert_ne!(
        child_identity(foreign(
            [9; 32],
            [11; 32],
            ObjectSymbolHandle::from_arena_index(3),
            origin(7),
            13,
            4,
            -4,
            RelocationKind::X86_64Relative32,
            callback,
            [30; 32],
        )),
        expected,
        "final-image symbol identity mutation"
    );
}

/// A projection that repeats one surviving occurrence has already failed
/// the survivor bijection before any child coordinate is compared, and a
/// boundary survivor presented under the operator tag is a substituted
/// occurrence rather than a boundary child.
#[test]
fn physical_child_coordinates_reject_repeated_and_cross_role_occurrences() {
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
    let repeated = native_optimization_projection(
        terminal,
        vec![operator, operator],
        vec![boundary],
        NativeOptimizationProjectionIdentity::from_canonical_bytes(b"repeated projection"),
    );
    assert_eq!(
        validate_exact_physical_child_coordinates(&repeated, Vec::new()),
        Err("native physical evidence projection repeats an optimized occurrence")
    );

    let projection = physical_projection();
    let [operator_coordinate, boundary_coordinate] = exact_coordinates(&projection);

    // A boundary survivor repeated as a boundary child.
    assert_eq!(
        validate_exact_physical_child_coordinates(
            &projection,
            [
                operator_coordinate,
                boundary_coordinate,
                boundary_coordinate
            ],
        ),
        Err("native physical evidence contains duplicate optimized occurrences")
    );

    // A boundary survivor's identity presented under the operator tag is
    // a substituted occurrence, not the boundary child.
    let substituted = PhysicalChildCoordinate {
        projection: projection.identity(),
        occurrence: NativePhysicalOccurrence::Operator(
            OptimizedOperatorOccurrenceIdentity::from_bytes(
                projection.boundary_occurrences()[0].identity().bytes(),
            ),
        ),
        parent_role: 1,
    };
    assert_eq!(
        validate_exact_physical_child_coordinates(
            &projection,
            [operator_coordinate, boundary_coordinate, substituted],
        ),
        Err("native physical child swapped or substituted its semantic parent role")
    );

    // A boundary child carrying the operator parent role is a role swap.
    let swapped = PhysicalChildCoordinate {
        parent_role: 1,
        ..boundary_coordinate
    };
    assert_eq!(
        validate_exact_physical_child_coordinates(&projection, [operator_coordinate, swapped],),
        Err("native physical child swapped or substituted its semantic parent role")
    );
}

#[test]
fn physical_evidence_gap_identity_binds_the_exact_subject() {
    use crate::physical::derivation::hashing::physical_evidence_gap_identity;
    use crate::physical::model::NativePhysicalEvidenceGapSubject;
    use semantic_vocabulary::{EdgeId, ServiceId};
    use target_operations::CallSiteOwner;

    let projection = physical_projection();
    let boundary = projection.boundary_occurrences()[0];
    let operator = projection.operator_occurrences()[0];
    let machine = semantic_vocabulary::MachineId::new(9).expect("machine");

    let subjects = [
        NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner {
            machine,
            owner: CallSiteOwner::CleanupAction {
                edge: EdgeId::new(11).expect("edge"),
                action_ordinal: 2,
            },
        },
        NativePhysicalEvidenceGapSubject::UnsupportedSettlementRealization {
            occurrence: boundary,
        },
        NativePhysicalEvidenceGapSubject::UnsupportedNormalizedForeignCall {
            occurrence: boundary,
        },
        NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence {
            occurrence: boundary,
        },
        NativePhysicalEvidenceGapSubject::UnsupportedOperatorSpan {
            occurrence: operator,
        },
        NativePhysicalEvidenceGapSubject::UnownedPortEffect {
            machine,
            psi_operation: boundary.operation(),
            service: ServiceId::new(5).expect("service"),
            port: 7,
            value: 9,
            operation_ordinal: 1,
            code_offset: 2,
            byte_count: 3,
        },
    ];
    // The encoding is deterministic.
    for subject in &subjects {
        assert_eq!(
            physical_evidence_gap_identity(subject),
            physical_evidence_gap_identity(subject)
        );
    }
    // Distinct subjects never share an identity.
    for (left_index, left) in subjects.iter().enumerate() {
        for (right_index, right) in subjects.iter().enumerate() {
            if left_index != right_index {
                assert_ne!(
                    physical_evidence_gap_identity(left),
                    physical_evidence_gap_identity(right),
                    "subjects {left_index} and {right_index} must not collide"
                );
            }
        }
    }
    // A single mutated field inside one variant diverges.
    let NativePhysicalEvidenceGapSubject::UnownedPortEffect {
        machine: moved_machine,
        psi_operation,
        service,
        port,
        value,
        operation_ordinal,
        code_offset,
        ..
    } = subjects[5]
    else {
        unreachable!("subjects[5] is an unowned port effect");
    };
    let moved = NativePhysicalEvidenceGapSubject::UnownedPortEffect {
        machine: moved_machine,
        psi_operation,
        service,
        port,
        value,
        operation_ordinal,
        code_offset,
        byte_count: 4,
    };
    assert_ne!(
        physical_evidence_gap_identity(&subjects[5]),
        physical_evidence_gap_identity(&moved)
    );
    let other_owner = NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner {
        machine,
        owner: CallSiteOwner::Operation(
            semantic_vocabulary::OperationId::new(13).expect("operation"),
        ),
    };
    assert_ne!(
        physical_evidence_gap_identity(&subjects[0]),
        physical_evidence_gap_identity(&other_owner)
    );
}

#[test]
fn physical_evidence_gap_names_the_blocking_occurrence() {
    use crate::physical::derivation::hashing::physical_evidence_gap_identity;
    use crate::physical::model::NativePhysicalEvidenceGapSubject;
    use crate::physical::model::native_physical_evidence_gap;
    use semantic_vocabulary::EdgeId;
    use target_operations::CallSiteOwner;

    let projection = physical_projection();
    let boundary = projection.boundary_occurrences()[0];
    let operator = projection.operator_occurrences()[0];
    let machine = semantic_vocabulary::MachineId::new(9).expect("machine");

    let gap =
        |subject| native_physical_evidence_gap(subject, physical_evidence_gap_identity(&subject));
    let blocked = gap(
        NativePhysicalEvidenceGapSubject::UnsupportedSettlementRealization {
            occurrence: boundary,
        },
    );
    assert_eq!(
        blocked.occurrence(),
        Some(NativePhysicalOccurrence::Boundary(boundary.identity()))
    );
    let blocked = gap(NativePhysicalEvidenceGapSubject::UnsupportedOperatorSpan {
        occurrence: operator,
    });
    assert_eq!(
        blocked.occurrence(),
        Some(NativePhysicalOccurrence::Operator(operator.identity()))
    );
    // Machine-level and retained-record subjects carry no occurrence.
    assert_eq!(
        gap(NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner {
            machine,
            owner: CallSiteOwner::CleanupAction {
                edge: EdgeId::new(11).expect("edge"),
                action_ordinal: 2,
            },
        })
        .occurrence(),
        None
    );
    assert_eq!(
        gap(NativePhysicalEvidenceGapSubject::UnownedPortEffect {
            machine,
            psi_operation: boundary.operation(),
            service: semantic_vocabulary::ServiceId::new(5).expect("service"),
            port: 7,
            value: 9,
            operation_ordinal: 1,
            code_offset: 2,
            byte_count: 3,
        })
        .occurrence(),
        None
    );
}
