//! Cursor store for a derived `Schema::encode`.
//!
//! Numbered data no longer grows a synthesized machine. The checked product
//! still owes the exclusive `written` update the old encoder performed: one
//! era varint, then each field's tag varint and, for a byte slice, its length
//! varint plus payload. The store lands on the one referent the selector
//! proved. An open selector or a field this walker cannot size fails the plan.

use checked_trees::CheckedUnitEffectOperationPlan;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::wire::WireMember;

pub(super) fn written_store(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    call: &typed_trees::statement::TableCall,
) -> Option<CheckedUnitEffectOperationPlan> {
    if call.target.as_str() != "encode" {
        return None;
    }
    let arguments = program.statement_table.expression_handles(call.arguments);
    let [value, _out, written] = arguments else {
        return None;
    };
    let length = framed_length(program, state, call, *value)?;
    let (root, segments) = validation::compiler_operations::single_exclusive_reference_storage(
        program, machine, *written,
    )?;
    let path = store_path(program, &segments)?;
    let literal = facts::u64_integer_literal(length, PrimitiveType::U64)?;
    Some(CheckedUnitEffectOperationPlan::new_store(
        statement_index,
        checked_trees::CheckedStoreRoot::Local { symbol: root },
        path,
        checked_trees::CheckedStoreLeaf::Primitive {
            primitive_type: PrimitiveType::U64,
            preserves_byte_memberships: false,
            receiving_range: None,
        },
        checked_trees::CheckedStoreValue::assigned(checked_trees::CheckedScalarStoreValue::Pure(
            literal,
        )),
    ))
}

fn store_path(
    program: &dyn ::validation::CheckingInput,
    segments: &[facts::PlaceSegment],
) -> Option<Vec<checked_trees::CheckedUnitStructuralPathSegment>> {
    let mut path = Vec::with_capacity(segments.len());
    for segment in segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            crate::execution::terminal_unit::types::terminal_field_identity(program, *symbol)?,
        ));
    }
    Some(path)
}

fn framed_length(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
    value: ExpressionHandle,
) -> Option<u64> {
    let leaf = program
        .statement_table
        .name_path_members(call.receiver)
        .last()?;
    let schema = program
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == leaf.as_str())?;
    let literal = struct_literal(program, state, value)?;
    // Era 0, then each numbered field. Reserved numbers contribute no bytes.
    let mut total = varint_len(0)?;
    for member in program.wire_members(schema.members) {
        match member {
            WireMember::Reserved(_) => {}
            WireMember::Field(field) if is_byte_slice(program, field.type_reference) => {
                let payload = field_slice_len(program, literal, field.name.as_str())?;
                total = total
                    .checked_add(varint_len(field.number)?)?
                    .checked_add(varint_len(payload)?)?
                    .checked_add(payload)?;
            }
            WireMember::Field(_) | WireMember::Version(_) => return None,
        }
    }
    Some(total)
}

fn struct_literal<'a>(
    program: &'a dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    value: ExpressionHandle,
) -> Option<&'a typed_trees::expression::TableStructLiteral> {
    let target = match program.expression_table.expression(value) {
        ExpressionNode::Borrow(borrow) => borrow.target,
        _ => value,
    };
    let ExpressionNode::Name(name) = program.expression_table.expression(target) else {
        return None;
    };
    let symbol = name.symbol;
    let initializer = program
        .statement_table
        .iter_statements(state.statement_nodes)
        .find_map(|(_, statement)| match statement {
            typed_trees::statement::StatementNode::LocalData(local) if local.symbol == symbol => {
                Some(local.initial_value)
            }
            _ => None,
        })?;
    match program.expression_table.expression(initializer) {
        ExpressionNode::StructLiteral(literal) => Some(literal),
        _ => None,
    }
}

fn field_slice_len(
    program: &dyn ::validation::CheckingInput,
    literal: &typed_trees::expression::TableStructLiteral,
    field: &str,
) -> Option<u64> {
    let supplied = program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .find(|supplied| supplied.name.as_str() == field)?;
    slice_len(program, supplied.value)
}

fn slice_len(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<u64> {
    let expression = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => borrow.target,
        _ => expression,
    };
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    if range.end_inclusive {
        return None;
    }
    constant_u64(program, range.end)?.checked_sub(constant_u64(program, range.start)?)
}

fn constant_u64(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<u64> {
    let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) else {
        return None;
    };
    u64::try_from(literal.value_i64()?).ok()
}

fn is_byte_slice(
    program: &dyn ::validation::CheckingInput,
    mut type_reference: TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Slice { element_type } => {
                return program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8);
            }
            _ => return false,
        }
    }
}

fn varint_len(mut value: u64) -> Option<u64> {
    let mut length = 1u64;
    while value >= 128 {
        value >>= 7;
        length = length.checked_add(1)?;
        if length > 10 {
            return None;
        }
    }
    Some(length)
}
