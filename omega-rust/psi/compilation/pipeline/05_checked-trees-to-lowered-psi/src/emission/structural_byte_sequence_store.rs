//! Source correspondence and ordered emission for bounded byte-field replacement.
use super::{
    CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError, Operation, OperationKind,
    OperationResult, PrimitiveType, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    ValueDeclaration, allocate_dense, obligation_id, place_id, structural_type_id,
    terminal_scalar_type, unsupported, value_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;

use checked_trees::{
    CheckedByteSequenceStoreDestination as Destination, CheckedByteSequenceValue as ByteValue,
    CheckedByteSequenceValuePlan,
};
use terminal_psi::control_flow::ByteSequenceOperand;

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
    if !declaration.shape.is_borrowed_byte_view() {
        return unsupported("generated literal-view identity has a different carrier");
    }
    Ok(declaration.id)
}

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    store: &checked_trees::CheckedByteSequenceStorePlan,
) -> Result<(), LoweringError> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    if owner.symbol != machine {
        return unsupported("byte-field store has a different authored machine");
    }
    // Alias replay needs the use occurrence to rejoin the declaration, active
    // loan and retirement evidence. Discarding this retained coordinate would
    // restrict replacement to directly spelled parameter fields even though
    // checking already normalized a borrowed destination to the same storage.
    let statement_index = store.statement_index as usize;
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement_index),
        assignment.target,
    )?;
    // Neither a whole byte owner nor a byte field has a local-rooted lowering.
    let Some(position) = store.root.parameter_position() else {
        return unsupported("byte-field store has no parameter root");
    };
    let parameter = checked
        .state_parameters(state)
        .get(position as usize)
        .ok_or(LoweringError::Unsupported(
            "byte-field store has no authored parameter",
        ))?;
    let path = match &store.destination {
        Destination::Whole => Vec::new(),
        Destination::Field {
            carrier_path,
            field_identity,
        } => {
            let mut path = carrier_path.clone();
            path.push(CheckedUnitStructuralPathSegment::Field(
                field_identity.clone(),
            ));
            path
        }
    };
    if source.root != parameter.symbol || source.path != path {
        return unsupported("byte-field store destination differs from its authored place");
    }
    validate_value(
        checked,
        machine,
        state_symbol,
        statement_index,
        assignment.value,
        &store.value,
        store.value.root,
        0,
    )?;
    Ok(())
}

fn validate_value(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    expression: checked_trees::expression::ExpressionHandle,
    plan: &CheckedByteSequenceValuePlan,
    node: arena::Handle<ByteValue>,
    depth: usize,
) -> Result<(), LoweringError> {
    use checked_trees::expression::{BinaryOperator, ExpressionNode};
    if depth > 128 || !plan.nodes.iter().any(|(identity, _)| identity == node) {
        return unsupported("byte value plan has an invalid child");
    }
    match (
        plan.nodes.get(node),
        checked.expression_table.expression(expression),
    ) {
        (ByteValue::Literal(expected), ExpressionNode::String(actual))
            if actual.as_ref() == expected.as_slice() =>
        {
            Ok(())
        }
        (ByteValue::Concatenation { left, right, .. }, ExpressionNode::Binary(binary))
            if binary.operator == BinaryOperator::Add
                && checked
                    .facts
                    .operators
                    .expression_use(expression)
                    .is_none_or(|selected| {
                        selected.status
                            == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                    }) =>
        {
            validate_value(
                checked,
                machine,
                state_symbol,
                statement_index,
                binary.left,
                plan,
                *left,
                depth + 1,
            )?;
            validate_value(
                checked,
                machine,
                state_symbol,
                statement_index,
                binary.right,
                plan,
                *right,
                depth + 1,
            )
        }
        (
            ByteValue::Field {
                parameter_position,
                carrier_path,
                field_identity,
                ..
            },
            _,
        ) => {
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                checked,
                state_symbol,
            )?;
            let source =
                crate::emission::call_source_custody::projected_receivers::store_destination(
                    checked,
                    machine,
                    state_symbol,
                    Some(statement_index),
                    expression,
                )?;
            let parameter = checked
                .state_parameters(state)
                .get(*parameter_position as usize)
                .ok_or(LoweringError::Unsupported("byte value parameter is absent"))?;
            let mut path = carrier_path.clone();
            path.push(CheckedUnitStructuralPathSegment::Field(
                field_identity.clone(),
            ));
            if source.root != parameter.symbol || source.path != path {
                return unsupported("byte value field differs from its authored place");
            }
            Ok(())
        }
        _ => unsupported("byte value differs from its authored expression"),
    }
}

