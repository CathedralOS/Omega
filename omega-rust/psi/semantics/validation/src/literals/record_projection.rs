//! A closed constructor projection retains its selected value, not storage.
//! Every sibling must be a literal in its declared carrier: erasing a call,
//! conversion, cleanup or partial computation would change evaluation. The
//! scalar producer and its source replay share only this typed identity join;
//! replay still checks the authored occurrence and the retained scalar value.

use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Select a scalar field through ordinary, fully explicit closed record
/// constructors. No constant-declaration identity or runtime place is needed.
pub fn closed_record_scalar_projection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, PrimitiveType)> {
    let (selected, selected_type) = closed_scalar_projection(program, expression)?;
    Some((selected, program.primitive_type_reference(selected_type)?))
}

/// A projected integer is already landed at its declared field, even when its
/// constructor spelled that leaf anonymously. Bounds retain that exact carrier.
pub(crate) fn closed_record_integer_projection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<typed_trees::closed_numeric::ClosedIntegerValue> {
    let (selected, reference) = closed_scalar_projection(program, expression)?;
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    program.symbols.builtin_type_atom(*symbol)?;
    let primitive = program.primitive_type_reference(reference)?;
    let value = program.closed_integer_value_in(selected, symbols::SymbolHandle::invalid())?;
    if value.primitive.is_some_and(|landed| landed != primitive) {
        return None;
    }
    typed_trees::closed_numeric::land_integer(&value.value, primitive)?;
    Some(typed_trees::closed_numeric::ClosedIntegerValue {
        value: value.value,
        primitive: Some(primitive),
        type_reference: Some(reference),
    })
}

fn closed_scalar_projection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
    let mut members = Vec::new();
    let mut root = expression;
    while let ExpressionNode::Member(member) = program.expression_table.expression(root) {
        if members.len() >= program.expression_table.expression_count()
            || member.case_variant.is_some()
            || !member.member_symbol.is_valid()
        {
            return None;
        }
        members.push(member);
        root = member.receiver;
    }
    if members.is_empty() || !closed_record(program, root, &mut Vec::new()) {
        return None;
    }
    let mut selected = root;
    let mut selected_type = TypeReferenceHandle::invalid();
    for member in members.into_iter().rev() {
        let (literal, definition) = record_constructor(program, selected)?;
        let field = crate::places::exact_data_member_field(
            program,
            definition,
            member.member_symbol,
            member.member.as_str(),
            None,
        )?;
        let actual = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|actual| actual.field_symbol == field.symbol)?;
        selected = actual.value;
        selected_type = field.type_reference;
    }
    program.primitive_type_reference(selected_type)?;
    Some((selected, selected_type))
}

fn record_constructor(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(&TableStructLiteral, &DataDefinition)> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return None;
    };
    if !literal.type_symbol.is_valid()
        || program.symbols.get(literal.type_symbol).kind != SymbolKind::Data
        || literal.case_name.is_some()
        || literal.case_symbol.is_some()
    {
        return None;
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == literal.type_symbol);
    let definition = definitions.next()?;
    if definitions.next().is_some()
        || !program.data_type_parameters(definition).is_empty()
        || definition.quotient.is_some()
        || definition.properties.multiplicity == language_semantics::Multiplicity::Linear
        || program.machines().iter().any(|machine| {
            machine.attached_data_symbol == definition.symbol
                && machine.name.as_str().ends_with("::drop")
        })
    {
        return None;
    }
    Some((literal, definition))
}

fn closed_record(
    program: &TypedTrees,
    expression: ExpressionHandle,
    active: &mut Vec<ExpressionHandle>,
) -> bool {
    if active.contains(&expression) {
        return false;
    }
    let Some((literal, definition)) = record_constructor(program, expression) else {
        return false;
    };
    let actuals = program.expression_table.struct_fields(literal.fields);
    let fields = program.data_members(definition);
    if actuals.len() != fields.len()
        || actuals.len() != literal.fields.count() as usize
        || fields.len() != definition.members.count() as usize
    {
        return false;
    }
    active.push(expression);
    let valid = fields.iter().enumerate().all(|(ordinal, member)| {
        let DataMember::Field(field) = member else {
            return false;
        };
        if !field.symbol.is_valid()
            || program.symbols.get(field.symbol).kind != SymbolKind::Field
            || program.symbols.get(field.symbol).parent != definition.symbol
            || field.relevance.is_erased()
            || fields[..ordinal].iter().any(|prior| {
                matches!(prior, DataMember::Field(prior)
                    if prior.symbol == field.symbol || prior.name == field.name)
            })
        {
            return false;
        }
        let mut matching = actuals
            .iter()
            .filter(|actual| actual.field_symbol == field.symbol && actual.name == field.name);
        let Some(actual) = matching.next() else {
            return false;
        };
        if matching.next().is_some() || !program.expression_table.expression_is_valid(actual.value)
        {
            return false;
        }
        if let ExpressionNode::StructLiteral(nested) =
            program.expression_table.expression(actual.value)
        {
            matches!(program.type_reference_table.type_reference(field.type_reference),
                TypeReferenceNode::Named { symbol, .. } if *symbol == nested.type_symbol)
                && closed_record(program, actual.value, active)
        } else {
            // Reuse the scalar/array literal carrier check, including exact
            // integer landing and every unselected array element.
            super::closed_literal_array_elements(program, actual.value, field.type_reference)
                .is_some()
        }
    });
    active.pop();
    valid
}
