//! Constant evaluation: facts.
use super::super::{
    CanonicalConstValue, ConstDefinition, Diagnostic, ExpressionHandle, ExpressionNode, HashMap,
    HashSet, Item, ProofFact, SyntaxTrees, TypeConstraintNode, TypeParameterKind,
    TypeReferenceHandle, TypeReferenceNode,
};
use arena::HandleSpan;
use syntax_trees::expression::UnaryOperator;

use crate::preparation::generic_data::const_evaluation::validate_anonymous_remainder;
use crate::preparation::generic_data::evaluate_const_fact_binary;
use crate::preparation::generic_data::integer_literal_value;
use crate::preparation::generic_data::{ConstFactValue, ConstScalarValue};

use super::anonymous::{evaluate_anonymous_numeric_expression, has_builtin_const_operator};

/// Evaluate a proof expression exactly when every operand is known at generic
/// instantiation time. `None` means the fact still depends on a runtime field
/// and must remain on the synthesized record. `self_value` is the concrete
/// scalar a selected domain binds to `self` while its facts replay.
/// Integer synthesis callers and mixed domain binders share the same fact
/// operations without copying their environments or encoding Booleans as 0/1.
pub(in crate::preparation::generic_data) fn evaluate_const_fact_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, impl Copy + Into<ConstScalarValue>>,
    self_value: Option<ConstScalarValue>,
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
                return Ok(Some((*value).into().into_fact_value()));
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
        ExpressionNode::SelfValue => Ok(self_value.map(ConstScalarValue::into_fact_value)),
        ExpressionNode::Unary(unary) => match unary.operator {
            UnaryOperator::LogicalNot => Ok(evaluate_const_fact_expression(
                syntax,
                unary.operand,
                const_values,
                parameter_values,
                self_value,
                warnings,
            )?
            .and_then(|value| match value {
                ConstFactValue::Boolean(value) => Some(ConstFactValue::Boolean(!value)),
                _ => None,
            })),
            UnaryOperator::BitwiseNot => Ok(None),
        },
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
        ConstScalarValue::Integer(value),
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
    value: ConstScalarValue,
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
    let domain_key = module_domain_key(syntax, domain);
    evaluate_selected_domain_facts(
        syntax,
        domain,
        domain_key,
        carrier,
        value,
        const_values,
        &HashMap::new(),
        visiting,
        selection,
        warnings,
    )
}

/// The selected declaration's complete logical path — module prefix plus the
/// declared name — so two authored spellings of the same owner share one
/// recursion-guard key.
fn module_domain_key(
    syntax: &SyntaxTrees,
    domain: &syntax_trees::item::DomainDefinition,
) -> String {
    match crate::preparation::generic_data::module_constants::module_path(
        syntax,
        domain.name.source_span().source_id,
    ) {
        Some(module) => format!("{module}::{}", domain.name.as_str()),
        None => domain.name.as_str().to_owned(),
    }
}

