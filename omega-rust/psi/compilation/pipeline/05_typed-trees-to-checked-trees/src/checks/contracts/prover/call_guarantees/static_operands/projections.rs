//! Project immutable aggregate actuals only after the parent joins the exact
//! invocation tuple. Constructor children keep their supplying owner/roster;
//! encoded values replay their entire schema before selecting any field.
//! This never reads runtime storage or executes a const initializer.

use super::{Invocation, arithmetic};
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use validation::{ImmutableConstRecordProjection, ScopedArithmeticValue};

pub(super) fn atom(
    program: &dyn validation::CheckingInput,
    projection: &ImmutableConstRecordProjection,
    primitive: PrimitiveType,
) -> Option<ScopedArithmeticValue> {
    (program.primitive_type_reference(projection.carrier) == Some(primitive)).then(|| {
        ScopedArithmeticValue::Atom {
            identity: format!(
                "\0static-parameter:{:?}:fields:{:?}",
                projection.root, projection.fields
            ),
            unsigned: matches!(
                primitive,
                typed_trees::unsigned_integer_primitive_pattern!()
            ),
        }
    })
}

pub(super) fn value(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    invocation: &Invocation<'_>,
    reference: TypeReferenceHandle,
    projection: &ImmutableConstRecordProjection,
    primitive: PrimitiveType,
) -> Option<ScopedArithmeticValue> {
    let parameter = program.data_type_parameter_by_symbol(projection.root)?;
    let TypeParameterKind::Const {
        type_reference: carrier,
    } = parameter.kind
    else {
        return None;
    };
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::ConstExpression(expression) => expression_value(
            program,
            facts,
            invocation,
            *expression,
            carrier,
            &projection.fields,
            primitive,
            &mut Vec::new(),
        ),
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(parameter) = program.data_type_parameter_by_symbol(*symbol) {
                let TypeParameterKind::Const {
                    type_reference: actual,
                } = parameter.kind
                else {
                    return None;
                };
                if program.normalized_type_identity(actual)
                    != program.normalized_type_identity(carrier)
                {
                    return None;
                }
                return atom(
                    program,
                    &ImmutableConstRecordProjection {
                        root: *symbol,
                        fields: projection.fields.clone(),
                        carrier: projection.carrier,
                    },
                    primitive,
                );
            }
            let value = if symbol.is_valid() {
                let declaration = program.const_declaration_by_symbol(*symbol)?;
                if program.normalized_type_identity(declaration.declared_type)
                    != program.normalized_type_identity(carrier)
                {
                    return None;
                }
                CanonicalConstValue::new(
                    program.normalized_type_identity_text(carrier),
                    declaration.evaluated_value.as_ref()?.encoding()?,
                    "",
                )
            } else {
                CanonicalConstValue::from_atom(name.as_str())?
            };
            decoded_value(program, carrier, &projection.fields, primitive, value)
        }
        _ => None,
    }
}

fn schema(
    program: &dyn validation::CheckingInput,
    mut carrier: TypeReferenceHandle,
    fields: &[SymbolHandle],
) -> Option<(Vec<String>, TypeReferenceHandle)> {
    let mut names = Vec::with_capacity(fields.len());
    for field in fields {
        let reference = program.unwrapped_type_reference(carrier)?;
        let symbol = program
            .type_reference_table
            .type_reference(reference)
            .as_named_symbol()?;
        let definition = program.data_definition_by_symbol(symbol)?;
        let mut selected = program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
            .filter(|candidate| candidate.symbol == *field);
        let selected = selected.next().filter(|_| selected.next().is_none())?;
        names.push(selected.name.as_str().to_owned());
        carrier = selected.type_reference;
    }
    Some((names, carrier))
}

