use super::{
    TopLevelSymbols, TypeParameterScope, TypeReferenceOwner, type_reference_label,
    type_references_match,
};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::const_value::{
    CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{
    DataMember, MachineParameterContract, TypeParameter, TypeParameterKind,
};
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::HashSet;

pub(super) fn validate_machine_data_argument(
    program: &TypedTrees,
    base_name: &str,
    parameter: &TypeParameter,
    contract: &MachineParameterContract,
    argument: TypeReferenceHandle,
    _type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "machine parameter `{}` of proof data `{base_name}` requires a static machine symbol argument",
            parameter.name
        )));
        return;
    };

    // Recursive references inside the family (`CauchySeq<S>`) forward the
    // family parameter governed by this exact declaration-site contract.
    if *symbol == parameter.symbol {
        return;
    }

    let generic_types = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == base_name)
        .map(|definition| {
            program
                .data_type_parameters(definition)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // A distinct in-scope machine parameter is admissible only through the
    // same recursive refinement judgment as a concrete symbol. This is the
    // higher-order N7 path (`Family<Inner>` inside a schema parameter): its
    // authored contract, never its name alone, proves compatibility.
    crate::machine_parameters::validate_data_machine_selection(
        program,
        base_name,
        parameter,
        contract,
        *symbol,
        name.as_str(),
        &generic_types,
        diagnostics,
    );
}

pub(super) fn machine_argument_name<'program>(
    program: &'program TypedTrees,
    argument: TypeReferenceHandle,
    type_parameter_scope: TypeParameterScope<'program>,
) -> Option<&'program str> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        return None;
    };
    if type_parameter_scope
        .machine_parameter(*symbol, name.as_str())
        .is_some()
        || program
            .machines()
            .iter()
            .any(|machine| machine.symbol == *symbol || machine.name.as_str() == name.as_str())
    {
        Some(name.as_str())
    } else {
        None
    }
}

pub(super) fn validate_symbolic_array_length(
    program: &TypedTrees,
    length: &FixedArrayLength,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    let FixedArrayLength::ConstParameter { symbol, name } = length else {
        return;
    };
    let parameter = type_parameter_scope
        .type_parameters
        .iter()
        .find(|parameter| {
            (symbol.is_valid() && parameter.symbol == *symbol)
                || parameter.name.as_str() == name.as_str()
        });
    let Some(parameter) = parameter else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses unresolved fixed-array length `{name}`; a symbolic array length \
             must name an in-scope `const` parameter"
        )));
        return;
    };
    let TypeParameterKind::Const { type_reference } = parameter.kind else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses type parameter `{name}` as a fixed-array length; array lengths \
             must name a `const` parameter"
        )));
        return;
    };
    let Some(primitive) = program.type_reference_table.primitive_type(type_reference) else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses const parameter `{name}` with a non-primitive type as a \
             fixed-array length; array lengths require an integer primitive"
        )));
        return;
    };
    if !primitive.accepts_integer_literal() {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses const parameter `{name}` of non-integer type `{}` as a \
             fixed-array length",
            primitive.name(),
        )));
    }
}