/// Replay one already-selected domain declaration's facts against `value`
/// with `self` bound and the application's closed index binders mapped in
/// `parameter_values` (empty for a monomorphic domain). Unindexed and closed
/// indexed membership share this body so both apply the same carrier check,
/// recursion guard, and fact order. Nested indexed memberships reuse the
/// same application evaluator with the enclosing binder values; their own
/// authored spans select declarations in the fact author's source context.
fn evaluate_selected_domain_facts(
    syntax: &SyntaxTrees,
    domain: &syntax_trees::item::DomainDefinition,
    domain_key: String,
    carrier: &str,
    value: ConstScalarValue,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, ConstScalarValue>,
    visiting: &mut Vec<String>,
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<bool>, String> {
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
                    parameter_values,
                    value,
                    carrier,
                    visiting,
                    selection,
                    warnings,
                )?,
                ProofFact::Membership(membership) => {
                    let Some(nested_value) = evaluate_const_fact_expression(
                        syntax,
                        membership.value,
                        const_values,
                        parameter_values,
                        Some(value),
                        warnings,
                    )?
                    else {
                        return Ok(None);
                    };
                    let Some((nested_carrier, nested_value)) =
                        nested_membership_operand(syntax, nested_value, carrier, warnings)?
                    else {
                        return Ok(None);
                    };
                    let members = syntax.items.identifier_path_members(membership.domain);
                    let path = members
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    let reference = members
                        .first()
                        .map(|member| member.source_span())
                        .unwrap_or_default();
                    let holds = if membership.domain_arguments.is_empty() {
                        evaluate_named_const_domain(
                            syntax,
                            &path,
                            nested_carrier,
                            nested_value,
                            const_values,
                            visiting,
                            reference,
                            selection,
                            warnings,
                        )?
                    } else {
                        evaluate_indexed_const_domain(
                            syntax,
                            &path,
                            membership.domain_arguments,
                            nested_carrier,
                            nested_value,
                            const_values,
                            parameter_values,
                            syntax.items.type_parameters(domain.type_parameters),
                            visiting,
                            reference,
                            selection,
                            warnings,
                        )?
                    };
                    holds.map(ConstFactValue::Boolean)
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

/// Bind one evaluated nested-membership operand as the `self` payload the next
/// selected domain replays. A Boolean operand always selects the `bool`
/// carrier; an integer operand keeps the enclosing carrier, which
/// `self`-derived expressions inherit. `Ok(None)` leaves the membership on the
/// checked-record path rather than binding a guessed payload.
fn nested_membership_operand<'a>(
    syntax: &SyntaxTrees,
    value: ConstFactValue,
    carrier: &'a str,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<(&'a str, ConstScalarValue)>, String> {
    match value {
        ConstFactValue::Boolean(value) => Ok(Some(("bool", ConstScalarValue::Boolean(value)))),
        value => Ok(value
            .into_integer(syntax, warnings)?
            .map(|value| (carrier, ConstScalarValue::Integer(value)))),
    }
}

/// Evaluate `value in <authored><args>` — a closed index application of a
/// generic domain family — at declaration site. The authored spelling selects
/// the family under the same module name law `domain` applies to monomorphic
/// domains (a qualified path names its exact owner, module-local precedence
/// ranks same-leaf candidates, and relative spellings stay import-gated);
/// contested, unreachable, or non-generic owners decline. Every index
/// parameter must be an integer or Boolean const binder and every argument
/// must close to that exact carrier against the enclosing scope's bindings; the
/// family's facts then replay through the same evaluator monomorphic domains
/// use, with `self` bound to `value` and each binder mapped to its argument.
/// A carrier-polymorphic target, a non-scalar or non-const index parameter,
/// an argument that stays open (enclosing binders, module-constant spellings,
/// names still owned by resolved selection), and an unprovable fact all
/// decline with `None` so the declaration keeps its fence instead of
/// discharging against a guessed binding.
fn evaluate_indexed_const_domain(
    syntax: &SyntaxTrees,
    authored: &str,
    argument_span: HandleSpan<TypeReferenceHandle>,
    carrier: &str,
    value: ConstScalarValue,
    const_values: &HashMap<String, i128>,
    argument_values: &HashMap<String, ConstScalarValue>,
    argument_parameters: &[syntax_trees::item::TypeParameter],
    visiting: &mut Vec<String>,
    reference: source::SourceSpan,
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<bool>, String> {
    let domain = match selection {
        Some(selection) => selection
            .domain_family(syntax, authored, reference)
            .map(|(_, definition)| definition),
        None => {
            // Source-free canonicalization has no module custody, so the
            // fallback may only select an unmoduled family declared exactly
            // once — the same restriction unindexed discharge applies.
            let expanded = if authored.contains("::") {
                authored.to_owned()
            } else {
                format!("{carrier}::{authored}")
            };
            let mut matches = syntax.root_items().filter_map(|item| match item {
                Item::Domain(domain) if domain.name.as_str() == expanded => Some(domain),
                _ => None,
            });
            match matches.next() {
                Some(domain)
                    if matches.next().is_none()
                        && !domain.type_parameters.is_empty()
                        && crate::preparation::generic_data::module_constants::module_path(
                            syntax,
                            domain.name.source_span().source_id,
                        )
                        .is_none() =>
                {
                    Some(domain)
                }
                _ => None,
            }
        }
    };
    let Some(domain) = domain else {
        return Ok(None);
    };
    let domain_key = module_domain_key(syntax, domain);
    // A carrier-polymorphic family (`domain<T, ...> T::D`) cannot prove
    // membership for a concrete value until its carrier instantiates; the
    // application keeps its authored shape for that stage.
    let TypeReferenceNode::Named(domain_target) =
        syntax.type_references.type_reference(domain.target_type)
    else {
        return Ok(None);
    };
    let parameters = syntax.items.type_parameters(domain.type_parameters);
    if parameters.first().is_some_and(|parameter| {
        matches!(parameter.kind, TypeParameterKind::Type)
            && domain_target.as_str() == parameter.name.as_str()
    }) {
        return Ok(None);
    }
    let Some(index_parameters) =
        crate::preparation::generic_data::domain_index_parameters(syntax, domain)
    else {
        return Ok(None);
    };
    let arguments = syntax.type_references.type_reference_handles(argument_span);
    if index_parameters.is_empty() {
        return Ok(None);
    }
    if arguments.len() != index_parameters.len() {
        return Err(format!(
            "indexed domain `{authored}` requires {} closed index argument(s), but {} were supplied",
            index_parameters.len(),
            arguments.len(),
        ));
    }
    let mut parameter_values = HashMap::new();
    for (parameter, argument) in index_parameters.iter().zip(arguments.iter()) {
        let TypeParameterKind::Const {
            type_reference: parameter_type,
        } = parameter.kind
        else {
            return Ok(None);
        };
        let Some(bound) = evaluate_domain_index_argument(
            syntax,
            authored,
            parameter.name.as_str(),
            parameter_type,
            *argument,
            const_values,
            argument_values,
            argument_parameters,
            selection,
            warnings,
        )?
        else {
            return Ok(None);
        };
        let required =
            crate::preparation::generic_data::syntax_type_identity(syntax, parameter_type)?;
        match bound {
            ConstScalarValue::Integer(bound) => {
                if required == "bool" {
                    return Err(format!(
                        "index argument for `{authored}::{}` is an integer, expected `bool`",
                        parameter.name.as_str(),
                    ));
                }
                crate::preparation::generic_data::validate_syntax_integer_range(&required, bound)?;
            }
            ConstScalarValue::Boolean(_) if required != "bool" => {
                return Err(format!(
                    "index argument for `{authored}::{}` has canonical type `bool`, expected `{required}`",
                    parameter.name.as_str(),
                ));
            }
            ConstScalarValue::Boolean(_) => {}
        }
        parameter_values.insert(parameter.name.as_str().to_owned(), bound);
    }
    evaluate_selected_domain_facts(
        syntax,
        domain,
        domain_key,
        carrier,
        value,
        const_values,
        &parameter_values,
        visiting,
        selection,
        warnings,
    )
}

/// Evaluate one supplied index argument of a domain-family application to the
/// scalar its const binder receives, under the same admissible forms the
/// closed-index canonicalizer accepts: literal and canonical leaves, the
/// caller's bound index parameters, exactly selected constant declarations,
/// and the scoped integer ledger. `Ok(None)` means the argument cannot be
/// decided at this point — an unresolved name, a spelling ordinary resolution
/// still owns, or a non-scalar canonical value — so the application stays
/// fenced rather than binding a guessed value.
fn evaluate_domain_index_argument(
    syntax: &SyntaxTrees,
    family_name: &str,
    parameter_name: &str,
    parameter_type: TypeReferenceHandle,
    argument: TypeReferenceHandle,
    const_values: &HashMap<String, i128>,
    argument_values: &HashMap<String, ConstScalarValue>,
    argument_parameters: &[syntax_trees::item::TypeParameter],
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstScalarValue>, String> {
    let integer_type = crate::preparation::generic_data::const_integer_type(syntax, parameter_type);
    let boolean_type = matches!(
        syntax.type_references.type_reference(parameter_type),
        TypeReferenceNode::Named(name) if name.as_str() == "bool"
    );
    // A constrained binder cannot borrow its base carrier's eligibility:
    // its qualification still needs separate evidence.
    if integer_type.is_none() && !boolean_type {
        return Ok(None);
    }
    match syntax.type_references.type_reference(argument) {
        TypeReferenceNode::Named(name) => {
            if let Some(atom) = CanonicalConstValue::from_atom(name.as_str()) {
                let required =
                    crate::preparation::generic_data::syntax_type_identity(syntax, parameter_type)?;
                if atom.type_name != required {
                    return Err(format!(
                        "index argument for `{family_name}::{parameter_name}` has canonical type `{}`, expected `{required}`",
                        atom.type_name,
                    ));
                }
                return Ok(ConstScalarValue::from_canonical(&atom));
            }
            if let Ok(value) = name.as_str().parse::<i128>() {
                return Ok(Some(ConstScalarValue::Integer(value)));
            }
            if matches!(name.as_str(), "true" | "false") {
                return Ok(Some(ConstScalarValue::Boolean(name.as_str() == "true")));
            }
            if let Some(value) = argument_values.get(name.as_str()) {
                // Forwarding binds a declared value, not an anonymous integer.
                // Retain the caller telescope until its carrier is checked;
                // reducing to i128 first would let `u64` silently become `u8`.
                let Some(syntax_trees::item::TypeParameter {
                    kind: TypeParameterKind::Const { type_reference },
                    ..
                }) = argument_parameters
                    .iter()
                    .find(|parameter| parameter.name.as_str() == name.as_str())
                else {
                    return Ok(None);
                };
                let actual = crate::preparation::generic_data::syntax_type_identity(
                    syntax,
                    *type_reference,
                )?;
                let required =
                    crate::preparation::generic_data::syntax_type_identity(syntax, parameter_type)?;
                if actual != required {
                    return Err(format!(
                        "index argument for `{family_name}::{parameter_name}` has carrier `{actual}`, expected `{required}`"
                    ));
                }
                return Ok(Some(*value));
            }
            if let Some(selection) = selection {
                // The authored name selects its exact constant declaration
                // under module name law — module-local constants, narrow
                // imports, and qualified spellings all resolve here.
                // Unresolved spellings stay authored for ordinary resolution
                // rather than borrowing a lexical guess.
                if let Some(definition) = selection
                    .select(syntax, name)
                    .map_err(|diagnostic| diagnostic.message)?
                {
                    let value = crate::preparation::generic_data::canonicalize_selected_const_definition(
                        syntax,
                        &definition,
                        parameter_type,
                        Some(selection),
                    )
                    .map_err(|reason| {
                        format!(
                            "index argument for `{family_name}::{parameter_name}` is invalid: {reason}"
                        )
                    })?;
                    return Ok(ConstScalarValue::from_canonical(&value));
                }
            }
            if let Some(value) = const_values.get(name.as_str()) {
                crate::preparation::generic_data::module_constants::reject_module_constant_selection(
                    syntax,
                    name.as_str(),
                    name.source_span(),
                )?;
                return Ok(Some(ConstScalarValue::Integer(*value)));
            }
            // Without source-aware selection, a non-literal scoped const
            // still evaluates through its own declaration. Module constants
            // stay out of the ledger: their names require resolved
            // declaration selection, which the guard enforces on any spelling
            // that could reach one.
            let mut definitions = syntax.root_items().filter_map(|item| match item {
                Item::Const(definition)
                    if !crate::preparation::generic_data::module_constants::is_module_constant(
                        syntax, definition,
                    ) && super::qualified_const_name(definition) == name.as_str() =>
                {
                    Some(definition)
                }
                _ => None,
            });
            let Some(definition) = definitions.next() else {
                return Ok(None);
            };
            if definitions.next().is_some() {
                return Ok(None);
            }
            crate::preparation::generic_data::module_constants::reject_module_constant_selection(
                syntax,
                name.as_str(),
                name.source_span(),
            )?;
            let value = crate::preparation::generic_data::canonicalize_const_definition(
                syntax,
                definition,
                parameter_type,
            )
            .map_err(|reason| {
                format!("index argument for `{family_name}::{parameter_name}` is invalid: {reason}")
            })?;
            Ok(ConstScalarValue::from_canonical(&value))
        }
        TypeReferenceNode::ConstExpression(expression) => {
            // Open arguments (enclosing binders, names awaiting resolved
            // selection, or operators needing declaration authority) keep
            // their authored shape for the downstream pass.
            if crate::preparation::generic_data::const_expression_contains_name(syntax, *expression)
                || super::anonymous::requires_const_operator_selection(syntax, *expression)
            {
                return Ok(None);
            }
            if boolean_type {
                // The typed index probe must establish operand carriers and
                // selected operations before producing a Boolean atom. Fact
                // evaluation alone cannot certify an authored comparison.
                return Ok(None);
            }
            let value = crate::preparation::generic_data::evaluate_const_argument_expression(
                syntax,
                *expression,
                const_values,
                &HashMap::new(),
                &HashSet::new(),
                integer_type,
                warnings,
            )
            .and_then(crate::preparation::generic_data::EvaluatedConst::into_concrete)
            .map_err(|reason| {
                format!("index argument expression for `{family_name}` is invalid: {reason}")
            })?;
            let required =
                crate::preparation::generic_data::syntax_type_identity(syntax, parameter_type)?;
            crate::preparation::generic_data::validate_syntax_integer_range(&required, value)?;
            Ok(Some(ConstScalarValue::Integer(value)))
        }
        _ => Ok(None),
    }
}

/// The shared fence for constrained const declarations whose carrier's
/// constraints cannot be proved at declaration site.
const CONSTRAINED_CONST_FENCE: &str = "constrained const declarations require declaration-site proof checking before they can publish compatibility identity";

/// Discharge a constrained const declaration's domain constraints at its
/// canonical value — the declaration-site proof a constrained carrier owes
/// before it may publish compatibility identity. Every constraint must be a
/// `Domain` application whose authored spelling selects one exact owner under
/// module name law (the same selection the resolver later applies to the
/// declaration's retained type); its facts then replay with `self` bound to
/// the value through the same evaluator `where`-membership discharge uses. A
/// closed index application additionally binds the family's index binders to
/// its evaluated arguments before that replay. Integer and Boolean canonical
/// values bind `self` directly; non-domain constraints, aggregate values,
/// contested or unreachable owners, open arguments, and unprovable facts keep
/// the declaration fenced rather than publishing identity against a guessed
/// or absent owner.
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
    let (carrier, self_value) = match value.decode_encoding() {
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer {
            type_name,
            value,
        }) => (type_name, ConstScalarValue::Integer(value)),
        // The `bool` carrier is the only other scalar canonical encoding; its
        // domain target check and `self` binding are exactly the integer
        // route's.
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Boolean(value)) => {
            ("bool".to_owned(), ConstScalarValue::Boolean(value))
        }
        _ => return Err(CONSTRAINED_CONST_FENCE.to_owned()),
    };
    let const_values =
        crate::preparation::generic_data::module_constants::lexical_integer_const_values(syntax);
    let mut warnings = Vec::new();
    for constraint in constraints {
        let TypeConstraintNode::Domain(domain) = constraint else {
            return Err(CONSTRAINED_CONST_FENCE.to_owned());
        };
        let holds = if domain.arguments.is_empty() {
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
            evaluate_named_const_domain(
                syntax,
                domain.name.as_str(),
                &carrier,
                self_value,
                &const_values,
                &mut Vec::new(),
                domain.name.source_span(),
                selection,
                &mut warnings,
            )?
        } else {
            evaluate_indexed_const_domain(
                syntax,
                domain.name.as_str(),
                domain.arguments,
                &carrier,
                self_value,
                &const_values,
                &HashMap::new(),
                &[],
                &mut Vec::new(),
                domain.name.source_span(),
                selection,
                &mut warnings,
            )?
        };
        match holds {
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

/// Evaluate one fact expression inside a selected domain declaration with
/// `self` bound to the checked value. `parameter_values` carries the
/// application's closed index bindings so a family fact like `self < N`
/// resolves its binder exactly; a monomorphic domain passes an empty map.
pub(in crate::preparation::generic_data) fn evaluate_const_domain_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, ConstScalarValue>,
    self_value: ConstScalarValue,
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
                parameter_values,
                Some(self_value),
                warnings,
            )?
            else {
                return Ok(None);
            };
            let Some((nested_carrier, value)) =
                nested_membership_operand(syntax, value, carrier, warnings)?
            else {
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
                nested_carrier,
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
                parameter_values,
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
                parameter_values,
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
            parameter_values,
            Some(self_value),
            warnings,
        ),
    })();
    if !matches!(result, Ok(Some(_))) {
        warnings.truncate(warning_start);
    }
    result
}
