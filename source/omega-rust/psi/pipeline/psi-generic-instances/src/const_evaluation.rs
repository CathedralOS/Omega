use super::*;

#[derive(Clone, Copy)]
pub(super) enum ConstFactValue {
    Integer(i128),
    Boolean(bool),
}

/// Evaluate a proof expression exactly when every operand is known at generic
/// instantiation time. `None` means the fact still depends on a runtime field
/// and must remain on the synthesized record.
pub(super) fn evaluate_const_fact_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    self_value: Option<i128>,
) -> Result<Option<ConstFactValue>, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(ConstFactValue::Integer)
            .map(Some)
            .ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            }),
        ExpressionNode::Boolean(value) => Ok(Some(ConstFactValue::Boolean(*value))),
        ExpressionNode::Name(path) => {
            let name = syntax
                .expressions
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            Ok(parameter_values
                .get(&name)
                .or_else(|| const_values.get(&name))
                .copied()
                .map(ConstFactValue::Integer))
        }
        ExpressionNode::SelfValue => Ok(self_value.map(ConstFactValue::Integer)),
        ExpressionNode::Binary(binary) => {
            let Some(left) = evaluate_const_fact_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                self_value,
            )?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_fact_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                self_value,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(binary.operator, left, right).map(Some)
        }
        _ => Ok(None),
    }
}

/// Discharge `N in Domain` when `N` is a concrete const parameter and the
/// domain is defined by evaluable boolean facts over `self`. Machine-call facts
/// stay on the concrete record for typed build-time evaluation.
pub(super) fn evaluate_const_membership_fact(
    syntax: &SyntaxTrees,
    membership: &psi_syntax_trees::item::ProofMembershipFact,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    parameter_type_names: &HashMap<String, String>,
) -> Result<Option<bool>, String> {
    let ExpressionNode::Name(value_path) = syntax.expressions.expression(membership.value) else {
        return Ok(None);
    };
    let [parameter_name] = syntax.expressions.identifier_path_members(*value_path) else {
        return Ok(None);
    };
    let Some(value) = parameter_values.get(parameter_name.as_str()).copied() else {
        return Ok(None);
    };
    let Some(parameter_type) = parameter_type_names.get(parameter_name.as_str()) else {
        return Ok(None);
    };
    let domain_path = syntax
        .items
        .identifier_path_members(membership.domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let domain_name = if domain_path.contains("::") {
        domain_path
    } else {
        format!("{parameter_type}::{domain_path}")
    };
    evaluate_named_const_domain(
        syntax,
        &domain_name,
        parameter_type,
        value,
        const_values,
        &mut Vec::new(),
    )
}

pub(super) fn evaluate_named_const_domain(
    syntax: &SyntaxTrees,
    domain_name: &str,
    carrier: &str,
    value: i128,
    const_values: &HashMap<String, i128>,
    visiting: &mut Vec<String>,
) -> Result<Option<bool>, String> {
    if visiting.iter().any(|name| name == domain_name) {
        return Ok(None);
    }
    let Some(domain) = syntax.root_items().find_map(|item| {
        let Item::Domain(domain) = item else {
            return None;
        };
        (domain.name.as_str() == domain_name).then_some(domain)
    }) else {
        return Ok(None);
    };
    let TypeReferenceNode::Named(domain_target) =
        syntax.type_references.type_reference(domain.target_type)
    else {
        return Ok(None);
    };
    if domain_target.as_str() != carrier {
        return Err(format!(
            "domain `{domain_name}` has carrier `{}`, but the const value has carrier `{carrier}`",
            domain_target.as_str(),
        ));
    }
    visiting.push(domain_name.to_owned());
    for fact in syntax.items.proof_facts(domain.facts) {
        let holds = match fact {
            ProofFact::Expression(expression) => evaluate_const_domain_expression(
                syntax,
                *expression,
                const_values,
                value,
                carrier,
                visiting,
            )?,
            ProofFact::Membership(membership) => {
                let Some(ConstFactValue::Integer(nested_value)) = evaluate_const_fact_expression(
                    syntax,
                    membership.value,
                    const_values,
                    &HashMap::new(),
                    Some(value),
                )?
                else {
                    visiting.pop();
                    return Ok(None);
                };
                let path = syntax
                    .items
                    .identifier_path_members(membership.domain)
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                let nested_domain = if path.contains("::") {
                    path
                } else {
                    format!("{carrier}::{path}")
                };
                evaluate_named_const_domain(
                    syntax,
                    &nested_domain,
                    carrier,
                    nested_value,
                    const_values,
                    visiting,
                )?
                .map(ConstFactValue::Boolean)
            }
        };
        let Some(ConstFactValue::Boolean(holds)) = holds else {
            visiting.pop();
            return Ok(None);
        };
        if !holds {
            visiting.pop();
            return Ok(Some(false));
        }
    }
    visiting.pop();
    Ok(Some(true))
}

pub(super) fn evaluate_const_domain_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    self_value: i128,
    carrier: &str,
    visiting: &mut Vec<String>,
) -> Result<Option<ConstFactValue>, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Membership(membership) => {
            let Some(ConstFactValue::Integer(value)) = evaluate_const_fact_expression(
                syntax,
                membership.value,
                const_values,
                &HashMap::new(),
                Some(self_value),
            )?
            else {
                return Ok(None);
            };
            let path = syntax
                .expressions
                .identifier_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let domain_name = if path.contains("::") {
                path
            } else {
                format!("{carrier}::{path}")
            };
            evaluate_named_const_domain(
                syntax,
                &domain_name,
                carrier,
                value,
                const_values,
                visiting,
            )
            .map(|result| result.map(ConstFactValue::Boolean))
        }
        ExpressionNode::Binary(binary) => {
            let Some(left) = evaluate_const_domain_expression(
                syntax,
                binary.left,
                const_values,
                self_value,
                carrier,
                visiting,
            )?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_domain_expression(
                syntax,
                binary.right,
                const_values,
                self_value,
                carrier,
                visiting,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(binary.operator, left, right).map(Some)
        }
        _ => evaluate_const_fact_expression(
            syntax,
            expression,
            const_values,
            &HashMap::new(),
            Some(self_value),
        ),
    }
}

