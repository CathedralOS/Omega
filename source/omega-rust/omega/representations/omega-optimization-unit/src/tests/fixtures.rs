use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use psi_core::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    SemanticFingerprint, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalPsiIdentity,
    VocabularyMarker,
};

pub(super) fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero test identity")
}

pub(super) fn plan() -> AbstractOperationPlan {
    let machine = id(1, MachineId::new);
    let block = id(2, BlockId::new);
    let value = id(3, ValueId::new);
    let result = id(4, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("valid width");
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: vec![AbstractParameter {
                value,
                scalar_type: ScalarType::Integer(integer),
            }],
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(omega_abstract_operations::AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(integer),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: id(5, OperationId::new),
                    result,
                    scalar_type: ScalarType::Integer(integer),
                    value: IntegerValue::Unsigned(9),
                },
                AbstractOperation::Return {
                    psi_edge: id(6, EdgeId::new),
                    result,
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn write_only_store_plan() -> AbstractOperationPlan {
    let machine = id(70, MachineId::new);
    let block = id(71, BlockId::new);
    let value = id(72, ValueId::new);
    let place = id(73, PlaceId::new);
    let structural_type = id(74, StructuralTypeId::new);
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let destination = StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([70; 32]),
        },
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::i32".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: vec![AbstractParameter { value, scalar_type }],
            structural_parameters: vec![destination.clone()],
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::WriteOnlyPrimitiveStore {
                    psi_operation: id(75, OperationId::new),
                    destination,
                    value: AbstractResult { value, scalar_type },
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: id(76, EdgeId::new),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn structural_scalar_fields_plan() -> AbstractOperationPlan {
    let machine = id(80, MachineId::new);
    let block = id(81, BlockId::new);
    let stored_value = id(82, ValueId::new);
    let read_value = id(83, ValueId::new);
    let place = id(84, PlaceId::new);
    let structural_type = id(85, StructuralTypeId::new);
    let field = id(86, StructuralFieldId::new);
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let structural_parameter = StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: true,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([80; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: Some(structural_type),
            entry: block,
            parameters: vec![AbstractParameter {
                value: stored_value,
                scalar_type,
            }],
            structural_parameters: vec![structural_parameter.clone()],
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: read_value,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::StructuralScalarFieldStore {
                    psi_operation: id(87, OperationId::new),
                    destination: structural_parameter.clone(),
                    path: vec![StructuralPathSegment::Field("payload".into())],
                    field,
                    value: AbstractResult {
                        value: stored_value,
                        scalar_type,
                    },
                },
                AbstractOperation::IntegerStructuralField {
                    psi_operation: id(88, OperationId::new),
                    result: AbstractResult {
                        value: read_value,
                        scalar_type,
                    },
                    source: structural_parameter,
                    field,
                },
                AbstractOperation::Return {
                    psi_edge: id(89, EdgeId::new),
                    result: read_value,
                    value: read_value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}
