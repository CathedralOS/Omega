//! A closed constructor projection retains its selected value, not storage.
//! Every sibling must be a literal in its declared carrier: erasing a call,
//! conversion, cleanup or partial computation would change evaluation. Field
//! and fixed-index steps compose over that one closed value; a record nested
//! inside an array does not need a runtime place just to read its scalar field. The
//! scalar producer and its source replay share only this typed identity join;
//! replay still checks the authored occurrence and the retained scalar value.

use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

/// Select a scalar field through ordinary, fully explicit closed record
/// constructors, optionally nested in constant arrays. An array root retains
/// its complete declaration type; no runtime place is created.
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
    // Qualification does not change the literal payload's carrier. Retain the
    // complete field type below so bounds do not discard its obligations.
    let mut carrier = reference;
    for _ in 0..program.type_reference_table.type_reference_count() {
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
            _ => break,
        }
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(carrier)
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

pub(super) fn closed_scalar_projection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
    let mut projections = Vec::new();
    let mut root = expression;
    let mut has_field = false;
    loop {
        if projections.len() >= program.expression_table.expression_count() {
            return None;
        }
        let receiver = match program.expression_table.expression(root) {
            ExpressionNode::Member(member)
                if member.case_variant.is_none() && member.member_symbol.is_valid() =>
            {
                has_field = true;
                member.receiver
            }
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => break,
        };
        projections.push(root);
        root = receiver;
    }
    if projections.is_empty() || !has_field {
        return None;
    }
    let root_type = super::declared_constant_array_type(program, root);
    let closed = match root_type {
        Some(reference) => closed_value(program, root, reference, &mut Vec::new()),
        None => closed_record(program, root, &mut Vec::new()),
    };
    if !closed {
        return None;
    }
    let mut selected = root;
    let mut selected_type = root_type.unwrap_or_default();
    for projection in projections.into_iter().rev() {
        match program.expression_table.expression(projection) {
            ExpressionNode::Member(member) => {
                let (literal, definition) = record_constructor(program, selected)?;
                let field = crate::value_custody::places::exact_data_member_field(
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
            ExpressionNode::Indexed(indexed) => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(selected_type)
                else {
                    return None;
                };
                let ExpressionNode::ArrayLiteral(elements) =
                    program.expression_table.expression(selected)
                else {
                    return None;
                };
                let ExpressionNode::Integer(ordinal) =
                    program.expression_table.expression(indexed.index)
                else {
                    return None;
                };
                if ordinal.landing().is_some_and(|landing| {
                    landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
                        || landing.landed_type == numerics::literals::LandedIntegerType::Addr
                }) || !builtin_index(program, projection, selected_type)
                {
                    return None;
                }
                let ordinal = usize::try_from(ordinal.value_u64()?).ok()?;
                if ordinal >= *length {
                    return None;
                }
                selected = *program
                    .expression_table
                    .expression_handles(*elements)
                    .get(ordinal)?;
                selected_type = *element_type;
            }
            _ => return None,
        }
    }
    program.primitive_type_reference(selected_type)?;
    // Scalar projection consumers currently carry Exact arithmetic. A policy
    // qualification must not enter that route until producer and source replay
    // can retain its selected arithmetic domain together with the payload.
    if program.arithmetic_domain_for_type_reference(selected_type)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    Some((selected, selected_type))
}

fn builtin_index(
    program: &TypedTrees,
    expression: ExpressionHandle,
    collection: TypeReferenceHandle,
) -> bool {
    let spelling = language_core::OperatorSpelling::Index;
    let operands = [Some(collection), None];
    // This value query has no invocation owner. An unresolved occurrence cannot
    // silently ignore a specialized trait meaning. Decline this query until
    // invocation-aware planning can retain that exact selected application;
    // this is not a claim that every such projection already has a native route.
    typed_trees::operator::resolve_indexed_spelling_for_operands(program, spelling, &operands)
        .is_empty()
        && program
            .machine_specializations
            .iter()
            .all(|specialization| {
                typed_trees::operator::selected_trait_operator_meanings(
                    program,
                    specialization.instance,
                    spelling,
                    &operands,
                )
                .is_empty()
            })
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            symbols::SymbolHandle::invalid(),
            expression,
            spelling,
            &operands,
        )
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
        closed_value(program, actual.value, field.type_reference, active)
    });
    active.pop();
    valid
}

fn closed_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    active: &mut Vec<ExpressionHandle>,
) -> bool {
    if active.contains(&expression) || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match (
        program.expression_table.expression(expression),
        program.type_reference_table.type_reference(reference),
    ) {
        (ExpressionNode::StructLiteral(literal), TypeReferenceNode::Named { symbol, .. }) => {
            *symbol == literal.type_symbol && closed_record(program, expression, active)
        }
        (
            ExpressionNode::ArrayLiteral(elements),
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            },
        ) => {
            let actuals = program.expression_table.expression_handles(*elements);
            if actuals.len() != *length || actuals.len() != elements.count() as usize {
                return false;
            }
            active.push(expression);
            let valid = actuals
                .iter()
                .all(|actual| closed_value(program, *actual, *element_type, active));
            active.pop();
            valid
        }
        // Scalar leaf checks retain exact landings. Every sibling is visited,
        // not only the field/element the consumer eventually selects.
        _ => super::closed_literal_array_elements(program, expression, reference).is_some(),
    }
}
