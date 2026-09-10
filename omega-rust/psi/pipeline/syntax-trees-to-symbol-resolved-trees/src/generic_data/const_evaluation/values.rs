//! Structural const values under exact declaration-site carrier selection.
//!
//! The header resolver selects each nominal type and constructor in its own
//! authored source, including nested field carriers. Compare those declarations
//! before erasing names into the existing canonical atom. Field order and leaf
//! landing still determine the encoded value; final resolution independently
//! checks the receiving parameter's nominal carrier.
//!
//! Structural encoding recursively lands anonymous scalar leaves at the declared
//! component type. A suffixed leaf has already landed: erasing that carrier or
//! arithmetic domain would equate a differently typed value with this component,
//! even when its numeric payload fits. Check the landing before encoding it.

use super::*;
use crate::generic_data::constant_selection::ConstantSelection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::generic_data) enum CanonicalConstNode {
    Integer {
        type_name: String,
        value: i128,
    },
    Boolean(bool),
    Array {
        type_name: String,
        values: Vec<CanonicalConstNode>,
    },
    Record {
        type_name: String,
        fields: Vec<(String, CanonicalConstNode)>,
    },
    Variant {
        type_name: String,
        case_name: String,
        fields: Vec<(String, CanonicalConstNode)>,
    },
}

impl CanonicalConstNode {
    pub(in crate::generic_data) fn encoding(&self) -> String {
        match self {
            Self::Integer { type_name, value } => {
                framed("integer", [type_name.clone(), value.to_string()])
            }
            Self::Boolean(value) => framed("boolean", [if *value { "true" } else { "false" }]),
            Self::Array { type_name, values } => framed(
                "array",
                std::iter::once(type_name.as_str().to_owned())
                    .chain(values.iter().map(Self::encoding)),
            ),
            Self::Record { type_name, fields } => framed(
                "record",
                std::iter::once(type_name.clone()).chain(
                    fields
                        .iter()
                        .flat_map(|(name, value)| [name.clone(), value.encoding()]),
                ),
            ),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } => framed(
                "variant",
                [type_name.clone(), case_name.clone()].into_iter().chain(
                    fields
                        .iter()
                        .flat_map(|(name, value)| [name.clone(), value.encoding()]),
                ),
            ),
        }
    }

    pub(in crate::generic_data) fn display(&self) -> String {
        match self {
            Self::Integer { value, .. } => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::Array { values, .. } => format!(
                "[{}]",
                values
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Record { type_name, fields } => format!(
                "{type_name} {{ {} }}",
                fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", value.display()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } if fields.is_empty() => format!("{type_name}::{case_name}"),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } => format!(
                "{type_name}::{case_name} {{ {} }}",
                fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", value.display()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

pub(in crate::generic_data) fn framed(
    tag: &str,
    pieces: impl IntoIterator<Item = impl AsRef<str>>,
) -> String {
    let mut encoded = tag.to_owned();
    for piece in pieces {
        let piece = piece.as_ref();
        encoded.push_str(&piece.len().to_string());
        encoded.push(':');
        encoded.push_str(piece);
    }
    encoded
}

pub(in crate::generic_data) fn canonicalize_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    parameter_type: TypeReferenceHandle,
) -> Result<CanonicalConstValue, String> {
    canonicalize_selected_const_definition(syntax, definition, parameter_type, None)
}

pub(in crate::generic_data) fn canonicalize_selected_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    parameter_type: TypeReferenceHandle,
    selection: Option<&ConstantSelection>,
) -> Result<CanonicalConstValue, String> {
    let declared = syntax_type_identity(syntax, definition.type_reference)?;
    let required = syntax_type_identity(syntax, parameter_type)?;
    if !same_carrier(syntax, definition.type_reference, parameter_type, selection)? {
        return Err(format!(
            "const `{}` declares type `{declared}`, but the parameter requires `{required}`",
            qualified_const_name(definition)
        ));
    }
    validate_selected_const_index_type(syntax, parameter_type, &mut Vec::new(), selection)?;
    let node = canonicalize_const_expression(
        syntax,
        definition.type_reference,
        definition.value,
        selection,
    )?;
    let required = selected_type_label(syntax, parameter_type, selection)?;
    if required == "Rat" {
        validate_canonical_rat(&node)?;
    }
    Ok(CanonicalConstValue::new(
        required,
        node.encoding(),
        node.display(),
    ))
}

pub(in crate::generic_data) fn syntax_type_identity(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Result<String, String> {
    Ok(
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Named(name) => name.as_str().to_owned(),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => format!(
                "[{}; {length}]",
                syntax_type_identity(syntax, *element_type)?
            ),
            TypeReferenceNode::Constrained { base_type, .. } => {
                syntax_type_identity(syntax, *base_type)?
            }
            TypeReferenceNode::Unit => "()".to_owned(),
            _ => {
                return Err(
                "structured const parameter types must be a canonical scalar, fixed array, or declared data value"
                    .to_owned(),
            );
            }
        },
    )
}

// Encoded labels are consistency claims. Nominal equality is established from
// the selected declarations before values lose their authored spelling; the
// receiving generic slot independently rejoins that declared carrier later.
fn selected_data<'syntax>(
    syntax: &'syntax SyntaxTrees,
    name: &Identifier,
    selection: Option<&ConstantSelection>,
) -> Result<&'syntax DataDefinition, String> {
    if let Some(selection) = selection {
        return selection.data(syntax, name);
    }
    let mut definitions = syntax.root_items().filter_map(|item| match item {
        Item::Data(definition) if definition.name.as_str() == name.as_str() => Some(definition),
        _ => None,
    });
    let definition = definitions
        .next()
        .ok_or_else(|| format!("`{name}` is not a declared canonical data type"))?;
    if definitions.next().is_some() {
        return Err(format!("`{name}` has ambiguous nominal declarations"));
    }
    Ok(definition)
}

