//! Exact constructor operands in authored order, independently of their consumer.
//!
//! This is a source classifier, not establishment evidence. Checking still owes
//! default domains and ownership; lowering must evaluate the returned operands
//! before constructing the selected nominal case. Only the existing closed,
//! fully authored scalar-payload sum route is described here.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub struct ScalarCaseConstructor {
    pub type_reference: TypeReferenceHandle,
    pub case: SymbolHandle,
    pub fields: Vec<(SymbolHandle, ExpressionHandle, PrimitiveType)>,
}

/// Classify before collecting scalar operand roots: a scalar/array match must
/// not leave structural dispatch operands behind when its leaves are rejected.
/// This checks destination identity, not field evaluation or match coverage.
pub fn is_fresh_scalar_case_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> bool {
    // This value route has no numeric field-establishment obligations yet.
    // Keep constrained constructors with the proof-bearing return producer;
    // selecting this plan must not displace a supported bounded-field return.
    if !crate::has_plain_owned_contents(program, reference) {
        return false;
    }
    let expected = program.normalized_type_identity(reference);
    let mut pending = vec![(expression, 0_usize)];
    while let Some((expression, depth)) = pending.pop() {
        if depth >= program.expression_table.expression_count() {
            return false;
        }
        if let Some(constructor) = scalar_case_constructor(program, expression) {
            if program.normalized_type_identity(constructor.type_reference) != expected {
                return false;
            }
            continue;
        }
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
        else {
            return false;
        };
        let arms = program.expression_table.match_arms(dispatch.arms);
        if arms.is_empty() {
            return false;
        }
        pending.extend(arms.iter().map(|arm| (arm.value, depth + 1)));
    }
    true
}

pub fn scalar_case_constructor(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ScalarCaseConstructor> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let (owner, selected, fields) = match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) => (
            program
                .data_definitions()
                .iter()
                .find(|owner| owner.symbol == literal.type_symbol)?,
            literal.case_symbol?,
            program.expression_table.struct_fields(literal.fields),
        ),
        ExpressionNode::Name(name) => (
            crate::exact_case_reference_owner(program, expression)?,
            name.symbol,
            &[][..],
        ),
        _ => return None,
    };
    let reference = program
        .type_reference_table
        .find_named_type_reference(owner.symbol)?;
    if !owner.type_parameters.is_empty()
        || !crate::has_plain_owned_contents_with_numeric_constraints(program, reference)
        || program
            .data_members(owner)
            .iter()
            .any(|member| matches!(member, DataMember::Field(_)))
        || program.symbols.get(selected).parent != owner.symbol
    {
        return None;
    }
    let case = program
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(case) if case.symbol == selected => Some(case),
            _ => None,
        })?;
    let declarations = program.data_payload_fields(case);
    if declarations.len() != fields.len() {
        return None;
    }
    let mut operands = Vec::with_capacity(fields.len());
    for field in fields {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.symbol == field.field_symbol)?;
        if !field.field_symbol.is_valid()
            || operands
                .iter()
                .any(|(symbol, _, _)| *symbol == field.field_symbol)
            || !program.expression_table.expression_is_valid(field.value)
        {
            return None;
        }
        operands.push((
            field.field_symbol,
            field.value,
            program.primitive_type_reference(declaration.type_reference)?,
        ));
    }
    Some(ScalarCaseConstructor {
        type_reference: reference,
        case: selected,
        fields: operands,
    })
}
