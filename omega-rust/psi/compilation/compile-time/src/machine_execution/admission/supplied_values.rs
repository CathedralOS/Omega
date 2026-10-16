//! Establish host-supplied values before admitting them as language arguments.
//!
//! Reflection constructs owned snapshots outside authored expression checking.
//! A well-shaped snapshot does not establish its declared predicates. Encode
//! each arrival against the exact parameter carrier, then use ordinary checked
//! construction on a private statement context. This is the first
//! language construction judgment for these values, not replay of a stage's
//! checked output. The policy body remains independently admitted and executed.
//! Nested fields, array elements and active payloads retain their declarations;
//! no schema-specific bounds or second predicate evaluator belong here.

use build_time_values::BuildTimeValue;
use numerics::literals::{FloatLiteral, IntegerLiteral, IntegerRadix};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableStructLiteral, TableStructLiteralField,
};
use typed_trees::machine::Machine;
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn require_established_arguments(
    program: &TypedTrees,
    machine: &Machine,
    arguments: &mut [BuildTimeValue],
) -> Result<(), String> {
    let entry = program
        .machine_states(machine)
        .first()
        .ok_or("policy has no entry state")?;
    let parameters = program.state_parameters(entry);
    if parameters.len() != arguments.len() {
        return Err("supplied policy arguments do not match the entry arity".into());
    }
    let mut probe = program.clone();
    let mut constructions = Vec::with_capacity(arguments.len());
    for (parameter, value) in parameters.iter().zip(arguments) {
        // A borrowed policy arrival refers to the evaluator's fresh owned
        // snapshot. Establish its pointee, not an escaping reference result.
        let destination = match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            _ => parameter.type_reference,
        };
        let value = literal(&mut probe, destination, value)
            .map_err(|reason| format!("policy argument `{}`: {reason}", parameter.name))?;
        // Receive the root through an ordinary declared field as well. Merely
        // checking its child constructors would miss a qualification on the
        // parameter itself (or a primitive/array root's receiving obligations).
        let container = probe.symbols.insert_generated_root_from(
            machine.symbol,
            symbols::SymbolKind::Data,
            "@supplied-carrier",
        );
        let field = probe
            .symbols
            .insert_generated_children(container, [(symbols::SymbolKind::Field, "value")])
            .start();
        let mut definition = DataDefinition {
            symbol: container,
            name: typed_trees::name::Identifier::generated("@supplied-carrier"),
            ..Default::default()
        };
        probe.push_data_member(
            &mut definition,
            DataMember::Field(DataField {
                symbol: field,
                name: typed_trees::name::Identifier::generated("value"),
                type_reference: destination,
                ..Default::default()
            }),
        );
        let fields = probe
            .expression_table
            .insert_struct_fields([TableStructLiteralField {
                name: typed_trees::name::Identifier::generated("value"),
                field_symbol: field,
                value,
            }]);
        let expression =
            probe
                .expression_table
                .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: definition.name.clone(),
                    type_symbol: container,
                    fields,
                    ..Default::default()
                }));
        let destination = probe.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: container,
            name: definition.name.clone(),
        });
        probe.push_data_definition(definition);
        probe.expression_table.set_source_span(
            expression,
            program
                .symbols
                .symbol_source_span(parameter.symbol)
                .unwrap_or_default(),
        );
        let owner = super::concrete_invocation::append_invocation_probe(
            &mut probe,
            machine.symbol,
            "@supplied-value",
            expression,
            destination,
        );
        constructions.push((owner, expression));
    }
    // Only these new constructions owe a judgment. Re-running stage checking
    // would duplicate already-admitted bodies and repeatedly prepare thousands
    // of reflected padding cells. The same construction walk is independently
    // callable without granting any machine admission or execution evidence.
    let checking = validation::CheckingContext::new(&probe);
    for (owner, expression) in constructions {
        let machine = probe
            .machine(owner)
            .ok_or("supplied-value construction lost its owner")?;
        let state = probe
            .machine_states(machine)
            .first()
            .ok_or("supplied-value construction lost its state")?;
        validation::validate_value_construction(&checking, machine, state, expression).map_err(
            |diagnostics| {
                format!(
                    "supplied policy arguments failed value establishment: {}",
                    diagnostics::join_messages(&diagnostics)
                )
            },
        )?;
    }
    Ok(())
}