/// Generated owned carriers are shared by capacity, independently of live extent.
pub(crate) fn prepare_types(
    store: &checked_trees::CheckedByteSequenceStorePlan,
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<(), LoweringError> {
    literal_view_type(types)?;
    for (identity, node) in store.value.nodes.iter() {
        let capacity = match node {
            ByteValue::Concatenation { capacity, .. } => Some(capacity),
            ByteValue::Field { capacity, .. } if identity == store.value.root => Some(capacity),
            _ => None,
        };
        if let Some(capacity) = capacity {
            let identity = format!("compiler(byte-sequence-owned:{capacity})");
            let shape = StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned {
                    capacity: *capacity,
                },
            );
            if let Some(existing) = types
                .iter()
                .find(|candidate| candidate.identity == identity)
            {
                if existing.shape != shape {
                    return unsupported("generated owned byte carrier has a different shape");
                }
            } else {
                let ordinal = types
                    .iter()
                    .map(|candidate| candidate.id.get())
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "owned byte type identity overflow",
                    ))?;
                types.push(StructuralTypeDeclaration {
                    id: structural_type_id(ordinal),
                    identity,
                    shape,
                });
            }
        }
    }
    Ok(())
}

/// A generated immutable carrier, not a nominal authored type or parameter.
pub(crate) fn literal_view_type(
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<StructuralTypeId, LoweringError> {
    let shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        });
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
    store: &checked_trees::CheckedByteSequenceStorePlan,
    parameters: &[(u32, StructuralParameterDeclaration)],
    structural_types: &[StructuralTypeDeclaration],
    literal_places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    // Checked stores retain authored source positions. The evaluation binding
    // carries their exact lowered declarations after scalar/structural partition.
    let Some(position) = store.root.parameter_position() else {
        return unsupported("byte-field store destination has no parameter root");
    };
    let (_, parameter) = parameters
        .iter()
        .find(|(authored, _)| *authored == position)
        .ok_or(LoweringError::Unsupported(
            "byte-field store destination is absent",
        ))?;
    let field_destination = match &store.destination {
        Destination::Whole => {
            if !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || !structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
                })
            {
                return unsupported("whole byte store requires a borrowed bounded owner");
            }
            None
        }
        Destination::Field {
            carrier_path,
            field_identity,
        } => {
            let (path, field) =
                crate::emission::structural_scalar_store::lower_structural_field_place(
                    store.statement_index,
                    store.statement_index,
                    parameter.position,
                    carrier_path,
                    field_identity,
                    parameter,
                    structural_types,
                    crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
                )?;
            if !matches!(
                field.field_type.byte_sequence_carrier(),
                Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
            ) {
                return unsupported("byte-field store destination is not bounded owned bytes");
            }
            Some((path, field.id))
        }
    };
    let mut emitter = ValueEmitter {
        plan: &store.value,
        parameters,
        structural_types,
        places: literal_places,
        next_place,
        next_value,
        next_obligation,
        operations,
    };
    let (operand, capacity) = emitter.emit(store.value.root, 0)?;
    let (source, length) = match operand {
        ByteSequenceOperand::Whole { source, length } => (source, length),
        field @ ByteSequenceOperand::Field { .. } => {
            // A mutable field cannot back an immutable store source. The
            // existing two-span materialization with an empty second span
            // snapshots its live bytes into fresh owned backing before any
            // destination write, including a write through an alias.
            let (empty, _) = emitter.literal(&[])?;
            emitter.owned(field, empty, capacity)?
        }
    };
    let obligation = obligation_id(allocate_dense(next_obligation)?);
    match field_destination {
        Some((path, field)) => {
            operations.invalidate_field_lengths_under(parameter.place, &path);
            Ok(OperationKind::StructuralByteSequenceFieldStore {
                destination: parameter.place,
                path,
                field,
                source,
                length,
                obligation,
            })
        }
        None => {
            operations.invalidate_field_lengths_under(parameter.place, &[]);
            Ok(OperationKind::ByteSequenceStore {
                destination: parameter.place,
                source,
                length,
                obligation,
            })
        }
    }
}

struct ValueEmitter<'input> {
    plan: &'input CheckedByteSequenceValuePlan,
    parameters: &'input [(u32, StructuralParameterDeclaration)],
    structural_types: &'input [StructuralTypeDeclaration],
    places: &'input mut Vec<StructuralPlaceDeclaration>,
    next_place: &'input mut u64,
    next_value: &'input mut u64,
    next_obligation: &'input mut u64,
    operations: &'input mut OperationBuffer,
}

