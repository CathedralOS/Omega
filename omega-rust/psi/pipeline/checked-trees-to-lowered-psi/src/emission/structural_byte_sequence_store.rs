//! Source correspondence and ordered emission for bounded byte-field replacement.
use super::{
    CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError, Operation, OperationKind,
    OperationResult, PrimitiveType, StructuralFieldType, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, ValueDeclaration, allocate_dense, obligation_id, place_id,
    structural_type_id, terminal_scalar_type, unsupported, value_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;

const LITERAL_VIEW_IDENTITY: &str = "compiler(byte-sequence-literal-view)";

/// Shared callees must use the closure's existing carrier, not allocate one.
pub(crate) fn existing_literal_view_type(
    types: &[StructuralTypeDeclaration],
) -> Result<StructuralTypeId, LoweringError> {
    let declaration = types
        .iter()
        .find(|declaration| declaration.identity == LITERAL_VIEW_IDENTITY)
        .ok_or(LoweringError::Unsupported(
            "composed callable produced a type absent from the shared catalog",
        ))?;
    if declaration.shape
        != StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
    {
        return unsupported("generated literal-view identity has a different carrier");
    }
    Ok(declaration.id)
}

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    store: &checked_trees::CheckedStructuralByteSequenceFieldStorePlan,
) -> Result<(), LoweringError> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    if owner.symbol != machine {
        return unsupported("byte-field store has a different authored machine");
    }
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        None,
        assignment.target,
    )?;
    let parameter = checked
        .state_parameters(state)
        .get(store.destination_parameter_position as usize)
        .ok_or(LoweringError::Unsupported(
            "byte-field store has no authored parameter",
        ))?;
    let mut path = store.carrier_path.clone();
    path.push(CheckedUnitStructuralPathSegment::Field(
        store.field_identity.clone(),
    ));
    if source.root != parameter.symbol || source.path != path {
        return unsupported("byte-field store destination differs from its authored place");
    }
    if !matches!(checked.expression_table.expression(assignment.value),
        checked_trees::expression::ExpressionNode::String(bytes) if bytes.as_ref() == store.bytes.as_slice())
    {
        return unsupported("byte-field store literal differs from its authored value");
    }
    Ok(())
}

/// A generated immutable carrier, not a nominal authored type or parameter.
pub(crate) fn literal_view_type(
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<StructuralTypeId, LoweringError> {
    let shape = StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView);
    if let Some(declaration) = types
        .iter()
        .find(|declaration| declaration.identity == LITERAL_VIEW_IDENTITY)
    {
        return if declaration.shape == shape {
            Ok(declaration.id)
        } else {
            unsupported("generated literal-view identity has a different carrier")
        };
    }
    let identity = types
        .iter()
        .map(|declaration| declaration.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "literal-view type identity overflow",
        ))?;
    let id = structural_type_id(identity);
    types.push(StructuralTypeDeclaration {
        id,
        identity: LITERAL_VIEW_IDENTITY.into(),
        shape,
    });
    Ok(id)
}

pub(crate) fn emit(
    store: &checked_trees::CheckedStructuralByteSequenceFieldStorePlan,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    literal_places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == store.destination_parameter_position)
        .ok_or(LoweringError::Unsupported(
            "byte-field store destination is absent",
        ))?;
    let (path, field) = crate::emission::structural_scalar_store::lower_structural_field_place(
        store.statement_index,
        store.statement_index,
        store.destination_parameter_position,
        &store.carrier_path,
        &store.field_identity,
        parameter,
        structural_types,
        crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
    )?;
    let StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    }) = field.field_type
    else {
        return unsupported("byte-field store destination is not bounded owned bytes");
    };
    if u64::try_from(store.bytes.len())
        .ok()
        .is_none_or(|length| length > capacity)
    {
        return unsupported("byte-field store literal exceeds capacity");
    }
    let field = field.id;
    let structural_type = existing_literal_view_type(structural_types)?;
    let source = place_id(allocate_dense(next_place)?);
    let declaration_ordinal = u32::try_from(literal_places.len())
        .map_err(|_| LoweringError::Unsupported("literal declaration ordinal exceeds u32"))?;
    literal_places.push(StructuralPlaceDeclaration {
        id: source,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        },
    });
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination: source,
            bytes: store.bytes.clone(),
            // A stored literal is not a call argument, so no check-admitted
            // domains replay onto it; qualification evidence belongs to
            // literal argument occurrences only.
            qualifications: Vec::new(),
        },
    });
    let length = value_id(allocate_dense(next_value)?);
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: length,
            scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
        }),
        kind: OperationKind::ByteSequenceLength { source },
    });
    Ok(OperationKind::StructuralByteSequenceFieldStore {
        destination: parameter.place,
        path,
        field,
        source,
        length,
        obligation: obligation_id(allocate_dense(next_obligation)?),
    })
}

#[cfg(test)]
mod tests {
    use super::{StructuralTypeShape, existing_literal_view_type, literal_view_type};

    #[test]
    fn shared_literal_carrier_requires_exact_prepared_declaration() {
        let mut types = Vec::new();
        assert!(existing_literal_view_type(&types).is_err());
        let identity = literal_view_type(&mut types).expect("prepare literal carrier");
        let original = types.clone();
        assert_eq!(existing_literal_view_type(&types).unwrap(), identity);
        assert_eq!(literal_view_type(&mut types).unwrap(), identity);
        assert_eq!(types, original);

        types[0].shape =
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: 8,
            });
        assert!(existing_literal_view_type(&types).is_err());
        assert!(literal_view_type(&mut types).is_err());
    }
}
