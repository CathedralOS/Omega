//! Lowering caller store and realization store operations.

use crate::unit::{
    LoweringError, PrimitiveType, allocate_dense, contract_id, integer_landing_scalar_type,
    integer_value, lookup_type_id, lower_structural_path, operation_id, terminal_scalar_type,
    unsupported, value_id,
};
use checked_trees::{
    CheckedBooleanExpression, CheckedDynamicScalarCallPlan, CheckedScalarExpression,
    CheckedStructuralPredicatePathSegment,
};
use terminal_psi::{
    MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralParameterDeclaration, ValueDeclaration,
};

pub(crate) fn lower_caller_store_operations(
    plan: &CheckedDynamicScalarCallPlan,
    caller_self: &StructuralParameterDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<Vec<Operation>, LoweringError> {
    let Some(store) = &plan.caller_structural_scalar_field_store else {
        return Ok(Vec::new());
    };
    if caller_self.access != StructuralAccess::MutableBorrow
        || store.destination.parameter_position() != Some(caller_self.position)
        || store.carrier_path != plan.source_path
    {
        return unsupported("direct dynamic caller store lost mutable carrier custody");
    }
    let source_type = lookup_type_id(type_ids, &plan.source_type_identity)?;
    let declaration = structural_types
        .iter()
        .find(|declaration| declaration.id == source_type)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic store carrier type is absent",
        ))?;
    let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return unsupported("direct dynamic store carrier must be a record");
    };
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    let matching = fields
        .iter()
        .filter(|field| {
            field.identity == store.field_identity
                && !field.relevance.is_erased()
                && field.field_type == terminal_psi::StructuralFieldType::Scalar(scalar_type)
        })
        .collect::<Vec<_>>();
    let [field] = matching.as_slice() else {
        return unsupported("direct dynamic store field is absent or ambiguous");
    };
    let constant = match store.value.as_pure().ok_or(LoweringError::Unsupported(
        "direct dynamic store computation is unsupported",
    ))? {
        CheckedScalarExpression::IntegerLiteral { literal }
            if store.primitive_type.accepts_integer_literal()
                && store.primitive_type != PrimitiveType::Addr =>
        {
            if integer_landing_scalar_type(literal)? != scalar_type {
                return unsupported("direct dynamic store integer landing drifted");
            }
            OperationKind::IntegerConstant {
                value: integer_value(literal, scalar_type)?,
            }
        }
        CheckedScalarExpression::Boolean(boolean)
            if store.primitive_type == PrimitiveType::Bool =>
        {
            let CheckedBooleanExpression::Constant(value) = boolean.as_ref() else {
                return unsupported("direct dynamic store Boolean value is not constant");
            };
            OperationKind::BooleanConstant { value: *value }
        }
        _ => return unsupported("direct dynamic store value is unsupported"),
    };
    Ok(vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(1),
                scalar_type,
            }),
            kind: constant,
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: caller_self.place,
                path: lower_structural_path(&store.carrier_path),
                field: field.id,
                value: value_id(1),
                range_obligation: None,
            },
        },
    ])
}

pub(crate) fn lower_realization_operations(
    stores: &[checked_trees::CheckedStructuralScalarFieldStorePlan],
    expression: &CheckedScalarExpression,
    expected: semantic_vocabulary::ScalarType,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_operation: &mut u64,
    next_value: &mut u64,
) -> Result<Vec<Operation>, LoweringError> {
    let mut operations = lower_realization_store_operations(
        stores,
        parameter,
        structural_types,
        next_operation,
        next_value,
    )?;
    let operation = operation_id(allocate_dense(next_operation)?);
    let value = value_id(allocate_dense(next_value)?);
    if let CheckedScalarExpression::Boolean(boolean) = expression
        && let CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } = boolean.as_ref()
    {
        let [CheckedStructuralPredicatePathSegment::Field(field_identity)] = path.as_slice() else {
            return unsupported("direct dynamic realization field path is unsupported");
        };
        if *parameter_position != 0 || expected != semantic_vocabulary::ScalarType::Boolean {
            return unsupported("direct dynamic realization field result does not match self");
        }
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic realization self type is absent",
            ))?;
        let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("direct dynamic realization self must be a record");
        };
        let matching = fields
            .iter()
            .filter(|field| {
                field.identity == *field_identity
                    && field.field_type
                        == terminal_psi::StructuralFieldType::Scalar(
                            semantic_vocabulary::ScalarType::Boolean,
                        )
            })
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return unsupported("direct dynamic realization Boolean field is absent or ambiguous");
        };
        operations.push(Operation {
            static_reach_binding: None,
            id: operation,
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value,
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: parameter.place,
                path: Vec::new(),
                field: field.id,
            },
        });
        return Ok(operations);
    }

    if let CheckedScalarExpression::StructuralParameterField {
        parameter_position,
        path,
        primitive_type: PrimitiveType::I32,
    } = expression
    {
        let [CheckedStructuralPredicatePathSegment::Field(field_identity)] = path.as_slice() else {
            return unsupported("direct dynamic realization integer field path is unsupported");
        };
        if *parameter_position != 0 || expected != terminal_scalar_type(PrimitiveType::I32)? {
            return unsupported(
                "direct dynamic realization integer field result does not match self",
            );
        }
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic realization self type is absent",
            ))?;
        let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("direct dynamic realization self must be a record");
        };
        let matching = fields
            .iter()
            .filter(|field| {
                field.identity == *field_identity
                    && field.field_type.scalar_type() == Some(expected)
            })
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return unsupported("direct dynamic realization integer field is absent or ambiguous");
        };
        operations.push(Operation {
            static_reach_binding: None,
            id: operation,
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value,
                scalar_type: expected,
            }),
            kind: OperationKind::IntegerStructuralField {
                source: parameter.place,
                path: Vec::new(),
                field: field.id,
            },
        });
        return Ok(operations);
    }

    unsupported("direct dynamic realization must return one exact Boolean or i32 self field")
}