pub(super) fn validate_const_data_argument(
    program: &TypedTrees,
    base_name: &str,
    parameter: &TypeParameter,
    parameter_type: TypeReferenceHandle,
    argument: TypeReferenceHandle,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let primitive = program.type_reference_table.primitive_type(parameter_type);
    let is_integer_parameter = primitive.is_some_and(PrimitiveType::accepts_integer_literal);
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        diagnostics.push(Diagnostic::error(if is_integer_parameter {
            format!(
                "const parameter `{}` of `{base_name}` requires an integer literal argument",
                parameter.name
            )
        } else {
            format!(
                "const parameter `{}` of `{base_name}` requires a canonical const value",
                parameter.name
            )
        }));
        return;
    };

    if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
        if exact_structured_const_data_carrier_is_eligible(program, parameter_type) {
            if let Err(reason) =
                validate_exact_typed_structured_const_argument(program, parameter_type, &value)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "const parameter `{}` of `{base_name}` has an invalid canonical value: {reason}",
                    parameter.name
                )));
            }
            return;
        }
        if let Err(reason) =
            validate_typed_const_index_type(program, parameter_type, &mut HashSet::new())
        {
            diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` has an ineligible index type: {reason}",
                parameter.name
            )));
            return;
        }
        let required_type = type_reference_label(program, parameter_type);
        if value.type_name != required_type {
            diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` has type `{required_type}`, but its canonical value has type `{}`",
                parameter.name, value.type_name
            )));
        }
        return;
    }

    if let Ok(value) = name.as_str().parse::<i128>() {
        let Some(primitive) = primitive.filter(|primitive| primitive.accepts_integer_literal())
        else {
            diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` requires a `{}` value, not integer `{value}`",
                parameter.name,
                type_reference_label(program, parameter_type),
            )));
            return;
        };
        validate_const_integer_range(base_name, parameter, primitive, value, diagnostics);
        return;
    }

    let forwarded = type_parameter_scope
        .type_parameters
        .iter()
        .find(|candidate| {
            if symbol.is_valid() {
                candidate.symbol == *symbol
            } else {
                candidate.name.as_str() == name.as_str()
            }
        });
    let Some(forwarded) = forwarded else {
        diagnostics.push(Diagnostic::error(if is_integer_parameter {
            format!(
                "const parameter `{}` of `{base_name}` requires an integer literal argument or an in-scope const parameter, got `{name}`",
                parameter.name,
            )
        } else {
            format!(
                "const parameter `{}` of `{base_name}` requires a canonical const value or an in-scope const parameter, got `{name}`",
                parameter.name,
            )
        }));
        return;
    };
    let TypeParameterKind::Const {
        type_reference: forwarded_type,
    } = forwarded.kind
    else {
        diagnostics.push(Diagnostic::error(format!(
            "const parameter `{}` of `{base_name}` requires a value, not a type",
            parameter.name
        )));
        return;
    };
    if !type_references_match(program, forwarded_type, parameter_type) {
        diagnostics.push(Diagnostic::error(format!(
            "const parameter `{}` of `{base_name}` has type `{}`, but forwarded const parameter `{}` has type `{}`",
            parameter.name,
            type_reference_label(program, parameter_type),
            forwarded.name,
            type_reference_label(program, forwarded_type),
        )));
    }
}

/// Replay one compiler-generated structured const atom against its exact typed
/// carrier. Encoded type and field names are canonical-format consistency
/// checks only; data selection always follows the resolved symbol carried by
/// `expected_type`.
///
/// This first exact cohort is intentionally narrower than the ordinary const
/// index surface: acyclic, monomorphic checked records and pure sums composed
/// only of exact integer/Boolean primitives, literal fixed arrays, and data of
/// the same cohort. Mixed data and types with separate canonical-representative
/// rules remain with the broader producer judgment until their typed replay is
/// shared too.
pub(crate) fn validate_exact_typed_structured_const_argument(
    program: &TypedTrees,
    expected_type: TypeReferenceHandle,
    value: &CanonicalConstValue,
) -> Result<(), String> {
    if !exact_structured_const_data_carrier_is_eligible(program, expected_type) {
        return Err(format!(
            "`{}` is outside the exact acyclic structured-data const cohort",
            type_reference_label(program, expected_type)
        ));
    }
    let expected_name = type_reference_label(program, expected_type);
    if value.type_name != expected_name {
        return Err(format!(
            "expected carrier `{expected_name}`, but the canonical atom names `{}`",
            value.type_name
        ));
    }
    let decoded = value
        .decode_encoding()
        .ok_or_else(|| "the canonical const encoding is malformed".to_owned())?;
    validate_decoded_structured_const(program, expected_type, &decoded, &mut Vec::new())
}

pub(crate) fn validate_exact_const_identity(
    program: &TypedTrees,
    expected_type: TypeReferenceHandle,
    identity: &CanonicalConstIdentity,
) -> Result<(), String> {
    if let Some(primitive) = exact_const_primitive(program, expected_type) {
        if identity.type_name != primitive.name() {
            return Err(format!(
                "expected carrier `{}`, but the canonical value names `{}`",
                primitive.name(),
                identity.type_name
            ));
        }
        let decoded = identity
            .decode_encoding()
            .ok_or_else(|| "the canonical const encoding is malformed".to_owned())?;
        return validate_decoded_structured_const(
            program,
            expected_type,
            &decoded,
            &mut Vec::new(),
        );
    }
    let value = CanonicalConstValue::new(
        identity.type_name.clone(),
        identity.encoding.clone(),
        String::new(),
    );
    validate_exact_typed_structured_const_argument(program, expected_type, &value)
}

/// Replay a retained canonical const encoding against one exact resolved
/// carrier. This syntax-free entry is for downstream compiler stages: names
/// embedded in the encoding are consistency claims, never type authority.
pub fn validate_exact_const_value_encoding(
    program: &TypedTrees,
    expected_type: TypeReferenceHandle,
    encoding: &str,
) -> Result<(), String> {
    let decoded = CanonicalConstValue::new("", encoding, "")
        .decode_encoding()
        .ok_or_else(|| "the canonical const encoding is malformed".to_owned())?;
    let type_name = match decoded {
        DecodedCanonicalConstValue::Integer { type_name, .. }
        | DecodedCanonicalConstValue::Array { type_name, .. }
        | DecodedCanonicalConstValue::Record { type_name, .. }
        | DecodedCanonicalConstValue::Variant { type_name, .. } => type_name,
        DecodedCanonicalConstValue::Boolean(_) => "bool".to_owned(),
    };
    validate_exact_const_identity(
        program,
        expected_type,
        &CanonicalConstIdentity {
            type_name,
            encoding: encoding.to_owned(),
        },
    )
}

fn exact_structured_const_data_carrier_is_eligible(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    symbol.is_valid()
        && program
            .data_definitions()
            .iter()
            .any(|definition| definition.symbol == *symbol)
        && exact_structured_const_carrier_is_eligible(program, type_reference, &mut Vec::new())
}

fn exact_structured_const_carrier_is_eligible(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> bool {
    if let Some(primitive) = exact_const_primitive(program, type_reference) {
        return primitive == PrimitiveType::Bool || primitive.accepts_integer_literal();
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => exact_structured_const_carrier_is_eligible(program, *element_type, visiting),
        TypeReferenceNode::Named { symbol, .. } => {
            if !symbol.is_valid() || visiting.contains(symbol) {
                return false;
            }
            visiting.push(*symbol);
            let eligible = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
                .is_some_and(|definition| {
                    let members = program.data_members(definition);
                    definition.supply_mode == psi_language_semantics::DataSupplyMode::CheckedShape
                        && definition.name.as_str() != "Rat"
                        && definition.lifetime_parameters.is_empty()
                        && program.data_type_parameters(definition).is_empty()
                        && definition.generic_instance.is_none()
                        && definition.quotient.is_none()
                        && definition.where_facts.is_empty()
                        && !definition.zero_gated
                        && match psi_typed_trees::data::DataDefinition::shape_kind_from_members(
                            members,
                        ) {
                            psi_typed_trees::data::DataShapeKind::Record => {
                                members.iter().all(|member| {
                                    let DataMember::Field(field) = member else {
                                        return false;
                                    };
                                    !field.relevance.is_erased()
                                        && exact_structured_const_carrier_is_eligible(
                                            program,
                                            field.type_reference,
                                            visiting,
                                        )
                                })
                            }
                            psi_typed_trees::data::DataShapeKind::Enum => {
                                members.iter().all(|member| {
                                    let DataMember::Variant(variant) = member else {
                                        return false;
                                    };
                                    program.data_payload_fields(variant).iter().all(|field| {
                                        !field.relevance.is_erased()
                                            && exact_structured_const_carrier_is_eligible(
                                                program,
                                                field.type_reference,
                                                visiting,
                                            )
                                    })
                                })
                            }
                            psi_typed_trees::data::DataShapeKind::Empty
                            | psi_typed_trees::data::DataShapeKind::Mixed => false,
                        }
                });
            visiting.pop();
            eligible
        }
        _ => false,
    }
}

fn validate_decoded_structured_const(
    program: &TypedTrees,
    expected_type: TypeReferenceHandle,
    value: &DecodedCanonicalConstValue,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> Result<(), String> {
    if let Some(primitive) = exact_const_primitive(program, expected_type) {
        return match (primitive, value) {
            (PrimitiveType::Bool, DecodedCanonicalConstValue::Boolean(_)) => Ok(()),
            (primitive, DecodedCanonicalConstValue::Integer { type_name, value })
                if primitive.accepts_integer_literal()
                    && type_name == primitive.name()
                    && const_integer_value_fits_primitive(primitive, *value) =>
            {
                Ok(())
            }
            (PrimitiveType::Bool, _) => Err("expected a canonical Boolean node".to_owned()),
            (primitive, DecodedCanonicalConstValue::Integer { type_name, value })
                if primitive.accepts_integer_literal() =>
            {
                Err(format!(
                    "integer node `{type_name}({value})` does not fit exact carrier `{}`",
                    primitive.name()
                ))
            }
            (primitive, _) => Err(format!(
                "expected a canonical integer node for `{}`",
                primitive.name()
            )),
        };
    }

    match (
        program.type_reference_table.type_reference(expected_type),
        value,
    ) {
        (
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            },
            DecodedCanonicalConstValue::Array { type_name, values },
        ) => {
            let expected_name = type_reference_label(program, expected_type);
            if type_name != &expected_name {
                return Err(format!(
                    "array node names `{type_name}`, expected `{expected_name}`"
                ));
            }
            if values.len() != *length {
                return Err(format!(
                    "array node has {} elements, expected {length}",
                    values.len()
                ));
            }
            for value in values {
                validate_decoded_structured_const(program, *element_type, value, visiting)?;
            }
            Ok(())
        }
        (
            TypeReferenceNode::Named { symbol, .. },
            DecodedCanonicalConstValue::Record { type_name, fields },
        ) => {
            if !symbol.is_valid() || visiting.contains(symbol) {
                return Err("structured const carrier is unresolved or recursive".to_owned());
            }
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
                .ok_or_else(|| {
                    "structured const carrier symbol has no data definition".to_owned()
                })?;
            if type_name.as_str() != definition.name.as_str() {
                return Err(format!(
                    "record node names `{type_name}`, expected `{}`",
                    definition.name
                ));
            }
            let members = program.data_members(definition);
            if psi_typed_trees::data::DataDefinition::shape_kind_from_members(members)
                != psi_typed_trees::data::DataShapeKind::Record
            {
                return Err("canonical record node names non-record data".to_owned());
            }
            if fields.len() != members.len() {
                return Err(format!(
                    "record node has {} fields, expected {}",
                    fields.len(),
                    members.len()
                ));
            }
            visiting.push(*symbol);
            let result = members.iter().zip(fields).try_for_each(
                |(member, (encoded_name, encoded_value))| {
                    let DataMember::Field(field) = member else {
                        return Err("structured const carrier contains a case".to_owned());
                    };
                    if encoded_name.as_str() != field.name.as_str() {
                        return Err(format!(
                            "record field `{encoded_name}` appears where `{}` is required",
                            field.name
                        ));
                    }
                    validate_decoded_structured_const(
                        program,
                        field.type_reference,
                        encoded_value,
                        visiting,
                    )
                },
            );
            visiting.pop();
            result
        }
        (
            TypeReferenceNode::Named { symbol, .. },
            DecodedCanonicalConstValue::Variant {
                type_name,
                case_name,
                fields,
            },
        ) => {
            if !symbol.is_valid() || visiting.contains(symbol) {
                return Err("structured const carrier is unresolved or recursive".to_owned());
            }
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
                .ok_or_else(|| {
                    "structured const carrier symbol has no data definition".to_owned()
                })?;
            if type_name.as_str() != definition.name.as_str() {
                return Err(format!(
                    "variant node names `{type_name}`, expected `{}`",
                    definition.name
                ));
            }
            let members = program.data_members(definition);
            if psi_typed_trees::data::DataDefinition::shape_kind_from_members(members)
                != psi_typed_trees::data::DataShapeKind::Enum
            {
                return Err("canonical variant node names non-sum data".to_owned());
            }
            let mut matching_cases = members.iter().filter_map(|member| {
                let DataMember::Variant(variant) = member else {
                    return None;
                };
                (variant.name.as_str() == case_name.as_str()).then_some(variant)
            });
            let variant = matching_cases.next().ok_or_else(|| {
                format!(
                    "sum `{}` has no case `{case_name}`",
                    definition.name.as_str()
                )
            })?;
            if matching_cases.next().is_some() {
                return Err(format!(
                    "sum `{}` has duplicate case name `{case_name}`",
                    definition.name.as_str()
                ));
            }
            let payload = program.data_payload_fields(variant);
            if fields.len() != payload.len() {
                return Err(format!(
                    "case `{case_name}` has {} payload fields, expected {}",
                    fields.len(),
                    payload.len()
                ));
            }
            visiting.push(*symbol);
            let result = payload.iter().zip(fields).try_for_each(
                |(field, (encoded_name, encoded_value))| {
                    if encoded_name.as_str() != field.name.as_str() {
                        return Err(format!(
                            "case payload field `{encoded_name}` appears where `{}` is required",
                            field.name
                        ));
                    }
                    validate_decoded_structured_const(
                        program,
                        field.type_reference,
                        encoded_value,
                        visiting,
                    )
                },
            );
            visiting.pop();
            result
        }
        _ => Err(format!(
            "canonical node does not match exact carrier `{}`",
            type_reference_label(program, expected_type)
        )),
    }
}

fn exact_const_primitive(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let primitive = match program.symbols.builtin_type_atom(*symbol) {
        Some(psi_symbols::BuiltinTypeAtom::Bool) => PrimitiveType::Bool,
        Some(psi_symbols::BuiltinTypeAtom::I8) => PrimitiveType::I8,
        Some(psi_symbols::BuiltinTypeAtom::I16) => PrimitiveType::I16,
        Some(psi_symbols::BuiltinTypeAtom::I32) => PrimitiveType::I32,
        Some(psi_symbols::BuiltinTypeAtom::I64) => PrimitiveType::I64,
        Some(psi_symbols::BuiltinTypeAtom::U8) => PrimitiveType::U8,
        Some(psi_symbols::BuiltinTypeAtom::U16) => PrimitiveType::U16,
        Some(psi_symbols::BuiltinTypeAtom::U32) => PrimitiveType::U32,
        Some(psi_symbols::BuiltinTypeAtom::U64) => PrimitiveType::U64,
        Some(psi_symbols::BuiltinTypeAtom::Address) => PrimitiveType::Addr,
        Some(_) => return None,
        None if !symbol.is_valid() => PrimitiveType::from_name(name.as_str())?,
        None => return None,
    };
    (name.as_str() == primitive.name()).then_some(primitive)
}

fn validate_typed_const_index_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<String>,
) -> Result<(), String> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            if matches!(
                name.as_str(),
                "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                    | "addr"
            ) {
                return Ok(());
            }
            if matches!(name.as_str(), "f32" | "f64" | "string") {
                return Err(format!(
                    "`{name}` does not have canonical structural index identity"
                ));
            }
            if !visiting.insert(name.as_str().to_owned()) {
                return Ok(());
            }
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name.as_str())
                .ok_or_else(|| format!("`{name}` is not declared data"))?;
            if definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!("boundary-opaque data `{name}` is not structural"));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{name}` has no compiler-verified canonical representative"
                ));
            }
            if !definition.where_facts.is_empty() {
                return Err(format!(
                    "data `{name}` has default-domain facts whose index-site proof is not implemented"
                ));
            }
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) => validate_typed_const_index_type(
                        program,
                        field.type_reference,
                        visiting,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in program.data_payload_fields(variant) {
                            validate_typed_const_index_type(
                                program,
                                field.type_reference,
                                visiting,
                            )?;
                        }
                    }
                }
            }
            visiting.remove(name.as_str());
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => validate_typed_const_index_type(program, *element_type, visiting),
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_typed_const_index_type(program, *base_type, visiting)
        }
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. } => Err(
            "indices require finite structural values with decidable equality and one canonical form"
                .to_owned(),
        ),
    }
}

