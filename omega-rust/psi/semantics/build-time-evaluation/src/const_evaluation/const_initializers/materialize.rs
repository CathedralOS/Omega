//! Structured leaf materialization and independent result encoding.
//!
//! An aggregate-producing leaf's checked interpreter result becomes
//! authored-shape literal syntax again only through the receiving
//! declaration's own source scope: the constructor spelling must re-resolve at
//! the leaf's source span before emission, so a same-named foreign carrier cannot
//! borrow the result. Receiving replay re-encodes interpreter results against the
//! declared carrier with `canonical_value` instead of trusting the
//! materialized literal, so both sides of the receipt stay independent.

use language_semantics::const_value::DecodedCanonicalConstValue;
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableStructLiteral, TableStructLiteralField,
};
use syntax_trees::identifier::Identifier;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use crate::BuildTimeValue;

/// Rewrite one admitted structured value as authored literal syntax. Every
/// constructor name re-resolves through `reference` to the exact selected
/// declaration the interpreter result claimed.
pub(super) fn literal(
    syntax: &mut SyntaxTrees,
    program: &TypedTrees,
    destination: TypeReferenceHandle,
    value: &BuildTimeValue,
    reference: SourceSpan,
) -> Result<ExpressionHandle, String> {
    let node = match program.type_reference_table.type_reference(destination) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return literal(syntax, program, *base_type, value, reference);
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err(
                    "evaluated aggregate leaf did not produce its declared array carrier"
                        .to_owned(),
                );
            };
            if elements.len() != *length {
                return Err("evaluated aggregate leaf array shape drifted".to_owned());
            }
            let mut materialized = Vec::with_capacity(elements.len());
            for element in elements {
                materialized.push(literal(syntax, program, *element_type, element, reference)?);
            }
            let elements = syntax.expressions.insert_expression_handles(materialized);
            ExpressionNode::ArrayLiteral(elements)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some(primitive) =
                crate::const_evaluation::const_generic_expressions::exact_probe_destination(
                    program,
                    destination,
                )
            {
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
                            numerics::literals::IntegerLiteral::from_parts(
                                primitive.is_signed_integer() && *value < 0,
                                numerics::literals::IntegerRadix::Decimal,
                                &digits,
                            )
                            .map_err(str::to_owned)?,
                        )
                    }
                    _ => {
                        return Err(
                            "evaluated aggregate leaf field does not match its declared scalar carrier"
                                .to_owned(),
                        );
                    }
                }
            } else {
                nominal_literal(syntax, program, *symbol, value, reference)?
            }
        }
        _ => {
            return Err("evaluated aggregate leaf has no materializable carrier".to_owned());
        }
    };
    let handle = syntax.expressions.insert(node);
    syntax.expressions.set_source_span(handle, reference);
    Ok(handle)
}