impl ValueEmitter<'_> {
    fn length(
        &mut self,
        kind: OperationKind,
    ) -> Result<semantic_vocabulary::ValueId, LoweringError> {
        let length = value_id(allocate_dense(self.next_value)?);
        let id = self.operations.allocate();
        self.operations.push(Operation {
            id,
            kind,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(ValueDeclaration {
                id: length,
                scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                qualifications: Default::default(),
            }),
        });
        Ok(length)
    }

    fn literal(&mut self, bytes: &[u8]) -> Result<(ByteSequenceOperand, u64), LoweringError> {
        let structural_type = existing_literal_view_type(self.structural_types)?;
        let source = place_id(allocate_dense(self.next_place)?);
        let declaration_ordinal = super::next_byte_sequence_literal_ordinal(self.places)?;
        self.places.push(StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            },
        });
        let id = self.operations.allocate();
        self.operations.push(Operation {
            id,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: source,
                bytes: bytes.to_vec(),
                qualifications: Vec::new(),
            },
        });
        let capacity = u64::try_from(bytes.len())
            .map_err(|_| LoweringError::Unsupported("byte literal length overflow"))?;
        let length = self.length(OperationKind::ByteSequenceLength { source })?;
        Ok((ByteSequenceOperand::Whole { source, length }, capacity))
    }

    fn owned(
        &mut self,
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
        capacity: u64,
    ) -> Result<(semantic_vocabulary::PlaceId, semantic_vocabulary::ValueId), LoweringError> {
        let identity = format!("compiler(byte-sequence-owned:{capacity})");
        let structural_type = self
            .structural_types
            .iter()
            .find(|candidate| {
                candidate.identity == identity
                    && candidate.shape
                        == StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                        )
            })
            .ok_or(LoweringError::Unsupported(
                "owned byte carrier absent from shared catalog",
            ))?
            .id;
        let source = place_id(allocate_dense(self.next_place)?);
        let id = self.operations.allocate();
        self.places.push(StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type,
            },
        });
        self.operations.push(Operation {
            id,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: source,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishByteSequenceConcatenation {
                left,
                right,
                capacity_obligation: obligation_id(allocate_dense(self.next_obligation)?),
            },
        });
        let length = self.length(OperationKind::ByteSequenceLength { source })?;
        Ok((source, length))
    }

    fn emit(
        &mut self,
        node: arena::Handle<ByteValue>,
        depth: usize,
    ) -> Result<(ByteSequenceOperand, u64), LoweringError> {
        if depth > 128 || !self.plan.nodes.iter().any(|(identity, _)| identity == node) {
            return unsupported("byte value plan has an invalid child");
        }
        match self.plan.nodes.get(node) {
            ByteValue::Literal(bytes) => self.literal(bytes),
            ByteValue::Field {
                parameter_position,
                carrier_path,
                field_identity,
                capacity,
            } => {
                let (_, parameter) = self
                    .parameters
                    .iter()
                    .find(|(authored, _)| *authored == *parameter_position)
                    .ok_or(LoweringError::Unsupported("byte value parameter is absent"))?;
                if !matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                ) || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return unsupported("byte value parameter has no ordinary read authority");
                }
                let (path, field) =
                    crate::emission::structural_scalar_store::lower_structural_field_path(
                        parameter.structural_type,
                        carrier_path,
                        field_identity,
                        self.structural_types,
                    )?;
                if field.field_type.byte_sequence_carrier()
                    != Some(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity: *capacity,
                    })
                {
                    return unsupported("byte value field has a different carrier");
                }
                let source = terminal_psi::StructuralArgument {
                    place: parameter.place,
                    path: path.clone(),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                };
                let field = field.id;
                // Observe at this operand's evaluation point. Later effects cannot
                // retroactively supply this source's extent or content custody.
                let length = self.length(OperationKind::StructuralByteSequenceFieldLength {
                    source: source.place,
                    path,
                    field,
                })?;
                Ok((
                    ByteSequenceOperand::Field {
                        source,
                        field,
                        length,
                    },
                    *capacity,
                ))
            }
            ByteValue::Concatenation {
                left,
                right,
                capacity,
            } => {
                let (left, left_capacity) = self.emit(*left, depth + 1)?;
                let (right, right_capacity) = self.emit(*right, depth + 1)?;
                if left_capacity.checked_add(right_capacity) != Some(*capacity) {
                    return unsupported(
                        "byte concatenation capacity differs from operand carriers",
                    );
                }
                let (source, length) = self.owned(left, right, *capacity)?;
                Ok((ByteSequenceOperand::Whole { source, length }, *capacity))
            }
        }
    }
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