fn validate_const_integer_range(
    base_name: &str,
    parameter: &TypeParameter,
    primitive: PrimitiveType,
    value: i128,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !const_integer_value_fits_primitive(primitive, value) {
        diagnostics.push(Diagnostic::error(format!(
            "const argument `{value}` for `{base_name}::{}` does not fit `{}`",
            parameter.name,
            primitive.name()
        )));
    }
}

pub(crate) fn const_integer_value_fits_primitive(primitive: PrimitiveType, value: i128) -> bool {
    let (minimum, maximum) = match primitive {
        PrimitiveType::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
        PrimitiveType::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
        PrimitiveType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveType::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        PrimitiveType::U8 => (0, i128::from(u8::MAX)),
        PrimitiveType::U16 => (0, i128::from(u16::MAX)),
        PrimitiveType::U32 => (0, i128::from(u32::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => (0, i128::from(u64::MAX)),
        _ => return false,
    };
    value >= minimum && value <= maximum
}

/// Instantiation-time property-bound check (frozen decision 13): every type
/// argument must carry each bound the matching parameter declares, e.g.
/// `Box<String>` is rejected when `Box` declares `data Box<T [copy]>`. Bounds
/// on in-scope type parameters count, so `data Outer<U [copy]>` may store a
/// `Box<U>`.
pub(super) fn validate_generic_argument_bounds(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    base_name: &str,
    arguments: psi_arena::HandleSpan<TypeReferenceHandle>,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == base_name)
    else {
        return;
    };

    let parameters = program.data_type_parameters(definition);
    let argument_handles = program
        .type_reference_table
        .type_reference_handles(arguments);
    for (parameter, argument) in parameters.iter().zip(argument_handles) {
        let bounds = crate::properties::declared_property_requirements(&parameter.bounds);
        let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
        for property in bounds {
            if crate::properties::type_satisfies_declared_property(
                program,
                symbols,
                type_parameter_scope.type_parameters,
                *argument,
                property,
            ) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "type parameter `{} [{}]` of `{base_name}` was instantiated with `{}`, which does not satisfy `[{property}]`",
                parameter.name,
                bound_labels.join(", "),
                type_reference_label(program, *argument)
            )));
        }
    }
}
