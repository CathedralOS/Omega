use super::{
    TopLevelSymbols, TypeParameterScope, TypeReferenceOwner, type_reference_label,
    type_references_match,
};
use diagnostics::Diagnostic;
use language_semantics::const_value::{
    CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
};
use std::collections::HashSet;
use symbols::SymbolHandle;
use typed_trees::data::{
    DataField, DataMember, MachineParameterContract, TypeParameter, TypeParameterKind,
};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_machine_data_argument(
    program: &dyn crate::CheckingInput,
    base_symbol: SymbolHandle,
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
        .find(|definition| {
            definition.symbol == base_symbol
                || (!program.symbols.has_source_metadata() && definition.name.as_str() == base_name)
        })
        .map(|definition| program.data_type_parameters(definition))
        .or_else(|| {
            program
                .trait_definition_by_symbol(base_symbol)
                .map(|definition| program.trait_type_parameters(definition))
        })
        .unwrap_or(&[])
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .collect::<Vec<_>>();
    // A distinct in-scope machine parameter is admissible only through the
    // same recursive refinement judgment as a concrete symbol. This is the
    // higher-order N7 path (`Family<Inner>` inside a schema parameter): its
    // authored contract, never its name alone, proves compatibility.
    crate::machine_calls::machine_parameters::validate_data_machine_selection(
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
    program: &'program dyn crate::CheckingInput,
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
        || program.machines().iter().any(|machine| {
            machine.symbol == *symbol
                || (!program.symbols.has_source_metadata()
                    && machine.name.as_str() == name.as_str())
        })
    {
        Some(name.as_str())
    } else {
        None
    }
}

pub(super) fn validate_symbolic_array_length(
    program: &dyn crate::CheckingInput,
    length: &FixedArrayLength,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    if let FixedArrayLength::Expression(value) = length {
        let TypeReferenceNode::ConstExpression(root) =
            program.type_reference_table.type_reference(*value)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} has an unclosed array extent without its expression recipe"
            )));
            return;
        };
        let parameters = type_parameter_scope
            .type_parameters
            .iter()
            .filter_map(|parameter| {
                let type_reference = match parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => type_reference,
                    _ => return None,
                };
                Some(typed_trees::signature::StateParameter {
                    symbol: parameter.symbol,
                    name: parameter.name.clone(),
                    type_reference,
                    is_const: matches!(parameter.kind, TypeParameterKind::Const { .. }),
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        let prior = program
            .data_definition_by_symbol(type_parameter_scope.lexical_owner)
            .map(|definition| {
                program
                    .proof_facts
                    .span_or_empty(definition.where_facts)
                    .iter()
                    .filter_map(typed_trees::domain::ProofFact::as_expression)
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        crate::proof_contracts::contract_entailment::specification_calls::validate_declaration_expression_formation(
            program, type_parameter_scope.lexical_owner, &parameters, *root, &prior, &owner.to_string(), diagnostics,
        );
        let natural = crate::parameter_expression_result_type_reference(
            program,
            type_parameter_scope.lexical_owner,
            &[],
            *root,
        )
        .and_then(|result| program.primitive_type_reference(result));
        let integer = natural.is_some_and(|primitive| primitive.accepts_integer_literal());
        let anonymous = crate::has_anonymous_numeric_results(program, *root)
            && crate::evaluate_anonymous_numeric_expression(program, *root, &[], |expression| {
                match program.expression_table.expression(expression) {
                    typed_trees::expression::ExpressionNode::Binary(binary) => {
                        typed_trees::operator::binary_operator_spelling(binary.operator).is_none_or(
                            |spelling| {
                                typed_trees::operator::has_builtin_spelled_expression_meaning(
                                    program,
                                    type_parameter_scope.lexical_owner,
                                    expression,
                                    spelling,
                                    &[None, None],
                                )
                            },
                        )
                    }
                    typed_trees::expression::ExpressionNode::Unary(_) => true,
                    _ => false,
                }
            })
            .and_then(|value| value.to_integer_exact())
            .and_then(|value| value.to_u64())
            .is_some();
        if !integer && !anonymous {
            diagnostics.push(Diagnostic::error(format!("{owner} array extent expression requires a well-typed integer result or exact nonnegative anonymous integer"))
                .with_source_span(program.expression_table.source_span(*root)));
        }
        return;
    }
    let FixedArrayLength::ConstParameter { symbol, name } = length else {
        return;
    };
    // Stage 02 binds an extent to the binder its owner declares. A spelling
    // that never resolved is not that binder, even when the scope holds a
    // same-named one: an attached machine that omits `<const N: ...>` names
    // its data's `N` without binding it.
    let parameter = type_parameter_scope
        .type_parameters
        .iter()
        .find(|parameter| symbol.is_valid() && parameter.symbol == *symbol);
    let Some(parameter) = parameter else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses unresolved fixed-array length `{name}`; a symbolic array length \
             must name an in-scope `const` parameter"
        )));
        return;
    };
    let (TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference }) =
        parameter.kind
    else {
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

/// Validate one normalized, closed const argument against its declared carrier.
///
/// Callers must check named declarations and forwarded binders against this
/// carrier before erasing their identities into value leaves. This judgment
/// admits neither unresolved binders nor unevaluated expressions and applies
/// even when the selected binder has no body occurrences.
pub fn validate_closed_const_argument(
    program: &dyn crate::CheckingInput,
    owner: &str,
    parameter: &TypeParameter,
    argument: TypeReferenceHandle,
) -> Result<(), Vec<Diagnostic>> {
    let (TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference }) =
        parameter.kind
    else {
        return Err(vec![Diagnostic::error(format!(
            "parameter `{}` of `{owner}` is not a const parameter",
            parameter.name,
        ))]);
    };
    if !argument.is_valid()
        || !matches!(
            program.type_reference_table.type_reference(argument),
            TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid()
        )
    {
        return Err(vec![Diagnostic::error(format!(
            "const parameter `{}` of `{owner}` requires a normalized closed value",
            parameter.name,
        ))]);
    }
    let mut diagnostics = Vec::new();
    validate_const_data_argument(
        program,
        owner,
        parameter,
        type_reference,
        argument,
        TypeParameterScope {
            lexical_owner: symbols::SymbolHandle::invalid(),
            type_parameters: &[],
            lifetime_parameters: &[],
        },
        &mut diagnostics,
    );
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Deriving an ordinary call substitution does not admit a const by shape.
/// Reuse constructor admission with the caller's exact lexical telescope;
/// runtime value binders cannot fill compile-time const slots.
pub(crate) fn const_argument_matches_parameter(
    program: &dyn crate::CheckingInput,
    parameter: &TypeParameter,
    argument: TypeReferenceHandle,
    caller_parameters: &[TypeParameter],
) -> bool {
    let TypeParameterKind::Const { type_reference } = parameter.kind else {
        return false;
    };
    if let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(argument)
        && caller_parameters.iter().any(|candidate| {
            candidate.symbol == *symbol && matches!(candidate.kind, TypeParameterKind::Value { .. })
        })
    {
        return false;
    }
    let mut diagnostics = Vec::new();
    validate_const_data_argument(
        program,
        "call",
        parameter,
        type_reference,
        argument,
        TypeParameterScope {
            lexical_owner: symbols::SymbolHandle::invalid(),
            type_parameters: caller_parameters,
            lifetime_parameters: &[],
        },
        &mut diagnostics,
    );
    !diagnostics.iter().any(Diagnostic::is_error)
}

pub(crate) fn validate_const_data_argument(
    program: &dyn crate::CheckingInput,
    base_name: &str,
    parameter: &TypeParameter,
    parameter_type: TypeReferenceHandle,
    argument: TypeReferenceHandle,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Ordinary value binders retain a runtime subject, independently of its
    // carrier. Const binders continue through static admission below.
    if matches!(parameter.kind, TypeParameterKind::Value { .. })
        && let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(argument)
        && symbol.is_valid()
        && matches!(
            program.symbols.get(*symbol).kind,
            symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
        )
    {
        let actual = crate::value_custody::places::declaration_type_reference(program, *symbol);
        if !actual.is_some_and(|actual| type_references_match(program, actual, parameter_type)) {
            diagnostics.push(Diagnostic::error(format!(
                "value parameter `{}` of `{base_name}` requires `{}`, but runtime subject `{name}` has a different carrier",
                parameter.name, type_reference_label(program, parameter_type),
            )));
        }
        return;
    }
    let primitive = program.type_reference_table.primitive_type(parameter_type);
    let is_integer_parameter = primitive.is_some_and(PrimitiveType::accepts_integer_literal);
    if let TypeReferenceNode::ConstExpression(root) =
        program.type_reference_table.type_reference(argument)
    {
        // A pending-marked application is pre-check continuation custody: the
        // semantic evaluation owner folds it once selected provider execution
        // is known. Preliminary checking tolerates the authored shape; every
        // other mode rejects surviving marks before reaching this argument.
        if program.pending_const_range_endpoints.contains(root) {
            return;
        }
    }
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

    // A resolved named value retains its declaration's carrier. Its spelling
    // is diagnostic payload, not a literal or a forwarded binder. Reuse exact
    // const admission without evaluating the initializer in this application.
    if symbol.is_valid()
        && let Some(declaration) = program.const_declaration_by_symbol(*symbol)
    {
        let admission =
            if !type_references_match(program, declaration.declared_type, parameter_type) {
                Err(format!(
                    "named constant `{name}` has type `{}`, expected `{}`",
                    type_reference_label(program, declaration.declared_type),
                    type_reference_label(program, parameter_type)
                ))
            } else if let Some(encoding) = declaration
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding())
            {
                validate_exact_const_value_encoding(program, parameter_type, encoding)
            } else {
                Err(format!(
                    "named constant `{name}` has no evaluated const value"
                ))
            };
        if let Err(reason) = admission {
            diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` has an invalid named value: {reason}",
                parameter.name
            )));
        }
        return;
    }

    if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
        if exact_structured_const_carrier_is_eligible(program, parameter_type, &mut Vec::new()) {
            if let Err(reason) =
                validate_exact_const_identity(program, parameter_type, &value.identity())
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
        validate_const_integer_range(
            program,
            base_name,
            parameter,
            parameter_type,
            primitive,
            value,
            diagnostics,
        );
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
    let (TypeParameterKind::Const {
        type_reference: forwarded_type,
    }
    | TypeParameterKind::Value {
        type_reference: forwarded_type,
    }) = forwarded.kind
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
    program: &dyn crate::CheckingInput,
    expected_type: TypeReferenceHandle,
    value: &CanonicalConstValue,
) -> Result<(), String> {
    if !exact_structured_const_data_carrier_is_eligible(program, expected_type) {
        return Err(format!(
            "`{}` is outside the exact acyclic structured-data const cohort",
            type_reference_label(program, expected_type)
        ));
    }
    let expected_name = canonical_carrier_label(program, expected_type);
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
    program: &dyn crate::CheckingInput,
    expected_type: TypeReferenceHandle,
    identity: &CanonicalConstIdentity,
) -> Result<(), String> {
    let expected_name = if let Some(primitive) = exact_const_primitive(program, expected_type) {
        primitive.name().to_owned()
    } else if matches!(
        program.type_reference_table.type_reference(expected_type),
        TypeReferenceNode::FixedArray { .. }
    ) && exact_structured_const_carrier_is_eligible(
        program,
        expected_type,
        &mut Vec::new(),
    ) {
        canonical_carrier_label(program, expected_type)
    } else {
        let value = CanonicalConstValue::new(
            identity.type_name.clone(),
            identity.encoding.clone(),
            String::new(),
        );
        return validate_exact_typed_structured_const_argument(program, expected_type, &value);
    };
    if identity.type_name != expected_name {
        return Err(format!(
            "expected carrier `{expected_name}`, but the canonical value names `{}`",
            identity.type_name
        ));
    }
    let decoded = identity
        .decode_encoding()
        .ok_or_else(|| "the canonical const encoding is malformed".to_owned())?;
    validate_decoded_structured_const(program, expected_type, &decoded, &mut Vec::new())
}

/// Replay a retained canonical const encoding against one exact resolved
/// carrier. This syntax-free entry is for downstream compiler stages: names
/// embedded in the encoding are consistency claims, never type authority.
pub fn validate_exact_const_value_encoding(
    program: &dyn crate::CheckingInput,
    expected_type: TypeReferenceHandle,
    encoding: &str,
) -> Result<(), String> {
    let decoded = CanonicalConstValue::new("", encoding, "")
        .decode_encoding()
        .ok_or_else(|| "the canonical const encoding is malformed".to_owned())?;
    let type_name = match decoded {
        DecodedCanonicalConstValue::Float { .. } => {
            return Err("floating declaration value is not a const index".into());
        }
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

/// Canonical labels follow the independently resolved carrier declaration.
/// A caller's qualification or import alias cannot change a value encoding;
/// the label still supplies no nominal authority without the exact typed slot.
fn canonical_carrier_label(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> String {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                .map_or_else(
                    || type_reference_label(program, reference),
                    |definition| definition.name.as_str().to_owned(),
                )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => format!(
            "[{}; {length}]",
            canonical_carrier_label(program, *element_type)
        ),
        _ => type_reference_label(program, reference),
    }
}

fn exact_structured_const_data_carrier_is_eligible(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    symbol.is_valid()
        && crate::declarations::data_queries::data_definition_by_symbol(program, *symbol).is_some()
        && exact_structured_const_carrier_is_eligible(program, type_reference, &mut Vec::new())
}

fn exact_structured_const_carrier_is_eligible(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<symbols::SymbolHandle>,
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
            let eligible =
                crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                    .is_some_and(|definition| {
                        let members = program.data_members(definition);
                        definition.supply_mode == language_semantics::DataSupplyMode::CheckedShape
                            && definition.name.as_str() != "Rat"
                            && definition.lifetime_parameters.is_empty()
                            && program.data_type_parameters(definition).is_empty()
                            && definition.generic_instance.is_none()
                            && definition.quotient.is_none()
                            && definition.where_facts.is_empty()
                            && !definition.zero_gated
                            && match typed_trees::data::DataDefinition::shape_kind_from_members(
                                members,
                            ) {
                                typed_trees::data::DataShapeKind::Record => {
                                    members.iter().all(|member| {
                                        let Some(field) = member.as_field() else {
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
                                typed_trees::data::DataShapeKind::Enum => {
                                    members.iter().all(|member| {
                                        let Some(variant) = member.as_variant() else {
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
                                typed_trees::data::DataShapeKind::Empty
                                | typed_trees::data::DataShapeKind::Mixed => false,
                            }
                    });
            visiting.pop();
            eligible
        }
        _ => false,
    }
}

/// The data definition a `Named` carrier's decoded node names: an
/// unresolved or recursive symbol and a declared type name mismatch are
/// errors; `node_kind` is `record` or `variant` in the diagnostic.
fn structured_const_carrier_definition<'p>(
    program: &'p dyn crate::CheckingInput,
    symbol: &symbols::SymbolHandle,
    type_name: &str,
    node_kind: &str,
    visiting: &[symbols::SymbolHandle],
) -> Result<&'p typed_trees::data::DataDefinition, String> {
    if !symbol.is_valid() || visiting.contains(symbol) {
        return Err("structured const carrier is unresolved or recursive".to_owned());
    }
    let definition = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
        .ok_or_else(|| "structured const carrier symbol has no data definition".to_owned())?;
    if type_name != definition.name.as_str() {
        return Err(format!(
            "{node_kind} node names `{type_name}`, expected `{}`",
            definition.name
        ));
    }
    Ok(definition)
}

fn validate_decoded_structured_const(
    program: &dyn crate::CheckingInput,
    expected_type: TypeReferenceHandle,
    value: &DecodedCanonicalConstValue,
    visiting: &mut Vec<symbols::SymbolHandle>,
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
            let expected_name = canonical_carrier_label(program, expected_type);
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
            let definition = structured_const_carrier_definition(
                program,
                symbol,
                type_name.as_str(),
                "record",
                visiting,
            )?;
            let members = program.data_members(definition);
            if typed_trees::data::DataDefinition::shape_kind_from_members(members)
                != typed_trees::data::DataShapeKind::Record
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
                    let Some(field) = member.as_field() else {
                        return Err("structured const carrier contains a case".to_owned());
                    };
                    validate_named_field_value(
                        program,
                        encoded_name.as_str(),
                        field,
                        encoded_value,
                        "record field",
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
            let definition = structured_const_carrier_definition(
                program,
                symbol,
                type_name.as_str(),
                "variant",
                visiting,
            )?;
            let members = program.data_members(definition);
            if typed_trees::data::DataDefinition::shape_kind_from_members(members)
                != typed_trees::data::DataShapeKind::Enum
            {
                return Err("canonical variant node names non-sum data".to_owned());
            }
            let mut matching_cases = members.iter().filter_map(|member| {
                let variant = member.as_variant()?;
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
                    validate_named_field_value(
                        program,
                        encoded_name.as_str(),
                        field,
                        encoded_value,
                        "case payload field",
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

/// One decoded field-payload pair: the encoded name must spell the declared
/// field's name, then the value recurses through the structured-const
/// validation with the visiting guard intact.
fn validate_named_field_value(
    program: &dyn crate::CheckingInput,
    encoded_name: &str,
    field: &DataField,
    encoded_value: &DecodedCanonicalConstValue,
    field_role: &str,
    visiting: &mut Vec<SymbolHandle>,
) -> Result<(), String> {
    if encoded_name != field.name.as_str() {
        return Err(format!(
            "{field_role} `{encoded_name}` appears where `{}` is required",
            field.name
        ));
    }
    validate_decoded_structured_const(program, field.type_reference, encoded_value, visiting)
}

fn exact_const_primitive(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let primitive = match program.symbols.builtin_type_atom(*symbol) {
        Some(symbols::BuiltinTypeAtom::Bool) => PrimitiveType::Bool,
        Some(symbols::BuiltinTypeAtom::I8) => PrimitiveType::I8,
        Some(symbols::BuiltinTypeAtom::I16) => PrimitiveType::I16,
        Some(symbols::BuiltinTypeAtom::I32) => PrimitiveType::I32,
        Some(symbols::BuiltinTypeAtom::I64) => PrimitiveType::I64,
        Some(symbols::BuiltinTypeAtom::U8) => PrimitiveType::U8,
        Some(symbols::BuiltinTypeAtom::U16) => PrimitiveType::U16,
        Some(symbols::BuiltinTypeAtom::U32) => PrimitiveType::U32,
        Some(symbols::BuiltinTypeAtom::U64) => PrimitiveType::U64,
        Some(symbols::BuiltinTypeAtom::Address) => PrimitiveType::Addr,
        Some(_) => return None,
        None if !symbol.is_valid() => PrimitiveType::from_name(name.as_str())?,
        None => return None,
    };
    (name.as_str() == primitive.name()).then_some(primitive)
}

fn validate_typed_const_index_type(
    program: &dyn crate::CheckingInput,
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
                .data_definition_by_name_str(name.as_str())
                .ok_or_else(|| format!("`{name}` is not declared data"))?;
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
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
    program: &dyn crate::CheckingInput,
    base_name: &str,
    parameter: &TypeParameter,
    mut parameter_type: TypeReferenceHandle,
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
        return;
    }
    let exact_value = numerics::bignum::BigInt::from_i128(value);
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(parameter_type)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            let typed_trees::types::TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                continue;
            };
            let bounds = crate::closed_integer_range_bound(program, *minimum).zip(
                crate::closed_integer_range_maximum(program, *maximum, *end_inclusive),
            );
            let Some((minimum, maximum)) = bounds else {
                diagnostics.push(Diagnostic::error(format!(
                    "const argument `{value}` for `{base_name}::{}` cannot establish its declared range from closed integer bounds",
                    parameter.name,
                )));
                continue;
            };
            if exact_value < minimum || exact_value > maximum {
                diagnostics.push(Diagnostic::error(format!(
                    "const argument `{value}` for `{base_name}::{}` is outside its declared range `[{minimum}..={maximum}]`",
                    parameter.name,
                )));
            }
        }
        parameter_type = *base_type;
    }
}

/// The declaration twin of `validate_const_integer_range`. An integer const
/// or value binder's carrier range is established by each argument from
/// closed integer bounds, so a range that does not close (`const N: u64
/// [0..K]`, naming a sibling binder) admits no argument. Stage 02 also leaves
/// such an endpoint unbound; refusing the declaration keeps every carrier
/// range a closed interval that index proofs read without a spelling.
/// Preliminary checking tolerates an endpoint whose evaluation is pending.
/// Every `const N: T` and `u: T` binder's carrier is an ordinary type
/// reference in its declaration's scope: it must resolve like a field type,
/// and an integer carrier's declared ranges must close over integers.
pub(crate) fn validate_const_binder_carriers(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    owner: &str,
    parameters: &[TypeParameter],
    scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for parameter in parameters {
        let (TypeParameterKind::Const {
            type_reference: carrier,
        }
        | TypeParameterKind::Value {
            type_reference: carrier,
        }) = parameter.kind
        else {
            continue;
        };
        super::validate_type_reference_handle(
            program,
            carrier,
            symbols,
            diagnostics,
            TypeReferenceOwner::GenericBinder {
                owner,
                parameter: parameter.name.as_str(),
                generic_depth: 0,
            },
            scope,
        );
        let Some(primitive_type) = program
            .type_reference_table
            .primitive_type(carrier)
            .filter(|primitive_type| primitive_type.accepts_integer_literal())
        else {
            continue;
        };
        let (ranges, _) = super::integer_ranges::constrained_range_constraints(program, carrier);
        for (minimum, maximum, end_inclusive) in ranges {
            let closes = crate::closed_integer_range_bound(program, minimum).is_some()
                && crate::closed_integer_range_maximum(program, maximum, end_inclusive).is_some();
            // The same pending admission formation applies to a declared
            // range: every leaf closes or is a marked endpoint call.
            let mut found_pending = false;
            let pending = [minimum, maximum].into_iter().all(|endpoint| {
                let (closed, marked) =
                    super::constraint_validation::range_bound_pending_evaluation_shape(
                        program,
                        primitive_type,
                        endpoint,
                    );
                found_pending |= marked;
                closed
            }) && found_pending;
            if !closes && !pending {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} parameter `{}` declares a range that does not close over \
                     integers; an argument establishes its declared range from closed \
                     integer bounds",
                    parameter.name,
                )));
            }
        }
    }
}

