//! Constant evaluation: facts.
use super::super::{
    CanonicalConstValue, ConstDefinition, Diagnostic, ExpressionHandle, ExpressionNode, HashMap,
    Item, ProofFact, SyntaxTrees, TypeConstraintNode, TypeReferenceNode,
};

use crate::preparation::generic_data::ConstFactValue;
use crate::preparation::generic_data::const_evaluation::validate_anonymous_remainder;
use crate::preparation::generic_data::evaluate_const_fact_binary;
use crate::preparation::generic_data::integer_literal_value;

use super::anonymous::{evaluate_anonymous_numeric_expression, has_builtin_const_operator};

/// Evaluate a proof expression exactly when every operand is known at generic
/// instantiation time. `None` means the fact still depends on a runtime field
/// and must remain on the synthesized record.
pub(in crate::preparation::generic_data) fn evaluate_const_fact_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    self_value: Option<i128>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    if let Some(value) = evaluate_anonymous_numeric_expression(syntax, expression)? {
        return Ok(Some(ConstFactValue::Anonymous(value)));
    }
    let warning_start = warnings.len();
    let result = (|| match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(ConstFactValue::Integer)
            .map(Some)
            .ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            }),
        ExpressionNode::Boolean(value) => Ok(Some(ConstFactValue::Boolean(*value))),
        ExpressionNode::Name(path) => {
            let members = syntax.expressions.identifier_path_members(*path);
            let name = members
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let Some(value) = parameter_values.get(&name) {
                return Ok(Some(ConstFactValue::Integer(*value)));
            }
            crate::preparation::generic_data::module_constants::reject_module_constant_selection(
                syntax,
                &name,
                members
                    .first()
                    .map(|member| member.source_span())
                    .unwrap_or_default(),
            )?;
            Ok(const_values
                .get(&name)
                .copied()
                .map(ConstFactValue::Integer))
        }
        ExpressionNode::SelfValue => Ok(self_value.map(ConstFactValue::Integer)),
        ExpressionNode::Binary(binary) => {
            if !has_builtin_const_operator(syntax, binary.operator) {
                return Ok(None);
            }
            validate_anonymous_remainder(syntax, binary)?;
            let Some(left) = evaluate_const_fact_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                self_value,
                warnings,
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
                warnings,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(syntax, expression, binary.operator, left, right, warnings)
                .map(Some)
        }
        _ => Ok(None),
    })();
    if !matches!(result, Ok(Some(_))) {
        warnings.truncate(warning_start);
    }
    result
}