fn selected_type_label(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    selection: Option<&ConstantSelection>,
) -> Result<String, String> {
    match syntax.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name)
            if !matches!(
                name.as_str(),
                "bool"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "addr"
                    | "f32"
                    | "f64"
                    | "string"
            ) =>
        {
            Ok(selected_data(syntax, name, selection)?
                .name
                .as_str()
                .to_owned())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => Ok(format!(
            "[{}; {length}]",
            selected_type_label(syntax, *element_type, selection)?
        )),
        TypeReferenceNode::Constrained { base_type, .. } => {
            selected_type_label(syntax, *base_type, selection)
        }
        _ => syntax_type_identity(syntax, reference),
    }
}

fn same_carrier(
    syntax: &SyntaxTrees,
    declared: TypeReferenceHandle,
    required: TypeReferenceHandle,
    selection: Option<&ConstantSelection>,
) -> Result<bool, String> {
    match (
        syntax.type_references.type_reference(declared),
        syntax.type_references.type_reference(required),
    ) {
        (TypeReferenceNode::Named(left), TypeReferenceNode::Named(right)) => {
            let primitive = |name: &str| {
                matches!(
                    name,
                    "bool"
                        | "i8"
                        | "i16"
                        | "i32"
                        | "i64"
                        | "u8"
                        | "u16"
                        | "u32"
                        | "u64"
                        | "addr"
                        | "f32"
                        | "f64"
                        | "string"
                )
            };
            if primitive(left.as_str()) || primitive(right.as_str()) {
                return Ok(left.as_str() == right.as_str());
            }
            // These references borrow the same immutable syntax owner. Its
            // exact declaration entries distinguish derived declarations too;
            // a shared template source span does not establish their identity.
            Ok(std::ptr::eq(
                selected_data(syntax, left, selection)?,
                selected_data(syntax, right, selection)?,
            ))
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: left,
                length: left_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: right,
                length: right_length,
            },
        ) => Ok(left_length == right_length && same_carrier(syntax, *left, *right, selection)?),
        (TypeReferenceNode::Constrained { base_type, .. }, _) => {
            same_carrier(syntax, *base_type, required, selection)
        }
        (_, TypeReferenceNode::Constrained { base_type, .. }) => {
            same_carrier(syntax, declared, *base_type, selection)
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => Ok(true),
        _ => Ok(false),
    }
}

pub(in crate::generic_data) fn validate_const_index_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    _visiting: &mut HashSet<String>,
) -> Result<(), String> {
    validate_selected_const_index_type(syntax, type_reference, &mut Vec::new(), None)
}