/// Encode values, not new evidence. Ordinary checking owns all obligations.
fn literal(
    program: &mut TypedTrees,
    destination: TypeReferenceHandle,
    value: &mut BuildTimeValue,
) -> Result<ExpressionHandle, String> {
    let carrier = program
        .type_reference_table
        .type_reference(destination)
        .clone();
    let node = match carrier {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return literal(program, base_type, value);
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err("expected an array snapshot".into());
            };
            if elements.len() != length {
                return Err(format!(
                    "expected {length} elements, found {}",
                    elements.len()
                ));
            }
            let mut expressions = Vec::with_capacity(length);
            for element in elements.iter_mut() {
                expressions.push(literal(program, element_type, element)?);
            }
            ExpressionNode::ArrayLiteral(
                program
                    .expression_table
                    .insert_expression_handles(expressions),
            )
        }
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(primitive) = PrimitiveType::from_name(name.as_str()) {
                match (primitive, value) {
                    (PrimitiveType::Bool, BuildTimeValue::Bool(value)) => {
                        ExpressionNode::Boolean(*value)
                    }
                    (primitive, BuildTimeValue::Int(value))
                        if primitive.accepts_integer_literal() =>
                    {
                        let digits = if primitive.is_signed_integer() {
                            value.unsigned_abs().to_string()
                        } else {
                            (*value as u64).to_string()
                        };
                        ExpressionNode::Integer(
                            IntegerLiteral::from_parts(
                                primitive.is_signed_integer() && *value < 0,
                                IntegerRadix::Decimal,
                                &digits,
                            )
                            .map_err(str::to_owned)?,
                        )
                    }
                    (PrimitiveType::F32 | PrimitiveType::F64, BuildTimeValue::Float(value)) => {
                        ExpressionNode::Float(FloatLiteral::from_f64(*value))
                    }
                    (
                        PrimitiveType::F32 | PrimitiveType::F64,
                        BuildTimeValue::UndeterminedNaN(format),
                    ) => ExpressionNode::UndeterminedNaN(*format),
                    _ => return Err(format!("snapshot does not match primitive `{name}`")),
                }
            } else {
                let definition = program
                    .data_definition_by_symbol(symbol)
                    .ok_or("snapshot has no exact data declaration")?
                    .clone();
                let (case_name, case_symbol, declared, supplied) = match value {
                    BuildTimeValue::Struct { type_name, fields } => {
                        if type_name != definition.name.as_str() {
                            return Err(format!(
                                "expected `{}`, found `{type_name}`",
                                definition.name
                            ));
                        }
                        let declared = program
                            .data_members(&definition)
                            .iter()
                            .filter_map(|member| member.as_field().cloned())
                            .collect::<Vec<_>>();
                        (None, None, declared, fields)
                    }
                    BuildTimeValue::Case { variant, payload } => {
                        let selected = program
                            .data_members(&definition)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(candidate)
                                    if candidate.name.as_str() == variant =>
                                {
                                    Some(candidate.clone())
                                }
                                _ => None,
                            })
                            .ok_or_else(|| {
                                format!("unknown case `{variant}` of `{}`", definition.name)
                            })?;
                        (
                            Some(selected.name.clone()),
                            Some(selected.symbol),
                            program.data_payload_fields(&selected).to_vec(),
                            payload,
                        )
                    }
                    _ => {
                        return Err(format!(
                            "expected a declared value of `{}`",
                            definition.name
                        ));
                    }
                };
                // Reflection exposes an extensible metadata roster; a policy
                // may declare an earlier schema shape. Project to its exact
                // declared fields before checking AND execution. Unknown host
                // metadata never becomes an undeclared language field, while
                // missing or repeated declared fields remain errors.
                supplied
                    .retain(|(name, _)| declared.iter().any(|field| name == field.name.as_str()));
                if declared.len() != supplied.len() {
                    return Err(format!(
                        "expected {} fields, found {}",
                        declared.len(),
                        supplied.len()
                    ));
                }
                let mut fields = Vec::with_capacity(declared.len());
                for field in declared {
                    let mut matches = supplied
                        .iter_mut()
                        .filter(|(name, _)| name == field.name.as_str());
                    let (_, value) = matches
                        .next()
                        .ok_or_else(|| format!("missing field `{}`", field.name))?;
                    if matches.next().is_some() {
                        return Err(format!("repeated field `{}`", field.name));
                    }
                    fields.push(TableStructLiteralField {
                        name: field.name,
                        field_symbol: field.symbol,
                        value: literal(program, field.type_reference, value)?,
                    });
                }
                ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: definition.name,
                    type_symbol: symbol,
                    case_name,
                    case_symbol,
                    fields: program.expression_table.insert_struct_fields(fields),
                })
            }
        }
        _ => return Err("snapshot has no closed literal carrier".into()),
    };
    Ok(program.expression_table.insert(node))
}