pub(super) fn evaluate_const_fact_binary(
    operator: BinaryOperator,
    left: ConstFactValue,
    right: ConstFactValue,
) -> Result<ConstFactValue, String> {
    use BinaryOperator::*;
    match (left, right) {
        (ConstFactValue::Integer(left), ConstFactValue::Integer(right)) => match operator {
            Add => checked_fact_integer(left.checked_add(right), "addition"),
            Subtract => checked_fact_integer(left.checked_sub(right), "subtraction"),
            Multiply => checked_fact_integer(left.checked_mul(right), "multiplication"),
            Divide => left
                .checked_div(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "division by zero is invalid".to_string()),
            Modulo => left
                .checked_rem(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "remainder by zero is invalid".to_string()),
            ShiftLeft if left >= 0 => u32::try_from(right)
                .ok()
                .filter(|amount| *amount < u64::BITS)
                .and_then(|amount| left.checked_shl(amount))
                .and_then(const_integer_in_envelope)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "left shift exceeds the `u64` width".to_string()),
            ShiftRight if left >= 0 => u32::try_from(right)
                .ok()
                .filter(|amount| *amount < u64::BITS)
                .and_then(|amount| left.checked_shr(amount))
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "right shift exceeds the `u64` width".to_string()),
            BitwiseAnd if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left & right)),
            BitwiseOr if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left | right)),
            BitwiseXor if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left ^ right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            Greater => Ok(ConstFactValue::Boolean(left > right)),
            GreaterOrEqual => Ok(ConstFactValue::Boolean(left >= right)),
            Less => Ok(ConstFactValue::Boolean(left < right)),
            LessOrEqual => Ok(ConstFactValue::Boolean(left <= right)),
            And | Or => Err("logical operators require boolean operands".to_string()),
            ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => Err(
                "signed shifts and bitwise operators require declared-width semantics".to_string(),
            ),
        },
        (ConstFactValue::Boolean(left), ConstFactValue::Boolean(right)) => match operator {
            And => Ok(ConstFactValue::Boolean(left && right)),
            Or => Ok(ConstFactValue::Boolean(left || right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            _ => Err("arithmetic and ordering operators require integer operands".to_string()),
        },
        _ => Err("const fact operands have incompatible types".to_string()),
    }
}

pub(super) fn integer_literal_value(value: &IntegerLiteral) -> Option<i128> {
    value
        .value_i64()
        .map(i128::from)
        .or_else(|| value.value_u64().map(i128::from))
}

pub(super) fn qualified_const_name(definition: &ConstDefinition) -> String {
    if definition.scope.as_str().is_empty() {
        definition.name.as_str().to_owned()
    } else {
        format!(
            "{}::{}",
            definition.scope.as_str(),
            definition.name.as_str()
        )
    }
}

#[derive(Clone)]
pub(super) struct ClosedDomainFamily {
    parameters: Vec<ClosedDomainParameter>,
}

#[derive(Clone)]
enum ClosedDomainParameter {
    Type {
        name: String,
    },
    Const {
        name: String,
        type_reference: TypeReferenceHandle,
    },
}

