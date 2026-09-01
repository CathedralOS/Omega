use super::super::{id, refresh_identity, unit};
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

pub(crate) fn boolean_structural_field_unit() -> PsiOptimizationUnit {
    let machine = id(4_700, MachineId::new);
    let block = id(4_701, BlockId::new);
    let place = id(4_702, PlaceId::new);
    let structural_type = id(4_703, StructuralTypeId::new);
    let field = id(4_704, psi_core::StructuralFieldId::new);
    let scalar_parameter = id(4_705, ValueId::new);
    let result = id(4_706, ValueId::new);
    let cleanup_machine = id(4_709, MachineId::new);
    let cleanup_block = id(4_710, BlockId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([47; 32]),
            },
            entry: machine,
            structural_types: vec![psi_terminal::StructuralTypeDeclaration {
                id: structural_type,
                identity: "validation::observed-affine-record".into(),
                shape: psi_terminal::StructuralTypeShape::Record {
                    fields: vec![psi_terminal::StructuralFieldDeclaration {
                        id: field,
                        identity: "ready".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
                    }],
                },
            }],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: vec![AbstractParameter {
                        value: scalar_parameter,
                        scalar_type: ScalarType::Boolean,
                    }],
                    structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
                        place,
                        position: 0,
                        is_self: false,
                        structural_type,
                        multiplicity: psi_terminal::StructuralMultiplicity::Affine,
                        access: psi_terminal::StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    }],
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        AbstractOperation::BooleanStructuralField {
                            psi_operation: id(4_707, OperationId::new),
                            result,
                            source: place,
                            field,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(4_708, EdgeId::new),
                            result,
                            value: result,
                            scalar_type: ScalarType::Boolean,
                            cleanup_actions: vec![
                                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(
                                    psi_terminal::NominalAffineCleanup {
                                        place,
                                        structural_type,
                                        cleanup_machine,
                                        cleanup_receiver: None,
                                        requirement_obligations: Vec::new(),
                                    },
                                ),
                            ],
                        },
                    ],
                },
                AbstractFunction {
                    machine: cleanup_machine,
                    attachment: Some(structural_type),
                    entry: cleanup_block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: cleanup_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![AbstractOperation::ReturnUnit {
                        psi_edge: id(4_711, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    }],
                },
            ],
        },
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("Boolean structural-field fixture")
}