fn validate_selected_const_index_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<source::SourceSpan>,
    selection: Option<&ConstantSelection>,
) -> Result<(), String> {
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => {
            if matches!(
                name.as_str(),
                "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                    | "addr"
            ) {
                return Ok(());
            }
            if matches!(name.as_str(), "f32" | "f64" | "string") {
                return Err(format!(
                    "`{name}` is not eligible as a const index: runtime floating/text identity is not canonical structural data"
                ));
            }
            let definition = selected_data(syntax, name, selection)?;
            let declaration = definition.name.source_span();
            if visiting.contains(&declaration) {
                return Ok(());
            }
            if !definition.type_parameters.is_empty() || !definition.lifetime_parameters.is_empty() {
                return Err("generic data const carriers require closed template normalization".to_owned());
            }
            visiting.push(declaration);
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!(
                    "boundary-opaque data `{name}` is not eligible as a const index"
                ));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{name}` is not eligible as a structural const index until quotient-backed canonical representatives land"
                ));
            }
            if !definition.where_facts.is_empty() {
                return Err(format!(
                    "data `{name}` has default-domain facts whose index-site proof is not implemented; it is not yet eligible as a const index"
                ));
            }
            for member in syntax.tables.items.data_members(definition.members) {
                match member {
                    DataMember::Field(field) => validate_selected_const_index_type(
                        syntax,
                        field.type_reference,
                        visiting,
                        selection,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in syntax.tables.items.data_payload_fields(variant.payload) {
                            validate_selected_const_index_type(syntax, field.type_reference, visiting, selection)?;
                        }
                    }
                    DataMember::Retired(_) => {}
                }
            }
            visiting.pop();
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => validate_selected_const_index_type(syntax, *element_type, visiting, selection),
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_selected_const_index_type(syntax, *base_type, visiting, selection)
        }
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::SelfType => Err(
            "const index types require finite structural values with decidable equality and one canonical form"
                .to_owned(),
        ),
    }
}

pub(in crate::generic_data) fn canonicalize_const_expression(
    syntax: &SyntaxTrees,
    expected_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    selection: Option<&ConstantSelection>,
) -> Result<CanonicalConstNode, String> {
    match syntax.tables.type_references.type_reference(expected_type) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            canonicalize_const_expression(syntax, *base_type, expression, selection)
        }
        TypeReferenceNode::Named(type_name)
            if matches!(
                type_name.as_str(),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "addr"
            ) =>
        {
            let ExpressionNode::Integer(literal) = syntax.expressions.expression(expression) else {
                return Err(format!("expected an integer literal for `{type_name}`"));
            };
            if literal.landing().is_some_and(|landing| {
                landing.landed_type.name() != type_name.as_str()
                    || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
            }) {
                return Err(format!(
                    "integer literal landing conflicts with declared const component `{type_name}`"
                ));
            }
            let value = integer_literal_value(literal)
                .ok_or_else(|| "integer literal exceeds the const-value envelope".to_owned())?;
            validate_syntax_integer_range(type_name.as_str(), value)?;
            Ok(CanonicalConstNode::Integer {
                type_name: type_name.as_str().to_owned(),
                value,
            })
        }
        TypeReferenceNode::Named(type_name) if type_name.as_str() == "bool" => {
            let ExpressionNode::Boolean(value) = syntax.expressions.expression(expression) else {
                return Err("expected a boolean literal for `bool`".to_owned());
            };
            Ok(CanonicalConstNode::Boolean(*value))
        }
        TypeReferenceNode::Named(type_name) => {
            canonicalize_data_const_expression(syntax, type_name, expression, selection)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let ExpressionNode::ArrayLiteral(values) = syntax.expressions.expression(expression)
            else {
                return Err("expected an array literal for fixed-array const value".to_owned());
            };
            let values = syntax.expressions.expression_handles(*values);
            if values.len() != *length {
                return Err(format!(
                    "fixed-array const value requires {length} elements but has {}",
                    values.len()
                ));
            }
            let values = values
                .iter()
                .map(|value| {
                    canonicalize_const_expression(syntax, *element_type, *value, selection)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CanonicalConstNode::Array {
                type_name: selected_type_label(syntax, expected_type, selection)?,
                values,
            })
        }
        TypeReferenceNode::Unit => Err(
            "unit const values do not yet have a source literal; use an empty declared record"
                .to_owned(),
        ),
        _ => Err("const value expression has an ineligible parameter type".to_owned()),
    }
}

pub(in crate::generic_data) fn canonicalize_data_const_expression(
    syntax: &SyntaxTrees,
    type_name: &Identifier,
    expression: ExpressionHandle,
    selection: Option<&ConstantSelection>,
) -> Result<CanonicalConstNode, String> {
    let definition = selected_data(syntax, type_name, selection)?;
    let type_name = definition.name.as_str();
    match syntax.expressions.expression(expression) {
        ExpressionNode::StructLiteral(literal) => {
            if !std::ptr::eq(
                selected_data(syntax, &literal.type_name, selection)?,
                definition,
            ) {
                return Err(format!(
                    "const constructor `{}` selects a different nominal carrier than `{type_name}`",
                    literal.type_name
                ));
            }
            if let Some(case_name) = &literal.case_name {
                let variant = syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant)
                            if variant.name.as_str() == case_name.as_str() =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| format!("`{type_name}` has no case `{}`", case_name.as_str()))?;
                let declared_fields = syntax
                    .tables
                    .items
                    .data_payload_fields(variant.payload)
                    .iter()
                    .collect::<Vec<_>>();
                let fields =
                    canonicalize_named_fields(syntax, &declared_fields, literal.fields, selection)?;
                Ok(CanonicalConstNode::Variant {
                    type_name: type_name.to_owned(),
                    case_name: case_name.as_str().to_owned(),
                    fields,
                })
            } else {
                if syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .any(|member| matches!(member, DataMember::Variant(_)))
                {
                    return Err(format!(
                        "`{type_name}` is case data; its const value must name one case"
                    ));
                }
                let declared_fields = syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let fields =
                    canonicalize_named_fields(syntax, &declared_fields, literal.fields, selection)?;
                Ok(CanonicalConstNode::Record {
                    type_name: type_name.to_owned(),
                    fields,
                })
            }
        }
        ExpressionNode::Name(path) => {
            let path = syntax.expressions.identifier_path_members(*path);
            let Some((case_name, owner)) = path.split_last() else {
                return Err(format!("expected a `{type_name}` structural literal"));
            };
            let Some(first) = owner.first() else {
                return Err(format!("expected a `{type_name}` case owner"));
            };
            let head = Identifier::new(
                owner
                    .iter()
                    .map(Identifier::as_str)
                    .collect::<Vec<_>>()
                    .join("::"),
                first.source_span(),
            );
            if !std::ptr::eq(selected_data(syntax, &head, selection)?, definition) {
                return Err(format!(
                    "expected a `{type_name}` value, got `{}`",
                    head.as_str()
                ));
            }
            let variant = syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                        Some(variant)
                    }
                    _ => None,
                })
                .ok_or_else(|| format!("`{type_name}` has no case `{case_name}`"))?;
            if !variant.payload.is_empty() {
                return Err(format!(
                    "case `{type_name}::{case_name}` requires named payload fields"
                ));
            }
            Ok(CanonicalConstNode::Variant {
                type_name: type_name.to_owned(),
                case_name: case_name.as_str().to_owned(),
                fields: Vec::new(),
            })
        }
        _ => Err(format!("expected a `{type_name}` structural literal")),
    }
}

pub(in crate::generic_data) fn canonicalize_named_fields(
    syntax: &SyntaxTrees,
    declared_fields: &[&syntax_trees::item::DataField],
    literal_fields: HandleSpan<syntax_trees::expression::TableStructLiteralField>,
    selection: Option<&ConstantSelection>,
) -> Result<Vec<(String, CanonicalConstNode)>, String> {
    let authored = syntax.expressions.struct_fields(literal_fields);
    let mut canonical = Vec::with_capacity(declared_fields.len());
    for declared in declared_fields {
        let matches = authored
            .iter()
            .filter(|field| field.name.as_str() == declared.name.as_str())
            .collect::<Vec<_>>();
        let [field] = matches.as_slice() else {
            return Err(if matches.is_empty() {
                format!("missing const field `{}`", declared.name.as_str())
            } else {
                format!("duplicate const field `{}`", declared.name.as_str())
            });
        };
        canonical.push((
            declared.name.as_str().to_owned(),
            canonicalize_const_expression(syntax, declared.type_reference, field.value, selection)?,
        ));
    }
    for field in authored {
        if !declared_fields
            .iter()
            .any(|declared| declared.name.as_str() == field.name.as_str())
        {
            return Err(format!("unknown const field `{}`", field.name.as_str()));
        }
    }
    Ok(canonical)
}

pub(in crate::generic_data) fn validate_syntax_integer_range(
    type_name: &str,
    value: i128,
) -> Result<(), String> {
    let (minimum, maximum) = match type_name {
        "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
        "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
        "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
        "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
        "u8" => (0, i128::from(u8::MAX)),
        "u16" => (0, i128::from(u16::MAX)),
        "u32" => (0, i128::from(u32::MAX)),
        "u64" | "addr" => (0, i128::from(u64::MAX)),
        _ => return Err(format!("`{type_name}` is not an integer const type")),
    };
    if value < minimum || value > maximum {
        Err(format!("const value `{value}` does not fit `{type_name}`"))
    } else {
        Ok(())
    }
}

pub(in crate::generic_data) fn validate_canonical_rat(
    value: &CanonicalConstNode,
) -> Result<(), String> {
    let CanonicalConstNode::Record { fields, .. } = value else {
        return Err("`Rat` index value must be a structural record".to_owned());
    };
    let numerator = fields
        .iter()
        .find(|(name, _)| name == "num")
        .map(|(_, value)| value)
        .ok_or_else(|| "`Rat` index value is missing `num`".to_owned())?;
    let denominator = fields
        .iter()
        .find(|(name, _)| name == "den")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat` index value is missing `den`".to_owned())?;
    let CanonicalConstNode::Record { fields, .. } = numerator else {
        return Err("`Rat.num` must be an `IntPair` record".to_owned());
    };
    let negative = fields
        .iter()
        .find(|(name, _)| name == "neg")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat.num` is missing `neg`".to_owned())?;
    let positive = fields
        .iter()
        .find(|(name, _)| name == "pos")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat.num` is missing `pos`".to_owned())?;
    if denominator == 0 {
        return Err("`Rat` index denominator must be positive".to_owned());
    }
    if negative != 0 && positive != 0 {
        return Err(
            "`Rat` index signed coordinates must be cancelled (at least one of `num.neg` and `num.pos` must be zero)"
                .to_owned(),
        );
    }
    let magnitude = negative.max(positive);
    if gcd_usize(magnitude, denominator) != 1 {
        return Err(
            "`Rat` index numerator magnitude and denominator must be gcd-reduced".to_owned(),
        );
    }
    Ok(())
}