/// PDI2's closed-index precursor runs beside generic-data canonicalization but
/// does not monomorphize the domain: the family remains nominal and erased.
/// Only its const arguments are rewritten to the same canonical leaves used by
/// PDI1 generic identity.
pub(super) fn canonicalize_closed_domain_indices(
    syntax: &mut SyntaxTrees,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
) -> Result<(), Diagnostic> {
    let mut families = HashMap::<String, ClosedDomainFamily>::new();

    for item in syntax.root_items() {
        let Item::Domain(definition) = item else {
            continue;
        };
        let parameters = syntax
            .tables
            .items
            .type_parameters(definition.type_parameters);
        let header_arguments = syntax
            .tables
            .type_references
            .type_reference_handles(definition.index_arguments);

        if parameters.is_empty() {
            if !header_arguments.is_empty() {
                return Err(Diagnostic::error(format!(
                    "domain `{}` supplies index arguments but declares no generic carrier/const telescope",
                    definition.name
                )));
            }
            continue;
        }

        let TypeReferenceNode::Named(target) = syntax
            .tables
            .type_references
            .type_reference(definition.target_type)
        else {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` must use its carrier binder directly before `::{}`",
                definition.name, definition.name
            )));
        };
        let generic_carrier = parameters.first().is_some_and(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Type)
                && target.as_str() == parameter.name.as_str()
        });
        let index_parameters = if generic_carrier {
            &parameters[1..]
        } else {
            parameters
        };
        if index_parameters.is_empty() {
            // A carrier-polymorphic, unindexed domain (`domain<T> T::D`)
            // needs no closed-family canonicalization. Its exact declaration
            // symbol is the complete nominal identity retained downstream.
            continue;
        }
        let mut family_parameters = Vec::with_capacity(index_parameters.len());
        for parameter in index_parameters {
            match parameter.kind {
                TypeParameterKind::Type => family_parameters.push(ClosedDomainParameter::Type {
                    name: parameter.name.as_str().to_owned(),
                }),
                TypeParameterKind::Const { type_reference } => {
                    validate_const_index_type(syntax, type_reference, &mut HashSet::new())
                        .map_err(|reason| {
                            Diagnostic::error(format!(
                                "indexed domain `{}::{}` has an ineligible index type: {reason}",
                                definition.name, parameter.name
                            ))
                        })?;
                    family_parameters.push(ClosedDomainParameter::Const {
                        name: parameter.name.as_str().to_owned(),
                        type_reference,
                    });
                }
                _ => {
                    return Err(Diagnostic::error(format!(
                        "indexed domain `{}` indices must be type or proof-static const parameters",
                        definition.name
                    )));
                }
            }
        }
        if header_arguments.len() != family_parameters.len() {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` declares {} index parameter(s) but selects {} index argument(s) in its family header",
                definition.name,
                family_parameters.len(),
                header_arguments.len()
            )));
        }
        for (parameter, argument) in family_parameters.iter().zip(header_arguments) {
            let parameter_name = match parameter {
                ClosedDomainParameter::Type { name }
                | ClosedDomainParameter::Const { name, .. } => name,
            };
            let TypeReferenceNode::Named(argument_name) =
                syntax.tables.type_references.type_reference(*argument)
            else {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must select each index binder directly in its family header",
                    definition.name
                )));
            };
            if argument_name.as_str() != parameter_name {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must select index binder `{parameter_name}` in declaration order, not `{argument_name}`",
                    definition.name
                )));
            }
        }
        if families
            .insert(
                definition.name.as_str().to_owned(),
                ClosedDomainFamily {
                    parameters: family_parameters,
                },
            )
            .is_some()
        {
            return Err(Diagnostic::error(format!(
                "indexed domain family `{}` is declared more than once",
                definition.name
            )));
        }
    }

    let mut applications = syntax
        .tables
        .type_references
        .domain_constraints()
        .into_iter()
        .map(|constraint| (constraint.name.as_str().to_owned(), constraint.arguments))
        .collect::<Vec<_>>();
    applications.extend(
        syntax
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                if cast.semantic_domain.is_empty() {
                    return None;
                }
                let name = syntax
                    .expressions
                    .identifier_path_members(cast.semantic_domain)
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                Some((name, cast.semantic_domain_arguments))
            }),
    );

    for (name, argument_span) in applications {
        let Some(family) = families.get(&name) else {
            continue;
        };
        let arguments = syntax
            .tables
            .type_references
            .type_reference_handles(argument_span)
            .to_vec();
        canonicalize_closed_domain_application(
            syntax,
            &name,
            family,
            arguments,
            const_definitions,
            const_values,
        )?;
    }
    Ok(())
}