fn lower_realization_store_operations(
    stores: &[checked_trees::CheckedStructuralScalarFieldStorePlan],
    parameter: &StructuralParameterDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_operation: &mut u64,
    next_value: &mut u64,
) -> Result<Vec<Operation>, LoweringError> {
    if stores.len() > 3 {
        return unsupported("dynamic realization has too many structural stores");
    }
    let mut operations = Vec::with_capacity(stores.len() * 2);
    for (statement_index, store) in stores.iter().enumerate() {
        if stores[..statement_index].iter().any(|earlier| {
            earlier.carrier_path == store.carrier_path
                && earlier.field_identity == store.field_identity
        }) {
            return unsupported("dynamic realization repeats a structural store destination");
        }
        operations.extend(lower_realization_store_operation(
            store,
            statement_index,
            parameter,
            structural_types,
            next_operation,
            next_value,
        )?);
    }
    Ok(operations)
}

fn lower_realization_store_operation(
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    statement_index: usize,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_operation: &mut u64,
    next_value: &mut u64,
) -> Result<Vec<Operation>, LoweringError> {
    let expected_statement_index = u32::try_from(statement_index)
        .map_err(|_| LoweringError::Unsupported("dynamic store index exceeds u32"))?;
    let lowered =
        crate::emission::structural_scalar_store::lower_structural_scalar_store_destination(
            store,
            expected_statement_index,
            parameter,
            structural_types,
            &[],
            &[],
            crate::emission::structural_scalar_store::StoreAccessPolicy::MutableOnly,
        )?;
    let scalar_type = lowered.scalar_type;
    if lowered.requires_range_obligation {
        return unsupported("dynamic realization store requires a range-proof emission context");
    }
    let constant = match store.value.as_pure().ok_or(LoweringError::Unsupported(
        "dynamic realization store computation is unsupported",
    ))? {
        CheckedScalarExpression::IntegerLiteral { literal }
            if store.primitive_type.accepts_integer_literal()
                && store.primitive_type != PrimitiveType::Addr =>
        {
            if integer_landing_scalar_type(literal)? != scalar_type {
                return unsupported("dynamic realization store integer landing drifted");
            }
            OperationKind::IntegerConstant {
                value: integer_value(literal, scalar_type)?,
            }
        }
        CheckedScalarExpression::Boolean(boolean)
            if store.primitive_type == PrimitiveType::Bool =>
        {
            let CheckedBooleanExpression::Constant(value) = boolean.as_ref() else {
                return unsupported("dynamic realization store Boolean value is not constant");
            };
            OperationKind::BooleanConstant { value: *value }
        }
        _ => return unsupported("dynamic realization store value is unsupported"),
    };
    let constant_operation = operation_id(allocate_dense(next_operation)?);
    let constant_value = value_id(allocate_dense(next_value)?);
    let store_operation = operation_id(allocate_dense(next_operation)?);
    Ok(vec![
        Operation {
            static_reach_binding: None,
            id: constant_operation,
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: constant_value,
                scalar_type,
            }),
            kind: constant,
        },
        Operation {
            static_reach_binding: None,
            id: store_operation,
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: parameter.place,
                path: lowered.path,
                field: lowered.field,
                value: constant_value,
                range_obligation: None,
            },
        },
    ])
}

pub(crate) fn empty_terminal_contract(identity: u64) -> MachineContract {
    MachineContract {
        id: contract_id(identity),
        crash_routes: Vec::new(),
        erased_scalar_formals: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}
