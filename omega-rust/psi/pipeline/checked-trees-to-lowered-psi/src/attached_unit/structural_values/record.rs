//! Scalar record fields use the shared evaluator before one atomic establishment.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_record(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    value: checked_trees::CheckedStructuralValueHandle,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    structural_types: &[StructuralTypeDeclaration],
    evaluation: &mut argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    source_count: usize,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let node = checked.facts.values.structural_values.nodes.get(value);
    let checked_trees::CheckedStructuralValueKind::Record {
        data_symbol,
        fields,
    } = node.kind
    else {
        return unsupported("scalar record producer missing");
    };
    let fields = checked
        .facts
        .values
        .structural_values
        .record_fields
        .span(fields)
        .ok_or(LoweringError::Unsupported("record field span is stale"))?;
    let record = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == data_symbol)
        .ok_or(LoweringError::Unsupported("record declaration missing"))?;
    let declarations = checked.data_members(record);
    let shape = structural_types
        .iter()
        .find(|item| item.id == structural_type)
        .ok_or(LoweringError::Unsupported("record structural type missing"))?;
    let StructuralTypeShape::Record {
        fields: shape_fields,
    } = &shape.shape
    else {
        return unsupported("record construction has a nonrecord shape");
    };
    let mut mapped = Vec::new();
    for field in fields {
        let declaration = declarations
            .iter()
            .find_map(|member| match member {
                checked_trees::data::DataMember::Field(declaration)
                    if declaration.symbol == field.field =>
                {
                    Some(declaration)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "record field declaration missing",
            ))?;
        let identity = declaration
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| declaration.name.as_str().to_owned());
        let position = shape_fields
            .iter()
            .position(|candidate| candidate.identity == identity)
            .ok_or(LoweringError::Unsupported("record field identity missing"))?;
        mapped.push((
            position,
            shape_fields[position].id,
            shape_fields[position].field_type.clone(),
        ));
    }
    let leaf_start = values.len();
    let mut field_positions = Vec::with_capacity(fields.len());
    for (ordinal, field) in fields.iter().enumerate() {
        let role = CheckedScalarExpressionRole::RecordField {
            expression: node.expression,
            field_ordinal: u32::try_from(ordinal)
                .map_err(|_| LoweringError::Unsupported("record field ordinal overflow"))?,
        };
        let value = evaluation.source_value(
            checked,
            machine,
            state,
            statement,
            role,
            &checked_trees::CheckedCallScalarArgument::Computation(field.value),
            source_count,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        if !matches!(
            mapped[ordinal].2,
            StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_)
        ) || mapped[ordinal].2.scalar_type() != Some(value.scalar_type)
        {
            return unsupported("record operand changed its field carrier");
        }
        field_positions.push(values.len());
        values.push(value);
    }
    let mut fields = mapped
        .iter()
        .enumerate()
        .map(|(ordinal, (position, field, _))| {
            (
                *position,
                terminal_psi::ScalarRecordFieldValue {
                    field: *field,
                    value: values[field_positions[ordinal]].id,
                },
            )
        })
        .collect::<Vec<_>>();
    fields.sort_by_key(|(position, _)| *position);
    let fields = fields.into_iter().map(|(_, field)| field).collect();
    values.truncate(leaf_start);
    let operation = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    let declaration = StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    };
    operations.push(Operation {
        id: operation,
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place,
            structural_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarRecord { fields },
    });
    Ok(declaration)
}