pub(super) fn canonicalize_closed_domain_application(
    syntax: &mut SyntaxTrees,
    family_name: &str,
    family: &ClosedDomainFamily,
    arguments: Vec<TypeReferenceHandle>,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
) -> Result<(), Diagnostic> {
    if arguments.len() != family.parameters.len() {
        return Err(Diagnostic::error(format!(
            "indexed domain `{}` requires {} closed index argument(s), but {} were supplied",
            family_name,
            family.parameters.len(),
            arguments.len()
        )));
    }
    for (parameter, argument) in family.parameters.iter().zip(arguments) {
        let ClosedDomainParameter::Const {
            name: parameter_name,
            type_reference: parameter_type,
        } = parameter
        else {
            continue;
        };
        let node = syntax
            .tables
            .type_references
            .type_reference(argument)
            .clone();
        match node {
            TypeReferenceNode::Named(name) => {
                if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
                    let required =
                        syntax_type_identity(syntax, *parameter_type).map_err(Diagnostic::error)?;
                    if value.type_name != required {
                        return Err(Diagnostic::error(format!(
                            "index argument for `{}::{parameter_name}` has canonical type `{}`, expected `{required}`",
                            family_name, value.type_name
                        )));
                    }
                    continue;
                }
                if let Some(value) = const_values.get(name.as_str()) {
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                    );
                    continue;
                }
                let Some(definition) = const_definitions.get(name.as_str()) else {
                    // A direct generic const binder is resolved and checked
                    // later in its declaration context. Unknown names fail
                    // there as well; never guess that a type is a value.
                    continue;
                };
                let value = canonicalize_const_definition(syntax, definition, *parameter_type)
                    .map_err(|reason| {
                        Diagnostic::error(format!(
                            "index argument for `{}::{parameter_name}` is invalid: {reason}",
                            family_name
                        ))
                    })?;
                syntax.tables.type_references.replace_type_reference(
                    argument,
                    TypeReferenceNode::Named(Identifier::generated(value.atom())),
                );
            }
            TypeReferenceNode::ConstExpression(expression) => {
                // PDI3 open indexed-domain expressions must survive this
                // pre-resolution pass so binder names and selected operators
                // can acquire exact symbols later. Closed integer arithmetic
                // keeps the existing eager fold and diagnostics.
                if const_expression_contains_name(syntax, expression) {
                    continue;
                }
                let value = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    const_integer_type(syntax, *parameter_type),
                )
                .and_then(EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "index argument expression for `{}` is invalid: {reason}",
                        family_name
                    ))
                })?;
                syntax.tables.type_references.replace_type_reference(
                    argument,
                    TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                );
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}::{parameter_name}` requires a closed const value or direct const binder",
                    family_name
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn const_expression_contains_name(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
) -> bool {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Binary(binary) => {
            const_expression_contains_name(syntax, binary.left)
                || const_expression_contains_name(syntax, binary.right)
        }
        ExpressionNode::Unary(unary) => const_expression_contains_name(syntax, unary.operand),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CanonicalConstNode {
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
    fn encoding(&self) -> String {
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

    fn display(&self) -> String {
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

pub(super) fn framed(tag: &str, pieces: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut encoded = tag.to_owned();
    for piece in pieces {
        let piece = piece.as_ref();
        encoded.push_str(&piece.len().to_string());
        encoded.push(':');
        encoded.push_str(piece);
    }
    encoded
}

pub(super) fn canonicalize_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    parameter_type: TypeReferenceHandle,
) -> Result<CanonicalConstValue, String> {
    let declared = syntax_type_identity(syntax, definition.type_reference)?;
    let required = syntax_type_identity(syntax, parameter_type)?;
    if declared != required {
        return Err(format!(
            "const `{}` declares type `{declared}`, but the parameter requires `{required}`",
            qualified_const_name(definition)
        ));
    }
    validate_const_index_type(syntax, parameter_type, &mut HashSet::new())?;
    let node = canonicalize_const_expression(syntax, parameter_type, definition.value)?;
    if required == "Rat" {
        validate_canonical_rat(&node)?;
    }
    Ok(CanonicalConstValue::new(
        required,
        node.encoding(),
        node.display(),
    ))
}

pub(super) fn syntax_type_identity(
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

pub(super) fn validate_const_index_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<String>,
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
            if !visiting.insert(name.as_str().to_owned()) {
                return Ok(());
            }
            let definition = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Data(definition) if definition.name.as_str() == name.as_str() => {
                        Some(definition)
                    }
                    _ => None,
                })
                .ok_or_else(|| format!("`{name}` is not a declared canonical data type"))?;
            if definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
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
                    DataMember::Field(field) => validate_const_index_type(
                        syntax,
                        field.type_reference,
                        visiting,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in syntax.tables.items.data_payload_fields(variant.payload) {
                            validate_const_index_type(syntax, field.type_reference, visiting)?;
                        }
                    }
                    DataMember::Retired(_) => {}
                }
            }
            visiting.remove(name.as_str());
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => validate_const_index_type(syntax, *element_type, visiting),
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_const_index_type(syntax, *base_type, visiting)
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

pub(super) fn canonicalize_const_expression(
    syntax: &SyntaxTrees,
    expected_type: TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Result<CanonicalConstNode, String> {
    match syntax.tables.type_references.type_reference(expected_type) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            canonicalize_const_expression(syntax, *base_type, expression)
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
            canonicalize_data_const_expression(syntax, type_name.as_str(), expression)
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
                .map(|value| canonicalize_const_expression(syntax, *element_type, *value))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CanonicalConstNode::Array {
                type_name: syntax_type_identity(syntax, expected_type)?,
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

pub(super) fn canonicalize_data_const_expression(
    syntax: &SyntaxTrees,
    type_name: &str,
    expression: ExpressionHandle,
) -> Result<CanonicalConstNode, String> {
    let definition = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == type_name => Some(definition),
            _ => None,
        })
        .ok_or_else(|| format!("`{type_name}` is not a declared data type"))?;
    match syntax.expressions.expression(expression) {
        ExpressionNode::StructLiteral(literal) if literal.type_name.as_str() == type_name => {
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
                let fields = canonicalize_named_fields(syntax, &declared_fields, literal.fields)?;
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
                let fields = canonicalize_named_fields(syntax, &declared_fields, literal.fields)?;
                Ok(CanonicalConstNode::Record {
                    type_name: type_name.to_owned(),
                    fields,
                })
            }
        }
        ExpressionNode::Name(path) => {
            let path = syntax.expressions.identifier_path_members(*path);
            let [head, case_name] = path else {
                return Err(format!("expected a `{type_name}` structural literal"));
            };
            if head.as_str() != type_name {
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

pub(super) fn canonicalize_named_fields(
    syntax: &SyntaxTrees,
    declared_fields: &[&psi_syntax_trees::item::DataField],
    literal_fields: HandleSpan<psi_syntax_trees::expression::TableStructLiteralField>,
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
            canonicalize_const_expression(syntax, declared.type_reference, field.value)?,
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

pub(super) fn validate_syntax_integer_range(type_name: &str, value: i128) -> Result<(), String> {
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

pub(super) fn validate_canonical_rat(value: &CanonicalConstNode) -> Result<(), String> {
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

pub(super) fn nat_value(value: &CanonicalConstNode) -> Result<usize, String> {
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

pub(super) fn gcd_usize(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub(super) fn const_integer_in_envelope(value: i128) -> Option<i128> {
    (value >= i128::from(i64::MIN) && value <= i128::from(u64::MAX)).then_some(value)
}

pub(super) fn checked_fact_integer(
    value: Option<i128>,
    operation: &str,
) -> Result<ConstFactValue, String> {
    value
        .and_then(const_integer_in_envelope)
        .map(ConstFactValue::Integer)
        .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
}

pub(super) fn replace_const_expression_names_from(
    syntax: &mut SyntaxTrees,
    expression_watermark: u32,
    const_literals: &HashMap<String, IntegerLiteral>,
) {
    let replacements = syntax
        .expressions
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() >= expression_watermark)
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Name(path) = expression else {
                return None;
            };
            let [name] = syntax.expressions.identifier_path_members(*path) else {
                return None;
            };
            const_literals
                .get(name.as_str())
                .cloned()
                .map(|literal| (handle, literal))
        })
        .collect::<Vec<_>>();
    for (handle, literal) in replacements {
        syntax
            .expressions
            .replace_expression(handle, ExpressionNode::Integer(literal));
    }
}

/// Generic definitions remain in the tree after their concrete records are
/// synthesized so the normal frontend can validate the template. A symbolic
/// const expression cannot cross that boundary yet, so reduce each template
/// expression to either its concrete value or one declared const-parameter
/// dependency. The concrete clones already carry the fully evaluated value;
/// this placeholder exists only to preserve the established generic type/kind
/// checks on the source template.
pub(super) fn normalize_generic_template_const_expressions(
    syntax: &mut SyntaxTrees,
    const_values: &HashMap<String, i128>,
) -> Result<(), Diagnostic> {
    let templates: Vec<(String, HashSet<String>, Vec<TypeReferenceHandle>)> = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Data(definition) = item else {
                return None;
            };
            if definition.type_parameters.is_empty() {
                return None;
            }
            let symbolic_parameters = syntax
                .tables
                .items
                .type_parameters(definition.type_parameters)
                .iter()
                .filter_map(|parameter| {
                    matches!(parameter.kind, TypeParameterKind::Const { .. })
                        .then(|| parameter.name.as_str().to_string())
                })
                .collect();
            let fields = syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field.type_reference),
                    DataMember::Variant(_) => None,
                    DataMember::Retired(_) => None,
                })
                .collect();
            Some((
                definition.name.as_str().to_string(),
                symbolic_parameters,
                fields,
            ))
        })
        .collect();

    for (template, symbolic_parameters, fields) in templates {
        for field in fields {
            normalize_template_type_reference(
                syntax,
                field,
                const_values,
                &symbolic_parameters,
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "const argument expression in generic data `{template}` is invalid: {reason}"
                ))
            })?;
        }
    }
    Ok(())
}

pub(super) fn normalize_template_type_reference(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    const_values: &HashMap<String, i128>,
    symbolic_parameters: &HashSet<String>,
) -> Result<(), String> {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Reference { referee, .. } => {
            normalize_template_type_reference(syntax, referee, const_values, symbolic_parameters)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            normalize_template_type_reference(syntax, base_type, const_values, symbolic_parameters)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => normalize_template_type_reference(
            syntax,
            element_type,
            const_values,
            symbolic_parameters,
        ),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let arguments = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let integer_types = generic_const_integer_types(syntax, base_name.as_str());
            for (index, argument) in arguments.into_iter().enumerate() {
                let node = syntax
                    .tables
                    .type_references
                    .type_reference(argument)
                    .clone();
                if let TypeReferenceNode::ConstExpression(expression) = node {
                    let placeholder = evaluate_const_argument_expression(
                        syntax,
                        expression,
                        const_values,
                        &HashMap::new(),
                        symbolic_parameters,
                        integer_types.get(index).copied().flatten(),
                    )?;
                    let name = match placeholder {
                        EvaluatedConst::Concrete(value) => value.to_string(),
                        EvaluatedConst::Symbolic(name) => name,
                    };
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(name)),
                    );
                } else {
                    normalize_template_type_reference(
                        syntax,
                        argument,
                        const_values,
                        symbolic_parameters,
                    )?;
                }
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            let placeholder = evaluate_const_argument_expression(
                syntax,
                expression,
                const_values,
                &HashMap::new(),
                symbolic_parameters,
                None,
            )?;
            let name = match placeholder {
                EvaluatedConst::Concrete(value) => value.to_string(),
                EvaluatedConst::Symbolic(name) => name,
            };
            syntax.tables.type_references.replace_type_reference(
                type_reference,
                TypeReferenceNode::Named(Identifier::generated(name)),
            );
            Ok(())
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => Ok(()),
    }
}

/// Every TYPE-REFERENCE position a generic-data spelling can appear in: data
/// FIELDS plus machine-body `let`-local, state PARAMETER, and RETURN types. Run
/// afresh each fixpoint round so newly-synthesized records' fields are seen.
pub(super) fn collect_type_reference_positions(syntax: &SyntaxTrees) -> Vec<TypeReferenceHandle> {
    fn collect(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        positions: &mut Vec<TypeReferenceHandle>,
    ) {
        positions.push(type_reference);
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => collect(syntax, *referee, positions),
            TypeReferenceNode::Constrained { base_type, .. } => {
                collect(syntax, *base_type, positions)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                collect(syntax, *element_type, positions)
            }
            TypeReferenceNode::Generic { arguments, .. } => {
                for argument in syntax
                    .tables
                    .type_references
                    .type_reference_handles(*arguments)
                {
                    collect(syntax, *argument, positions);
                }
            }
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Named(_)
            | TypeReferenceNode::SelfType
            | TypeReferenceNode::Unit => {}
        }
    }

    let mut positions: Vec<TypeReferenceHandle> = Vec::new();
    for item in syntax.root_items() {
        match item {
            // SKIP the bodies of GENERIC TEMPLATES (defs/machines with type
            // parameters): their `Box<T>` fields carry the type PARAMETER as an
            // argument, not a concrete instantiation -- monomorphizing them would
            // synthesize a bogus `Box<T>` record and corrupt the template. Only
            // concrete records (incl. synthesized instances) and non-generic
            // machine bodies hold real `Box<i32>` spellings.
            Item::Data(definition) if definition.type_parameters.is_empty() => {
                for member in syntax.tables.items.data_members(definition.members) {
                    match member {
                        DataMember::Field(field) => {
                            collect(syntax, field.type_reference, &mut positions)
                        }
                        DataMember::Variant(variant) => {
                            for field in syntax.tables.items.data_payload_fields(variant.payload) {
                                collect(syntax, field.type_reference, &mut positions);
                            }
                        }
                        DataMember::Retired(_) => {}
                    }
                }
            }
            Item::Machine(machine) if machine.type_parameters.is_empty() => {
                // Conformance arguments participate in the same concrete
                // generic-data identity as the machine signature. Rewriting
                // `-> Algebra<Unit>` while leaving
                // `satisfies Trait<Algebra<Unit>>` generic makes an otherwise
                // exact requirement mismatch after instance synthesis.
                for conformance in syntax.tables.items.satisfies_clauses(machine.satisfies) {
                    for argument in syntax
                        .tables
                        .type_references
                        .type_reference_handles(conformance.arguments)
                    {
                        collect(syntax, *argument, &mut positions);
                    }
                }
                for state_handle in syntax.tables.items.state_handles(machine.states) {
                    let state = syntax.tables.items.state(*state_handle);
                    collect(syntax, state.return_type, &mut positions);
                    for parameter_handle in syntax.tables.items.state_parameters(state.parameters) {
                        collect(
                            syntax,
                            syntax
                                .tables
                                .items
                                .state_parameter(*parameter_handle)
                                .type_reference,
                            &mut positions,
                        );
                    }
                    for statement_handle in syntax.tables.items.statements(state.statements) {
                        if let StatementNode::LocalData(local) =
                            syntax.tables.statements.statement(*statement_handle)
                        {
                            collect(syntax, local.type_reference, &mut positions);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Cast targets are type-reference owners in concrete machine bodies too.
    // Rewriting the stated local while leaving `as &Pair<u32>` as a raw
    // Generic node gives downstream representation validation two identities
    // for the same synthesized instance. Walk only expressions reachable from
    // non-generic machines; generic template bodies remain deliberately open.
    let concrete_expressions = super::concrete_machine_expression_handles(syntax);
    for (handle, expression) in syntax.expressions.iter_expressions() {
        if concrete_expressions.contains(&handle.arena_index())
            && let ExpressionNode::Cast(cast) = expression
        {
            collect(syntax, cast.target_type, &mut positions);
        }
    }
    positions
}

/// If `type_reference` is a `Base<Args..>` spelling of a fully-monomorphizable
/// generic data definition, record the rewrite-to-plain-name and the (deduped)
/// instantiation. Anything Phase 1 cannot lower completely -- a non-generic base,
/// wrong arity, a non-sluggable argument, or a data shape whose members cannot
/// substitute every parameter occurrence exactly -- is left
/// UNTOUCHED for the existing type-check-only path (skip, never reject).
pub(super) fn consider_generic_spelling(
    syntax: &mut SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
    type_reference: TypeReferenceHandle,
    rewrites: &mut Vec<PendingRewrite>,
    instantiations: &mut Vec<Instantiation>,
) -> Result<(), Diagnostic> {
    let (base_name, lifetime_arguments, arguments) =
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => (base_name.clone(), lifetime_arguments.clone(), *arguments),
            _ => return Ok(()),
        };
    let base = base_name.as_str().to_string();
    let Some(base_info) = generic_data.get(&base) else {
        return Ok(()); // non-generic base: plan-laid / existing error paths
    };

    let argument_handles: Vec<TypeReferenceHandle> = syntax
        .tables
        .type_references
        .type_reference_handles(arguments)
        .to_vec();
    if argument_handles.len() != base_info.parameter_names.len() {
        return Ok(());
    }
    for ((parameter_name, parameter_type), argument) in base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
        .zip(&argument_handles)
    {
        let Some(parameter_type) = *parameter_type else {
            if matches!(
                syntax.tables.type_references.type_reference(*argument),
                TypeReferenceNode::ConstExpression(_)
            ) {
                return Err(Diagnostic::error(format!(
                    "generic argument expression for `{base}` is only valid for a const parameter"
                )));
            }
            continue;
        };
        match syntax
            .tables
            .type_references
            .type_reference(*argument)
            .clone()
        {
            TypeReferenceNode::Named(name) => {
                if CanonicalConstValue::from_atom(name.as_str()).is_some() {
                    continue;
                }
                if matches!(
                    syntax
                        .tables
                        .type_references
                        .type_reference(parameter_type),
                    TypeReferenceNode::Named(type_name) if type_name.as_str() == "bool"
                ) && matches!(name.as_str(), "true" | "false")
                {
                    let value = CanonicalConstValue::boolean(name.as_str() == "true");
                    syntax.tables.type_references.replace_type_reference(
                        *argument,
                        TypeReferenceNode::Named(Identifier::generated(value.atom())),
                    );
                    continue;
                }
                if let Some(value) = const_values.get(name.as_str()) {
                    syntax.tables.type_references.replace_type_reference(
                        *argument,
                        TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                    );
                    continue;
                }
                let Some(definition) = const_definitions.get(name.as_str()) else {
                    continue;
                };
                let value = canonicalize_const_definition(syntax, definition, parameter_type)
                    .map_err(|reason| {
                        Diagnostic::error(format!(
                            "const argument for `{base}::{parameter_name}` is invalid at this index site: {reason}"
                        ))
                    })?;
                syntax.tables.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(value.atom())),
                );
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let value = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    const_integer_type(syntax, parameter_type),
                )
                .and_then(EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "const argument expression for `{base}` is invalid: {reason}"
                    ))
                })?;
                syntax.tables.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                );
            }
            _ => continue,
        }
    }
    let Some(argument_handles) = canonicalize_monomorphizable_argument_handles(
        syntax,
        base_info,
        &lifetime_arguments,
        &argument_handles,
    ) else {
        return Ok(());
    };
    let Some(argument_names) = monomorphizable_argument_slugs(syntax, &argument_handles) else {
        return Ok(());
    };
    if !const_arguments_fit_declarations(syntax, base_info, &argument_handles) {
        // Leave malformed/out-of-range const applications intact so the normal
        // declaration-aware validator emits its precise diagnostic.
        return Ok(());
    }
    if !base_is_fully_monomorphizable(syntax, generic_data, base_info) {
        return Ok(());
    }

    let synthetic_name = format!("{base}<{}>", argument_names.join(", "));
    rewrites.push(PendingRewrite {
        type_reference,
        synthetic_name: synthetic_name.clone(),
        lifetime_arguments,
    });
    if !instantiations
        .iter()
        .any(|instance| instance.synthetic_name == synthetic_name)
    {
        instantiations.push(Instantiation {
            synthetic_name,
            base_name: base,
            argument_handles,
        });
    }
    Ok(())
}

pub(super) fn const_arguments_fit_declarations(
    syntax: &SyntaxTrees,
    base_info: &GenericData,
    arguments: &[TypeReferenceHandle],
) -> bool {
    base_info
        .const_parameter_types
        .iter()
        .zip(arguments)
        .all(|(parameter_type, argument)| {
            let Some(parameter_type) = parameter_type else {
                return true;
            };
            let TypeReferenceNode::Named(value) =
                syntax.tables.type_references.type_reference(*argument)
            else {
                return false;
            };
            let TypeReferenceNode::Named(type_name) = syntax
                .tables
                .type_references
                .type_reference(*parameter_type)
            else {
                return false;
            };
            if let Some(value) = CanonicalConstValue::from_atom(value.as_str()) {
                return value.type_name == type_name.as_str();
            }
            let Ok(value) = value.as_str().parse::<i128>() else {
                return false;
            };
            let (minimum, maximum) = match type_name.as_str() {
                "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
                "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
                "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
                "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
                "u8" => (0, i128::from(u8::MAX)),
                "u16" => (0, i128::from(u16::MAX)),
                "u32" => (0, i128::from(u32::MAX)),
                "u64" | "addr" => (0, i128::from(u64::MAX)),
                _ => return false,
            };
            value >= minimum && value <= maximum
        })
}

/// Evaluate the symbolic integer subset retained in a const-generic argument.
/// Names resolve to literal scoped const declarations collected above.
/// Arithmetic deliberately matches the closed-expression parser fold over the
/// current signed/unsigned 64-bit envelope. Shifts and bitwise operations use
/// the matched const parameter's declared width and signedness.
pub(super) fn evaluate_const_argument_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    symbolic_parameters: &HashSet<String>,
    integer_type: Option<ConstIntegerType>,
) -> Result<EvaluatedConst, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(EvaluatedConst::Concrete)
            .ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            }),
        ExpressionNode::Name(path) => {
            let name = syntax
                .expressions
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let Some(value) = parameter_values
                .get(&name)
                .or_else(|| const_values.get(&name))
            {
                Ok(EvaluatedConst::Concrete(*value))
            } else if symbolic_parameters.contains(&name) {
                Ok(EvaluatedConst::Symbolic(name))
            } else {
                Err(format!("`{name}` is not a scoped integer const"))
            }
        }
        ExpressionNode::Binary(binary) => {
            let left = evaluate_const_argument_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                symbolic_parameters,
                integer_type,
            )?;
            let right = evaluate_const_argument_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                symbolic_parameters,
                integer_type,
            )?;
            match (binary.operator, &right) {
                (BinaryOperator::Divide | BinaryOperator::Modulo, EvaluatedConst::Concrete(0)) => {
                    return Err(match binary.operator {
                        BinaryOperator::Divide => "division by zero is invalid".to_string(),
                        _ => "remainder by zero is invalid".to_string(),
                    });
                }
                (
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight,
                    EvaluatedConst::Concrete(amount),
                ) if *amount < 0 || *amount >= i128::from(u64::BITS) => {
                    return Err(match binary.operator {
                        BinaryOperator::ShiftLeft => {
                            "left shift exceeds the `u64` width".to_string()
                        }
                        _ => "right shift exceeds the `u64` width".to_string(),
                    });
                }
                (
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor,
                    _,
                ) => {}
                _ => {
                    return Err(
                        "only integer arithmetic, shifts, and bitwise operators are supported"
                            .to_string(),
                    );
                }
            }
            let (EvaluatedConst::Concrete(left), EvaluatedConst::Concrete(right)) = (&left, &right)
            else {
                return Ok(left.or_symbolic(right));
            };
            let (left, right) = (*left, *right);
            match binary.operator {
                BinaryOperator::Add => checked_evaluated_const(left.checked_add(right), "addition"),
                BinaryOperator::Subtract => {
                    checked_evaluated_const(left.checked_sub(right), "subtraction")
                }
                BinaryOperator::Multiply => {
                    checked_evaluated_const(left.checked_mul(right), "multiplication")
                }
                BinaryOperator::Divide => left
                    .checked_div(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "division by zero is invalid".to_string()),
                BinaryOperator::Modulo => left
                    .checked_rem(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "remainder by zero is invalid".to_string()),
                BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor => {
                    evaluate_declared_width_operation(binary.operator, left, right, integer_type)
                        .map(EvaluatedConst::Concrete)
                }
                _ => unreachable!("const integer operator was validated above"),
            }
        }
        _ => Err("expression is not a symbolic integer const expression".to_string()),
    }
}

#[derive(Clone, Copy)]
pub(super) struct ConstIntegerType {
    name: &'static str,
    bits: u32,
    signed: bool,
}

pub(super) fn const_integer_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Option<ConstIntegerType> {
    let TypeReferenceNode::Named(name) =
        syntax.tables.type_references.type_reference(type_reference)
    else {
        return None;
    };
    Some(match name.as_str() {
        "i8" => ConstIntegerType {
            name: "i8",
            bits: 8,
            signed: true,
        },
        "i16" => ConstIntegerType {
            name: "i16",
            bits: 16,
            signed: true,
        },
        "i32" => ConstIntegerType {
            name: "i32",
            bits: 32,
            signed: true,
        },
        "i64" => ConstIntegerType {
            name: "i64",
            bits: 64,
            signed: true,
        },
        "u8" => ConstIntegerType {
            name: "u8",
            bits: 8,
            signed: false,
        },
        "u16" => ConstIntegerType {
            name: "u16",
            bits: 16,
            signed: false,
        },
        "u32" => ConstIntegerType {
            name: "u32",
            bits: 32,
            signed: false,
        },
        "u64" => ConstIntegerType {
            name: "u64",
            bits: 64,
            signed: false,
        },
        "addr" => ConstIntegerType {
            name: "addr",
            bits: 64,
            signed: false,
        },
        _ => return None,
    })
}

pub(super) fn generic_const_integer_types(
    syntax: &SyntaxTrees,
    generic_name: &str,
) -> Vec<Option<ConstIntegerType>> {
    syntax
        .root_items()
        .find_map(|item| {
            let Item::Data(definition) = item else {
                return None;
            };
            (definition.name.as_str() == generic_name).then(|| {
                syntax
                    .tables
                    .items
                    .type_parameters(definition.type_parameters)
                    .iter()
                    .map(|parameter| match parameter.kind {
                        TypeParameterKind::Const { type_reference } => {
                            const_integer_type(syntax, type_reference)
                        }
                        _ => None,
                    })
                    .collect()
            })
        })
        .unwrap_or_default()
}

pub(super) fn evaluate_declared_width_operation(
    operator: BinaryOperator,
    left: i128,
    right: i128,
    integer_type: Option<ConstIntegerType>,
) -> Result<i128, String> {
    let Some(integer_type) = integer_type else {
        return Err(
            "shifts and bitwise operators require a declared integer const type".to_string(),
        );
    };
    let modulus = 1i128 << integer_type.bits;
    let maximum = if integer_type.signed {
        (modulus >> 1) - 1
    } else {
        modulus - 1
    };
    let minimum = if integer_type.signed {
        -(modulus >> 1)
    } else {
        0
    };
    if left < minimum || left > maximum {
        return Err(format!(
            "left operand `{left}` is outside the declared `{}` range",
            integer_type.name
        ));
    }

    match operator {
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            let amount = u32::try_from(right)
                .ok()
                .filter(|amount| *amount < integer_type.bits)
                .ok_or_else(|| {
                    format!(
                        "shift count must be non-negative and below the declared `{}` width",
                        integer_type.name
                    )
                })?;
            if operator == BinaryOperator::ShiftRight {
                // `i128 >>` sign-extends negative signed operands. Unsigned
                // operands were range-checked non-negative, for which the same
                // operation is the required logical shift.
                return Ok(left >> amount);
            }
            let shifted = left.checked_shl(amount).ok_or_else(|| {
                format!(
                    "left shift exceeds the declared `{}` range",
                    integer_type.name
                )
            })?;
            if shifted < minimum || shifted > maximum {
                return Err(format!(
                    "left shift exceeds the declared `{}` range",
                    integer_type.name
                ));
            }
            Ok(shifted)
        }
        BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor => {
            if right < minimum || right > maximum {
                return Err(format!(
                    "right operand `{right}` is outside the declared `{}` range",
                    integer_type.name
                ));
            }
            let mask = modulus - 1;
            let left_bits = left & mask;
            let right_bits = right & mask;
            let result_bits = match operator {
                BinaryOperator::BitwiseAnd => left_bits & right_bits,
                BinaryOperator::BitwiseOr => left_bits | right_bits,
                BinaryOperator::BitwiseXor => left_bits ^ right_bits,
                _ => unreachable!(),
            };
            if integer_type.signed && result_bits >= modulus >> 1 {
                Ok(result_bits - modulus)
            } else {
                Ok(result_bits)
            }
        }
        _ => unreachable!("caller provides only shifts and bitwise operators"),
    }
}

#[derive(Debug)]
pub(super) enum EvaluatedConst {
    Concrete(i128),
    Symbolic(String),
}

pub(super) fn checked_evaluated_const(
    value: Option<i128>,
    operation: &str,
) -> Result<EvaluatedConst, String> {
    value
        .and_then(const_integer_in_envelope)
        .map(EvaluatedConst::Concrete)
        .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
}

impl EvaluatedConst {
    pub(super) fn into_concrete(self) -> Result<i128, String> {
        match self {
            Self::Concrete(value) => Ok(value),
            Self::Symbolic(name) => Err(format!(
                "`{name}` is a const parameter that has no binding at this use"
            )),
        }
    }

    fn or_symbolic(self, other: Self) -> Self {
        match self {
            Self::Symbolic(_) => self,
            Self::Concrete(_) => other,
        }
    }
}