pub(in crate::generic_data) fn nat_value(value: &CanonicalConstNode) -> Result<usize, String> {
    match value {
        CanonicalConstNode::Variant {
            type_name,
            case_name,
            fields,
        } if type_name == "Nat" && case_name == "Zero" && fields.is_empty() => Ok(0),
        CanonicalConstNode::Variant {
            type_name,
            case_name,
            fields,
        } if type_name == "Nat" && case_name == "Succ" => {
            let previous = fields
                .iter()
                .find(|(name, _)| name == "prev")
                .map(|(_, value)| nat_value(value))
                .transpose()?
                .ok_or_else(|| "`Nat::Succ` is missing `prev`".to_owned())?;
            previous
                .checked_add(1)
                .ok_or_else(|| "`Nat` const value is too large".to_owned())
        }
        _ => Err("`Rat` canonicality requires structural core `Nat` fields".to_owned()),
    }
}

pub(in crate::generic_data) fn gcd_usize(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub(in crate::generic_data) fn const_integer_in_envelope(value: i128) -> Option<i128> {
    (value >= i128::from(i64::MIN) && value <= i128::from(u64::MAX)).then_some(value)
}

pub(in crate::generic_data) fn checked_fact_integer(
    value: Option<i128>,
    operation: &str,
) -> Result<ConstFactValue, String> {
    value
        .and_then(const_integer_in_envelope)
        .map(ConstFactValue::Integer)
        .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
}