fn decoded_value(
    program: &dyn validation::CheckingInput,
    carrier: TypeReferenceHandle,
    fields: &[SymbolHandle],
    primitive: PrimitiveType,
    value: CanonicalConstValue,
) -> Option<ScopedArithmeticValue> {
    if program.type_identity_by_text(&value.type_name) != program.normalized_type_identity(carrier)
    {
        return None;
    }
    validation::validate_exact_const_value_encoding(program, carrier, &value.encoding).ok()?;
    let (names, terminal) = schema(program, carrier, fields)?;
    if program.primitive_type_reference(terminal) != Some(primitive) {
        return None;
    }
    let mut decoded = value.decode_encoding()?;
    for name in names {
        let DecodedCanonicalConstValue::Record { fields, .. } = decoded else {
            return None;
        };
        decoded = fields.into_iter().find(|(field, _)| *field == name)?.1;
    }
    let DecodedCanonicalConstValue::Integer { type_name, value } = decoded else {
        return None;
    };
    (type_name == primitive.name())
        .then(|| ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i128(value)))
}

fn expression_value(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
    carrier: TypeReferenceHandle,
    fields: &[SymbolHandle],
    primitive: PrimitiveType,
    active: &mut Vec<ExpressionHandle>,
) -> Option<ScopedArithmeticValue> {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return None;
    }
    if fields.is_empty() {
        let handle = match invocation.site {
            validation::CallSite::Expression { call, .. } => call.authored_machine_application,
            validation::CallSite::Statement(call) => call.authored_machine_application,
            validation::CallSite::TransitionNamed { .. } => return None,
        };
        let application = program.expression_table.static_machine_application(handle);
        let machine = program.machine_by_symbol(application.lexical_owner)?;
        let state = program.machine_states(machine).first()?;
        return arithmetic::scalar_term(
            program,
            facts,
            &super::Callable::Machine { machine, state },
            expression,
            |_, _| None,
        );
    }
    active.push(expression);
    let result = match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal)
            if literal.case_symbol.is_none() && literal.case_name.is_none() =>
        {
            let reference = program.unwrapped_type_reference(carrier)?;
            let symbol = program
                .type_reference_table
                .type_reference(reference)
                .as_named_symbol()?;
            if symbol != literal.type_symbol {
                return None;
            }
            let definition = program.data_definition_by_symbol(symbol)?;
            let declared = program
                .data_members(definition)
                .iter()
                .map(|member| member.as_field())
                .collect::<Option<Vec<_>>>()?;
            let supplied = program.expression_table.struct_fields(literal.fields);
            if declared.len() != supplied.len()
                || declared.iter().any(|field| {
                    supplied
                        .iter()
                        .filter(|value| value.field_symbol == field.symbol)
                        .count()
                        != 1
                })
            {
                return None;
            }
            let (_, child_carrier) = schema(program, carrier, &fields[..1])?;
            let selected = supplied
                .iter()
                .find(|value| value.field_symbol == fields[0])?;
            expression_value(
                program,
                facts,
                invocation,
                selected.value,
                child_carrier,
                &fields[1..],
                primitive,
                active,
            )
        }
        ExpressionNode::Name(path) => {
            if let Some(parameter) = program.data_type_parameter_by_symbol(path.symbol) {
                let TypeParameterKind::Const {
                    type_reference: actual,
                } = parameter.kind
                else {
                    return None;
                };
                if program.normalized_type_identity(actual)
                    != program.normalized_type_identity(carrier)
                {
                    return None;
                }
                let (_, terminal) = schema(program, actual, fields)?;
                atom(
                    program,
                    &ImmutableConstRecordProjection {
                        root: parameter.symbol,
                        fields: fields.to_vec(),
                        carrier: terminal,
                    },
                    primitive,
                )
            } else {
                let declaration = program.const_declaration_by_symbol(path.symbol)?;
                if program.normalized_type_identity(declaration.declared_type)
                    != program.normalized_type_identity(carrier)
                {
                    return None;
                }
                decoded_value(
                    program,
                    carrier,
                    fields,
                    primitive,
                    CanonicalConstValue::new(
                        program.normalized_type_identity_text(carrier),
                        declaration.evaluated_value.as_ref()?.encoding()?,
                        "",
                    ),
                )
            }
        }
        _ => None,
    };
    active.pop();
    result
}