pub(crate) fn const_integer_value_fits_primitive(primitive: PrimitiveType, value: i128) -> bool {
    primitive
        .integer_bounds()
        .is_some_and(|(minimum, maximum)| value >= minimum && value <= maximum)
}

/// Instantiation-time property-bound check (frozen decision 13): every type
/// argument must carry each bound the matching parameter declares, e.g.
/// `Box<String>` is rejected when `Box` declares `data Box<T [copy]>`. Bounds
/// on in-scope type parameters count, so `data Outer<U [copy]>` may store a
/// `Box<U>`.
pub(super) fn validate_generic_argument_bounds(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    base_symbol: SymbolHandle,
    base_name: &str,
    arguments: arena::HandleSpan<TypeReferenceHandle>,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(definition) = program.data_definitions().iter().find(|definition| {
        definition.symbol == base_symbol
            || (!program.symbols.has_source_metadata() && definition.name.as_str() == base_name)
    }) else {
        return;
    };

    let parameters = program.data_type_parameters(definition);
    let argument_handles = program
        .type_reference_table
        .type_reference_handles(arguments);
    validate_parameter_bounds(
        program,
        symbols,
        base_name,
        parameters,
        argument_handles,
        type_parameter_scope,
        diagnostics,
    );
}

/// Property admission is independent of whether a telescope belongs to stored
/// data or a trait. Unused/phantom parameters still carry every declared bound.
pub(crate) fn validate_parameter_bounds(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    base_name: &str,
    parameters: &[TypeParameter],
    arguments: &[TypeReferenceHandle],
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let bounds =
            crate::proof_contracts::properties::declared_property_requirements(&parameter.bounds);
        let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
        for property in bounds {
            if crate::proof_contracts::properties::type_satisfies_declared_property(
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