/// Materialize one record or realized case value as its selected constructor
/// literal. The owner path is taken from the symbol's own declaration and must
/// re-resolve to that exact symbol at the leaf's source span.
fn nominal_literal(
    syntax: &mut SyntaxTrees,
    program: &TypedTrees,
    symbol: SymbolHandle,
    value: &BuildTimeValue,
    reference: SourceSpan,
) -> Result<ExpressionNode, String> {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let definition = definitions
        .next()
        .ok_or("evaluated aggregate leaf has no exact nominal declaration")?;
    if definitions.next().is_some() {
        return Err("evaluated aggregate leaf has ambiguous nominal identity".to_owned());
    }
    let owner_path = program.symbols.display_path(symbol, "::");
    if program
        .symbols
        .find_top_level_by_name_and_kinds_from_source(&owner_path, &[SymbolKind::Data], reference)
        != Some(symbol)
    {
        return Err(format!(
            "evaluated constructor `{owner_path}` cannot reselect its exact carrier from the constant's source"
        ));
    }
    // We are returning to pre-normalization syntax, where a closed instance's
    // synthetic name is not a declaration. Reconstruct the template constructor
    // from its retained generic origin, never by stripping display text. The
    // destination and interpreted fields still carry the exact closed tuple;
    // syntax canonicalization and receiving replay independently check it.
    let owner_path = if let Some(origin) = definition.generic_instance {
        let TypeReferenceNode::Generic { base_symbol, .. } =
            program.type_reference_table.type_reference(origin)
        else {
            return Err("evaluated generic constructor lost its template origin".to_owned());
        };
        let template_path = program.symbols.display_path(*base_symbol, "::");
        if !base_symbol.is_valid()
            || program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    &template_path,
                    &[SymbolKind::Data],
                    reference,
                )
                != Some(*base_symbol)
        {
            return Err(
                "evaluated generic constructor cannot reselect its exact template".to_owned(),
            );
        }
        template_path
    } else {
        owner_path
    };
    let members = program.data_members(definition);
    let (constructor, declared) = match value {
        BuildTimeValue::Struct { type_name, fields } => {
            if type_name != definition.name.as_str() {
                return Err(format!(
                    "evaluated aggregate leaf expected record `{}`, found record `{type_name}`",
                    definition.name
                ));
            }
            (
                owner_path,
                members
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some((field, fields)),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            )
        }
        BuildTimeValue::Case { variant, payload } => {
            let case = members
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(case) if case.name.as_str() == variant.as_str() => {
                        Some(case)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    format!("evaluated aggregate leaf case `{variant}` is absent from its owner")
                })?;
            (
                format!("{owner_path}::{variant}"),
                program
                    .data_payload_fields(case)
                    .iter()
                    .map(|field| (field, payload))
                    .collect::<Vec<_>>(),
            )
        }
        _ => {
            return Err("evaluated aggregate leaf is not a nominal constructor value".to_owned());
        }
    };
    let mut literal_fields = Vec::with_capacity(declared.len());
    for (field, supplied) in declared {
        let value = supplied
            .iter()
            .find(|(name, _)| name.as_str() == field.name.as_str())
            .map(|(_, value)| value)
            .ok_or_else(|| {
                format!(
                    "evaluated aggregate leaf lost declared field `{}`",
                    field.name
                )
            })?;
        let value = literal(syntax, program, field.type_reference, value, reference)?;
        literal_fields.push(TableStructLiteralField {
            // Materialized field names are compiler-derived copies, not
            // authored selections: a generated identifier carries no token
            // span, so distinct declared fields cannot collide as "copies of
            // one authored field selection".
            name: Identifier::generated(field.name.as_str()),
            value,
        });
    }
    let fields = syntax.expressions.insert_struct_fields(literal_fields);
    Ok(ExpressionNode::StructLiteral(TableStructLiteral {
        // Keep the source location but not a fabricated authored token shared
        // by every generated constructor. Lookup was checked above against the
        // exact owner; the receiving carrier and initializer receipt still
        // independently validate the generated value.
        constructor_name: Identifier::new(
            constructor,
            SourceSpan::new(
                reference.source_id,
                source::Span::new(reference.span.start, reference.span.start),
            ),
        ),
        fields,
    }))
}

/// Encode one checked interpreter result against its declared carrier without
/// consulting the materialized literal — the receiving replay's independent
/// reconstruction of the same canonical const identity.
pub(super) fn canonical_value(
    program: &TypedTrees,
    destination: TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<DecodedCanonicalConstValue, String> {
    match program.type_reference_table.type_reference(destination) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            canonical_value(program, *base_type, value)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err("evaluated leaf did not produce its declared array carrier".to_owned());
            };
            if elements.len() != *length {
                return Err("evaluated leaf array shape drifted".to_owned());
            }
            let mut values = Vec::with_capacity(elements.len());
            for element in elements {
                values.push(canonical_value(program, *element_type, element)?);
            }
            Ok(DecodedCanonicalConstValue::Array {
                type_name: type_label(program, destination)?,
                values,
            })
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some(primitive) =
                crate::const_evaluation::const_generic_expressions::exact_probe_destination(
                    program,
                    destination,
                )
            {
                match (primitive, value) {
                    (PrimitiveType::Bool, BuildTimeValue::Bool(value)) => {
                        Ok(DecodedCanonicalConstValue::Boolean(*value))
                    }
                    (primitive, BuildTimeValue::Int(value))
                        if primitive.accepts_integer_literal() =>
                    {
                        Ok(DecodedCanonicalConstValue::Integer {
                            type_name: primitive.name().to_owned(),
                            value: if primitive.is_signed_integer() {
                                i128::from(*value)
                            } else {
                                i128::from(*value as u64)
                            },
                        })
                    }
                    _ => {
                        Err("evaluated leaf does not match its declared scalar carrier".to_owned())
                    }
                }
            } else {
                nominal_value(program, *symbol, value)
            }
        }
        _ => Err("evaluated leaf has no encodable carrier".to_owned()),
    }
}