pub(crate) fn direct_realization_boolean_structural_field_unit() -> PsiOptimizationUnit {
    let mut unit = boolean_structural_field_unit();
    let realization_type = unit.structural_types[0].id;
    unit.entry = unit.functions[1].machine;

    let realization = &mut unit.functions[0];
    realization.attachment = Some(realization_type);
    realization.parameters.clear();
    let parameter = &mut realization.structural_parameters[0];
    parameter.is_self = true;
    parameter.multiplicity = psi_terminal::StructuralMultiplicity::Unrestricted;
    parameter.access = psi_terminal::StructuralAccess::SharedBorrow;
    let place = realization
        .structural_places
        .iter_mut()
        .find(|place| place.id == parameter.place)
        .expect("direct realization self place");
    place.kind = psi_core::StructuralPlaceKind::Parameter {
        position: parameter.position,
        is_self: true,
    };
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut realization
        .blocks
        .first_mut()
        .and_then(|block| block.nodes.last_mut())
        .expect("direct realization block ends in a node")
        .operation
    else {
        panic!("direct realization fixture ends in a scalar return")
    };
    cleanup_actions.clear();

    super::super::refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn direct_realization_integer_structural_field_unit() -> PsiOptimizationUnit {
    let mut unit = boolean_structural_field_unit();
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let structural_type = unit.structural_types[0].id;
    let psi_terminal::StructuralTypeShape::Record { fields } = &unit.structural_types[0].shape
    else {
        unreachable!()
    };
    let field = fields.first().expect("fixture field").id;
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut unit.structural_types[0].shape
    else {
        unreachable!()
    };
    fields[0].field_type = psi_terminal::StructuralFieldType::Scalar(integer);

    let function = &mut unit.functions[0];
    function.attachment = Some(structural_type);
    function.parameters.clear();
    let parameter = &mut function.structural_parameters[0];
    parameter.is_self = true;
    parameter.multiplicity = psi_terminal::StructuralMultiplicity::Unrestricted;
    parameter.access = psi_terminal::StructuralAccess::SharedBorrow;
    let source = parameter.clone();
    let place = function
        .structural_places
        .iter_mut()
        .find(|place| place.id == source.place)
        .expect("integer realization self place");
    place.kind = psi_core::StructuralPlaceKind::Parameter {
        position: source.position,
        is_self: true,
    };
    let result = function.result.scalar().expect("scalar result").value;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: result,
        scalar_type: integer,
    });
    function.blocks[0].nodes[0].operation = AbstractOperation::IntegerStructuralField {
        psi_operation: id(4_707, OperationId::new),
        result: AbstractResult {
            value: result,
            scalar_type: integer,
        },
        source,
        field,
    };
    function.blocks[0]
        .nodes
        .last_mut()
        .expect("integer realization return")
        .operation = AbstractOperation::Return {
        psi_edge: id(4_708, EdgeId::new),
        result,
        value: result,
        scalar_type: integer,
        cleanup_actions: Vec::new(),
    };
    super::super::refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn structural_scalar_field_store_unit() -> PsiOptimizationUnit {
    let mut unit = direct_realization_integer_structural_field_unit();
    let item_type = unit.structural_types[0].id;
    let psi_terminal::StructuralTypeShape::Record { fields } = &unit.structural_types[0].shape
    else {
        unreachable!()
    };
    let value_field = fields.first().expect("fixture value field").id;
    let owner_type = id(4_699, StructuralTypeId::new);
    unit.structural_types
        .push(psi_terminal::StructuralTypeDeclaration {
            id: owner_type,
            identity: "validation::store-owner".into(),
            shape: psi_terminal::StructuralTypeShape::Record {
                fields: vec![psi_terminal::StructuralFieldDeclaration {
                    id: id(4_699, psi_core::StructuralFieldId::new),
                    identity: "item".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: psi_terminal::StructuralFieldType::Structural(item_type),
                }],
            },
        });
    unit.structural_types
        .sort_by_key(|declaration| declaration.id);

    let function = &mut unit.functions[0];
    function.attachment = Some(owner_type);
    let destination = &mut function.structural_parameters[0];
    destination.structural_type = owner_type;
    destination.access = psi_terminal::StructuralAccess::MutableBorrow;
    let destination = destination.clone();
    let value = function.result.scalar().expect("integer result").value;
    let integer = function
        .result
        .scalar()
        .expect("integer result")
        .scalar_type;
    let mut store = function.blocks[0].nodes[0].clone();
    function.blocks[0].nodes[0].operation = AbstractOperation::IntegerConstant {
        psi_operation: id(4_707, OperationId::new),
        result: value,
        scalar_type: integer,
        value: IntegerValue::Signed(17),
    };
    store.operation = AbstractOperation::StructuralScalarFieldStore {
        psi_operation: id(4_712, OperationId::new),
        destination,
        path: vec![psi_terminal::StructuralPathSegment::Field("item".into())],
        field: value_field,
        value: AbstractResult {
            value,
            scalar_type: integer,
        },
    };
    function.blocks[0].nodes.insert(1, store);
    function.result = AbstractFunctionResult::Unit;
    function.blocks[0]
        .nodes
        .last_mut()
        .expect("store fixture return")
        .operation = AbstractOperation::ReturnUnit {
        psi_edge: id(4_708, EdgeId::new),
        cleanup_actions: Vec::new(),
    };
    super::super::refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn structural_field(
    raw: u64,
    target: StructuralTypeId,
) -> psi_terminal::StructuralFieldDeclaration {
    structural_leaf_field(
        raw,
        psi_terminal::BindingRelevance::Relevant,
        psi_terminal::StructuralFieldType::Structural(target),
    )
}

pub(crate) fn structural_leaf_field(
    raw: u64,
    relevance: psi_terminal::BindingRelevance,
    field_type: psi_terminal::StructuralFieldType,
) -> psi_terminal::StructuralFieldDeclaration {
    psi_terminal::StructuralFieldDeclaration {
        id: id(raw, psi_core::StructuralFieldId::new),
        identity: format!("validation::field-{raw}"),
        relevance,
        field_type,
    }
}

pub(crate) fn structural_case(
    raw: u64,
    fields: Vec<psi_terminal::StructuralFieldDeclaration>,
) -> psi_terminal::StructuralCaseDeclaration {
    psi_terminal::StructuralCaseDeclaration {
        id: id(raw, psi_core::StructuralCaseId::new),
        identity: format!("validation::case-{raw}"),
        fields,
    }
}

pub(crate) fn structural_type(
    raw: u64,
    shape: psi_terminal::StructuralTypeShape,
) -> psi_terminal::StructuralTypeDeclaration {
    psi_terminal::StructuralTypeDeclaration {
        id: id(raw, StructuralTypeId::new),
        identity: format!("validation::type-{raw}"),
        shape,
    }
}

pub(crate) fn structural_catalog_unit(
    structural_types: Vec<psi_terminal::StructuralTypeDeclaration>,
) -> PsiOptimizationUnit {
    let mut candidate = unit();
    candidate.structural_types = structural_types;
    refresh_identity(&mut candidate);
    candidate
}
