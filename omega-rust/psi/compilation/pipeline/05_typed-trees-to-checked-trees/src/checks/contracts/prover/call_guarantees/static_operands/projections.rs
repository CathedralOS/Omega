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
use validation::{ConstProjectionStep, ImmutableConstProjection, ScopedArithmeticValue};

pub(super) fn atom(
    program: &dyn validation::CheckingInput,
    projection: &ImmutableConstProjection,
    primitive: PrimitiveType,
) -> Option<ScopedArithmeticValue> {
    (program.primitive_type_reference(projection.carrier) == Some(primitive)).then(|| {
        ScopedArithmeticValue::Atom {
            identity: format!(
                "\0static-parameter:{:?}:path:{:?}",
                projection.root, projection.path
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
    projection: &ImmutableConstProjection,
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
            &projection.path,
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
                    &ImmutableConstProjection {
                        root: *symbol,
                        path: projection.path.clone(),
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
            decoded_value(program, carrier, &projection.path, primitive, value)
        }
        _ => None,
    }
}

/// Walk a projection path through its declared carriers. Each field step
/// names its declared field; each element step stays inside the fixed extent.
fn schema(
    program: &dyn validation::CheckingInput,
    mut carrier: TypeReferenceHandle,
    path: &[ConstProjectionStep],
) -> Option<TypeReferenceHandle> {
    for step in path {
        carrier = step_carrier(program, carrier, *step)?;
    }
    Some(carrier)
}

fn step_carrier(
    program: &dyn validation::CheckingInput,
    carrier: TypeReferenceHandle,
    step: ConstProjectionStep,
) -> Option<TypeReferenceHandle> {
    match step {
        ConstProjectionStep::Field(field) => {
            field_step(program, carrier, field).map(|(_, carrier)| carrier)
        }
        ConstProjectionStep::Element(index) => {
            let (element, length) = validation::fixed_array_geometry(program, carrier)?;
            (index < length).then_some(element)
        }
    }
}

/// The declared name and carrier of one exact record field of `carrier`.
fn field_step(
    program: &dyn validation::CheckingInput,
    carrier: TypeReferenceHandle,
    field: SymbolHandle,
) -> Option<(String, TypeReferenceHandle)> {
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
        .filter(|candidate| candidate.symbol == field);
    let selected = selected.next().filter(|_| selected.next().is_none())?;
    Some((selected.name.as_str().to_owned(), selected.type_reference))
}

fn decoded_value(
    program: &dyn validation::CheckingInput,
    carrier: TypeReferenceHandle,
    path: &[ConstProjectionStep],
    primitive: PrimitiveType,
    value: CanonicalConstValue,
) -> Option<ScopedArithmeticValue> {
    if program.type_identity_by_text(&value.type_name) != program.normalized_type_identity(carrier)
    {
        return None;
    }
    validation::validate_exact_const_value_encoding(program, carrier, &value.encoding).ok()?;
    let terminal = schema(program, carrier, path)?;
    if program.primitive_type_reference(terminal) != Some(primitive) {
        return None;
    }
    let mut decoded = value.decode_encoding()?;
    let mut current = carrier;
    for step in path {
        decoded = match (step, decoded) {
            (
                ConstProjectionStep::Field(field),
                DecodedCanonicalConstValue::Record { fields, .. },
            ) => {
                let (name, _) = field_step(program, current, *field)?;
                fields.into_iter().find(|(field, _)| *field == name)?.1
            }
            (
                ConstProjectionStep::Element(index),
                DecodedCanonicalConstValue::Array { values, .. },
            ) => values.into_iter().nth(*index)?,
            _ => return None,
        };
        current = step_carrier(program, current, *step)?;
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
    path: &[ConstProjectionStep],
    primitive: PrimitiveType,
    active: &mut Vec<ExpressionHandle>,
) -> Option<ScopedArithmeticValue> {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return None;
    }
    if path.is_empty() {
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
            let ConstProjectionStep::Field(field) = path[0] else {
                return None;
            };
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
            let child_carrier = step_carrier(program, carrier, path[0])?;
            let selected = supplied.iter().find(|value| value.field_symbol == field)?;
            expression_value(
                program,
                facts,
                invocation,
                selected.value,
                child_carrier,
                &path[1..],
                primitive,
                active,
            )
        }
        // An element step selects the literal's own element under the
        // array's declared element carrier; the extent must match exactly.
        ExpressionNode::ArrayLiteral(elements) => {
            let ConstProjectionStep::Element(index) = path[0] else {
                return None;
            };
            let (_, length) = validation::fixed_array_geometry(program, carrier)?;
            let elements = program.expression_table.expression_handles(*elements);
            if elements.len() != length {
                return None;
            }
            let child_carrier = step_carrier(program, carrier, path[0])?;
            expression_value(
                program,
                facts,
                invocation,
                *elements.get(index)?,
                child_carrier,
                &path[1..],
                primitive,
                active,
            )
        }
        ExpressionNode::Name(name) => {
            if let Some(parameter) = program.data_type_parameter_by_symbol(name.symbol) {
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
                let terminal = schema(program, actual, path)?;
                atom(
                    program,
                    &ImmutableConstProjection {
                        root: parameter.symbol,
                        path: path.to_vec(),
                        carrier: terminal,
                    },
                    primitive,
                )
            } else {
                let declaration = program.const_declaration_by_symbol(name.symbol)?;
                if program.normalized_type_identity(declaration.declared_type)
                    != program.normalized_type_identity(carrier)
                {
                    return None;
                }
                decoded_value(
                    program,
                    carrier,
                    path,
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