/// Discharge `N in Domain` when `N` is a concrete const parameter and the
/// domain is defined by evaluable boolean facts over `self`. Machine-call facts
/// stay on the concrete record for typed build-time evaluation.
pub(in crate::preparation::generic_data) fn evaluate_const_membership_fact(
    syntax: &SyntaxTrees,
    membership: &syntax_trees::item::ProofMembershipFact,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    parameter_type_names: &HashMap<String, String>,
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<bool>, String> {
    // An indexed application names one instance of a family; the family's
    // declaration facts alone cannot discharge it, so it stays on the record
    // for typed interning and checked membership evidence.
    if !membership.domain_arguments.is_empty() {
        return Ok(None);
    }
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
    let members = syntax.items.identifier_path_members(membership.domain);
    let domain_path = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    evaluate_named_const_domain(
        syntax,
        &domain_path,
        parameter_type,
        value,
        const_values,
        &mut Vec::new(),
        members
            .first()
            .map(|member| member.source_span())
            .unwrap_or_default(),
        selection,
        warnings,
    )
}

/// Evaluate `value in <authored>` against one declared domain. With a constant
/// selection the authored path selects its exact owner under module name law —
/// the same selection full resolution later assigns the retained fact — so
/// same-spelled domains in sibling modules no longer collide and an ambiguous
/// or unauthorized occurrence declines instead of guessing. Without a
/// selection (header-free probes) a leaf expands against the value carrier
/// exactly as the original name-only lookup did.
pub(in crate::preparation::generic_data) fn evaluate_named_const_domain(
    syntax: &SyntaxTrees,
    authored: &str,
    carrier: &str,
    value: i128,
    const_values: &HashMap<String, i128>,
    visiting: &mut Vec<String>,
    reference: source::SourceSpan,
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<bool>, String> {
    let domain = match selection {
        Some(selection) => selection.domain(syntax, authored, reference),
        None => {
            let expanded = if authored.contains("::") {
                authored.to_owned()
            } else {
                format!("{carrier}::{authored}")
            };
            syntax.root_items().find_map(|item| {
                let Item::Domain(domain) = item else {
                    return None;
                };
                (domain.name.as_str() == expanded).then_some(domain)
            })
        }
    };
    let Some(domain) = domain else {
        return Ok(None);
    };
    // The visiting key is the selected declaration's complete logical path so
    // two authored spellings of the same owner still catch recursion.
    let domain_key = match crate::preparation::generic_data::module_constants::module_path(
        syntax,
        domain.name.source_span().source_id,
    ) {
        Some(module) => format!("{module}::{}", domain.name.as_str()),
        None => domain.name.as_str().to_owned(),
    };
    if visiting.iter().any(|name| name == &domain_key) {
        return Ok(None);
    }
    let TypeReferenceNode::Named(domain_target) =
        syntax.type_references.type_reference(domain.target_type)
    else {
        return Ok(None);
    };
    if domain_target.as_str() != carrier {
        return Err(format!(
            "domain `{domain_key}` has carrier `{}`, but the const value has carrier `{carrier}`",
            domain_target.as_str(),
        ));
    }
    let warning_start = warnings.len();
    visiting.push(domain_key);
    let result = (|| {
        for fact in syntax.items.proof_facts(domain.facts) {
            let holds = match fact {
                ProofFact::Expression(expression) => evaluate_const_domain_expression(
                    syntax,
                    *expression,
                    const_values,
                    value,
                    carrier,
                    visiting,
                    selection,
                    warnings,
                )?,
                ProofFact::Membership(membership) => {
                    if !membership.domain_arguments.is_empty() {
                        return Ok(None);
                    }
                    let Some(nested_value) = evaluate_const_fact_expression(
                        syntax,
                        membership.value,
                        const_values,
                        &HashMap::new(),
                        Some(value),
                        warnings,
                    )?
                    else {
                        return Ok(None);
                    };
                    let Some(nested_value) = nested_value.into_integer(syntax, warnings)? else {
                        return Ok(None);
                    };
                    let members = syntax.items.identifier_path_members(membership.domain);
                    let path = members
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    evaluate_named_const_domain(
                        syntax,
                        &path,
                        carrier,
                        nested_value,
                        const_values,
                        visiting,
                        members
                            .first()
                            .map(|member| member.source_span())
                            .unwrap_or_default(),
                        selection,
                        warnings,
                    )?
                    .map(ConstFactValue::Boolean)
                }
            };
            let Some(ConstFactValue::Boolean(holds)) = holds else {
                return Ok(None);
            };
            if !holds {
                return Ok(Some(false));
            }
        }
        Ok(Some(true))
    })();
    visiting.pop();
    if !matches!(result, Ok(Some(_))) {
        warnings.truncate(warning_start);
    }
    result
}

/// The shared fence for constrained const declarations whose carrier's
/// constraints cannot be proved at declaration site.
const CONSTRAINED_CONST_FENCE: &str = "constrained const declarations require declaration-site proof checking before they can publish compatibility identity";

/// Discharge a constrained const declaration's domain constraints at its
/// canonical value — the declaration-site proof a constrained carrier owes
/// before it may publish compatibility identity. Every constraint must be an
/// unindexed `Domain` application whose authored spelling selects one exact
/// owner under module name law (the same selection the resolver later applies
/// to the declaration's retained type); its facts then replay with `self`
/// bound to the value through the same evaluator `where`-membership discharge
/// uses. Indexed applications, non-domain constraints, non-integer values,
/// and contested or unreachable owners keep the declaration fenced rather
/// than publishing identity against a guessed or absent owner.
///
/// Without a `ConstantSelection` (source-free canonicalization) the evaluator
/// falls back to exact declared-name matching, which cannot see module
/// ownership. That fallback may only select an unmoduled domain declared
/// exactly once — a module-owned or duplicated leaf match stays fenced rather
/// than borrowing an owner the name alone cannot establish.
pub(in crate::preparation::generic_data) fn prove_declared_const_domain_constraints(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    constraints: &[TypeConstraintNode],
    value: &CanonicalConstValue,
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
) -> Result<(), String> {
    let Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer {
        type_name: carrier,
        value: integer,
    }) = value.decode_encoding()
    else {
        return Err(CONSTRAINED_CONST_FENCE.to_owned());
    };
    let const_values =
        crate::preparation::generic_data::module_constants::lexical_integer_const_values(syntax);
    let mut warnings = Vec::new();
    for constraint in constraints {
        let TypeConstraintNode::Domain(domain) = constraint else {
            return Err(CONSTRAINED_CONST_FENCE.to_owned());
        };
        if !domain.arguments.is_empty() {
            // A closed index application on a domain family still owes its
            // open-template membership proof; keep the declaration fenced.
            return Err(CONSTRAINED_CONST_FENCE.to_owned());
        }
        if selection.is_none() {
            let expanded = if domain.name.as_str().contains("::") {
                domain.name.as_str().to_owned()
            } else {
                format!("{carrier}::{}", domain.name.as_str())
            };
            let mut matches = syntax.root_items().filter_map(|item| match item {
                Item::Domain(domain) if domain.name.as_str() == expanded => Some(domain),
                _ => None,
            });
            let unmoduled_unique = matches.next().is_some_and(|domain| {
                matches.next().is_none()
                    && crate::preparation::generic_data::module_constants::module_path(
                        syntax,
                        domain.name.source_span().source_id,
                    )
                    .is_none()
            });
            if !unmoduled_unique {
                return Err(CONSTRAINED_CONST_FENCE.to_owned());
            }
        }
        match evaluate_named_const_domain(
            syntax,
            domain.name.as_str(),
            &carrier,
            integer,
            &const_values,
            &mut Vec::new(),
            domain.name.source_span(),
            selection,
            &mut warnings,
        )? {
            Some(true) => {}
            Some(false) => {
                return Err(format!(
                    "domain constraint `{}` for const `{}` is false",
                    domain.name.as_str(),
                    super::qualified_const_name(definition),
                ));
            }
            None => return Err(CONSTRAINED_CONST_FENCE.to_owned()),
        }
    }
    Ok(())
}

pub(in crate::preparation::generic_data) fn evaluate_const_domain_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    self_value: i128,
    carrier: &str,
    visiting: &mut Vec<String>,
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    if let Some(value) = evaluate_anonymous_numeric_expression(syntax, expression)? {
        return Ok(Some(ConstFactValue::Anonymous(value)));
    }
    let warning_start = warnings.len();
    let result = (|| match syntax.expressions.expression(expression) {
        ExpressionNode::Membership(membership) => {
            let Some(value) = evaluate_const_fact_expression(
                syntax,
                membership.value,
                const_values,
                &HashMap::new(),
                Some(self_value),
                warnings,
            )?
            else {
                return Ok(None);
            };
            let Some(value) = value.into_integer(syntax, warnings)? else {
                return Ok(None);
            };
            let members = syntax
                .expressions
                .identifier_path_members(membership.domain);
            let path = members
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            evaluate_named_const_domain(
                syntax,
                &path,
                carrier,
                value,
                const_values,
                visiting,
                members
                    .first()
                    .map(|member| member.source_span())
                    .unwrap_or_default(),
                selection,
                warnings,
            )
            .map(|result| result.map(ConstFactValue::Boolean))
        }
        ExpressionNode::Binary(binary) => {
            if !has_builtin_const_operator(syntax, binary.operator) {
                return Ok(None);
            }
            validate_anonymous_remainder(syntax, binary)?;
            let Some(left) = evaluate_const_domain_expression(
                syntax,
                binary.left,
                const_values,
                self_value,
                carrier,
                visiting,
                selection,
                warnings,
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
                selection,
                warnings,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(syntax, expression, binary.operator, left, right, warnings)
                .map(Some)
        }
        _ => evaluate_const_fact_expression(
            syntax,
            expression,
            const_values,
            &HashMap::new(),
            Some(self_value),
            warnings,
        ),
    })();
    if !matches!(result, Ok(Some(_))) {
        warnings.truncate(warning_start);
    }
    result
}