fn nominal_value(
    program: &TypedTrees,
    symbol: SymbolHandle,
    value: &BuildTimeValue,
) -> Result<DecodedCanonicalConstValue, String> {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let definition = definitions
        .next()
        .ok_or("evaluated leaf has no exact nominal declaration")?;
    if definitions.next().is_some() {
        return Err("evaluated leaf has ambiguous nominal identity".to_owned());
    }
    let members = program.data_members(definition);
    let type_name = definition.name.as_str().to_owned();
    let encode_fields = |declared: &[&typed_trees::data::DataField],
                         supplied: &[(String, BuildTimeValue)]|
     -> Result<Vec<(String, DecodedCanonicalConstValue)>, String> {
        let mut fields = Vec::with_capacity(declared.len());
        for field in declared {
            let value = supplied
                .iter()
                .find(|(name, _)| name.as_str() == field.name.as_str())
                .map(|(_, value)| value)
                .ok_or_else(|| format!("evaluated leaf lost declared field `{}`", field.name))?;
            fields.push((
                field.name.as_str().to_owned(),
                canonical_value(program, field.type_reference, value)?,
            ));
        }
        Ok(fields)
    };
    match value {
        BuildTimeValue::Struct {
            type_name: actual,
            fields,
        } => {
            if actual != &type_name {
                return Err(format!(
                    "evaluated leaf expected record `{type_name}`, found record `{actual}`"
                ));
            }
            let declared = members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    _ => None,
                })
                .collect::<Vec<_>>();
            Ok(DecodedCanonicalConstValue::Record {
                type_name,
                fields: encode_fields(&declared, fields)?,
            })
        }
        BuildTimeValue::Case { variant, payload } => {
            let case = members
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(case) if case.name.as_str() == variant.as_str() => {
                        Some(case)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    format!("evaluated leaf case `{variant}` is absent from its owner")
                })?;
            let declared = program.data_payload_fields(case).iter().collect::<Vec<_>>();
            Ok(DecodedCanonicalConstValue::Variant {
                type_name,
                case_name: variant.clone(),
                fields: encode_fields(&declared, payload)?,
            })
        }
        _ => Err("evaluated leaf is not a nominal constructor value".to_owned()),
    }
}

/// The canonical carrier label a closed destination contributes to its value
/// encoding: the declared simple name for nominal carriers, the builtin
/// spelling for scalars, and `[element; length]` for fixed arrays — matching
/// the syntax-side `selected_type_label`/`syntax_type_identity` contract.
fn type_label(program: &TypedTrees, destination: TypeReferenceHandle) -> Result<String, String> {
    match program.type_reference_table.type_reference(destination) {
        TypeReferenceNode::Named { symbol, name } => {
            if program.symbols.builtin_type_atom(*symbol).is_some() {
                return Ok(name.as_str().to_owned());
            }
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|definition| definition.symbol == *symbol);
            let definition = definitions
                .next()
                .ok_or("aggregate leaf carrier has no exact nominal declaration")?;
            if definitions.next().is_some() {
                return Err("aggregate leaf carrier has ambiguous nominal identity".to_owned());
            }
            Ok(definition.name.as_str().to_owned())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => Ok(format!(
            "[{}; {length}]",
            type_label(program, *element_type)?
        )),
        TypeReferenceNode::Constrained { base_type, .. } => type_label(program, *base_type),
        TypeReferenceNode::Unit => Ok("()".to_owned()),
        _ => Err("aggregate leaf carrier has no canonical label".to_owned()),
    }
}
