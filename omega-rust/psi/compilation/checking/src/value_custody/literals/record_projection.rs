//! A closed constructor projection retains its selected value, not storage.
//! Every sibling must be a literal in its declared carrier: erasing a call,
//! conversion, cleanup or partial computation would change evaluation. Field
//! and fixed-index steps compose over that one closed value; a record nested
//! inside an array does not need a runtime place just to read its scalar field. The
//! scalar producer and its source replay share only this typed identity join;
//! replay still checks the authored occurrence and the retained scalar value.
//! Case constructors use the same closed-value rule: common fields and the
//! selected case's payload must all be closed. A projection retains that exact
//! case; inactive payloads cannot supply a same-named scalar. Constructor
//! establishment separately owes the case's predicate and ownership checks.

use symbols::SymbolKind;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Select a scalar field through ordinary, fully explicit closed record
/// constructors, optionally nested in constant arrays. An array root retains
/// its complete declaration type; no runtime place is created.
pub fn closed_record_scalar_projection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, PrimitiveType)> {
    let (selected, selected_type) = closed_scalar_projection(program, expression)?;
    Some((selected, program.primitive_type_reference(selected_type)?))
}

/// Observe a scalar inside one already established closed constructor value.
/// Callers retain its exact declaration and program point. This projects only
/// closed contents: a captured call or mutable read must keep its SSA value,
/// never be re-executed by unfolding an initializer. Whole-owner custody and
/// cleanup remain independent obligations of the original establishment.
pub fn closed_scalar_place_projection(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
    reference: TypeReferenceHandle,
    path: &[facts::PlaceSegment],
) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
    if path.is_empty() || !closed_value(program, value, reference, &mut Vec::new()) {
        return None;
    }
    let mut selected = value;
    let mut selected_type = reference;
    for segment in path {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                let (literal, definition) = record_constructor(program, selected)?;
                let fields = constructor_fields(program, literal, definition)?;
                let (field, _) = fields
                    .into_iter()
                    .find(|(field, _)| field.symbol == *symbol)?;
                let actual = program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .find(|actual| actual.field_symbol == field.symbol)?;
                selected = actual.value;
                selected_type = field.type_reference;
            }
            facts::PlaceSegment::Case { variant } => {
                let (literal, _) = record_constructor(program, selected)?;
                if literal.case_symbol != Some(*variant) {
                    return None;
                }
            }
            facts::PlaceSegment::FixedIndex { index } => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(selected_type)
                else {
                    return None;
                };
                if index >= length {
                    return None;
                }
                let elements = program.expression_table.array_literal_node(selected)?;
                selected = *program
                    .expression_table
                    .expression_handles(*elements)
                    .get(*index)?;
                selected_type = *element_type;
            }
            _ => return None,
        }
    }
    program.primitive_type_reference(selected_type)?;
    Some((selected, selected_type))
}

/// A projected integer is already landed at its declared field, even when its
/// constructor spelled that leaf anonymously. Bounds retain that exact carrier.
pub(crate) fn closed_record_integer_projection(
    program: &dyn crate::CheckingInput,
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
    let symbol = &program
        .type_reference_table
        .type_reference(carrier)
        .as_named_symbol()?;
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
    program: &dyn crate::CheckingInput,
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
            ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
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
                if member.case_variant.is_some() && member.case_variant != literal.case_name {
                    return None;
                }
                let field = crate::value_custody::places::exact_data_member_field(
                    program,
                    definition,
                    member.member_symbol,
                    member.member.as_str(),
                    member.case_variant.as_ref().map(|variant| variant.as_str()),
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
                let elements = program.expression_table.array_literal_node(selected)?;
                let ordinal = program.expression_table.integer_node(indexed.index)?;
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    collection: TypeReferenceHandle,
) -> bool {
    let spelling = language_core::OperatorSpelling::Index;
    let operands = [Some(collection), None];
    // This value query has no invocation owner. An unresolved occurrence cannot
    // silently ignore a specialized trait meaning. Decline this query until
    // invocation-aware planning can retain that exact selected application;
    // this is not a claim that every such projection already has a native route.
    typed_trees::operator::resolve_indexed_spelling_for_operands(
        program,
        spelling,
        &operands,
        program.expression_table.source_span(expression),
    )
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(&TableStructLiteral, &DataDefinition)> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let literal = program.expression_table.struct_literal_node(expression)?;
    if !literal.type_symbol.is_valid()
        || program.symbols.get(literal.type_symbol).kind != SymbolKind::Data
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

/// The complete active constructor roster, retaining each field's owning
/// declaration. A case label and symbol must select the same declared variant;
/// neither spelling alone nor an inactive sibling's fields establish a value.
fn constructor_fields<'program>(
    program: &'program dyn crate::CheckingInput,
    literal: &TableStructLiteral,
    definition: &'program DataDefinition,
) -> Option<Vec<(&'program DataField, symbols::SymbolHandle)>> {
    let mut fields = Vec::new();
    let mut selected = false;
    let mut has_cases = false;
    let members = program.data_members(definition);
    if members.len() != definition.members.count() as usize {
        return None;
    }
    for member in members {
        match member {
            DataMember::Field(field) => fields.push((field, definition.symbol)),
            DataMember::Variant(variant) => {
                has_cases = true;
                if literal.case_symbol == Some(variant.symbol)
                    && literal.case_name.as_ref() == Some(&variant.name)
                {
                    if selected {
                        return None;
                    }
                    selected = true;
                    fields.extend(
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .map(|field| (field, variant.symbol)),
                    );
                }
            }
        }
    }
    if has_cases != selected
        || (!has_cases && (literal.case_name.is_some() || literal.case_symbol.is_some()))
    {
        return None;
    }
    Some(fields)
}

fn closed_record(
    program: &dyn crate::CheckingInput,
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
    let Some(fields) = constructor_fields(program, literal, definition) else {
        return false;
    };
    if actuals.len() != fields.len() || actuals.len() != literal.fields.count() as usize {
        return false;
    }
    active.push(expression);
    let valid = fields.iter().enumerate().all(|(ordinal, (field, owner))| {
        if !field.symbol.is_valid()
            || program.symbols.get(field.symbol).kind != SymbolKind::Field
            || program.symbols.get(field.symbol).parent != *owner
            || field.relevance.is_erased()
            || fields[..ordinal]
                .iter()
                .any(|(prior, _)| prior.symbol == field.symbol || prior.name == field.name)
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
    program: &dyn crate::CheckingInput,
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
