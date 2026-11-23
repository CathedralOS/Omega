use super::BigInt;
/*
    Declaration-site domain discharge shares the closed fact evaluator used by
    generic data. Initializer probes determine values, not membership: ordinary
    constant validation still relies on this gate before publishing identity.
    Scalar replay carries declaration selection through every recursive operand;
    lexical subjects and binders precede selected constants, whose canonical
    values retain their carrier and policy before arithmetic.
    Exact declaration selection, complete index binding and establishment
    obligations therefore precede fact replay. A generic carrier does not supply
    a selected algebra for the subject; only carrier-independent facts can close here.
*/
use super::super::{
    Diagnostic, ExpressionHandle, ExpressionNode, HashMap, HashSet, Item, ProofFact, SyntaxTrees,
    TypeConstraintNode, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
};
use crate::resolution::tables::syntax_expressions;
use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;
use arena::HandleSpan;
use syntax_trees::expression::UnaryOperator;
use syntax_trees::identifier::Identifier;
use syntax_trees::identifier::JoinedSyntaxPath;

use crate::resolution::declarations::generic_data::const_evaluation::validate_anonymous_remainder;
use crate::resolution::declarations::generic_data::evaluate_const_fact_binary;
use crate::resolution::declarations::generic_data::integer_literal_value;
use crate::resolution::declarations::generic_data::{
    ConstFactValue, ConstScalarSpelling, ConstScalarValue, domain_index_parameters,
    syntax_type_identity,
};

use super::anonymous::{
    authored_operator_can_bind, evaluate_anonymous_numeric_expression, has_builtin_const_operator,
};
use super::arguments::validate_declared_integer;
use super::values::CanonicalConstNode;
use super::values::canonicalize_selected_declaration_value;
use crate::resolution::declarations::generic_data::source_modules::module_path;
use language_core::Multiplicity;
use numeric_literals::LandedIntegerType;
use syntax_trees::item::DataMember;
use syntax_trees::item::DomainDefinition;

/*
    Evaluate a proof expression exactly when every operand is known at generic
    instantiation time. `None` means the fact still depends on a runtime field
    and must remain on the synthesized record.
    Integer synthesis callers and mixed domain binders share the same fact
    operations without copying their environments or encoding Booleans as 0/1.
*/
pub(crate) fn evaluate_const_fact_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    parameter_values: &HashMap<String, impl Clone + Into<ConstScalarValue>>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    evaluate_const_fact_expression_at(
        syntax,
        expression,
        parameter_values,
        selection,
        None,
        warnings,
    )
}

/*
    Replay facts with the selected domain's lexical subject, if any. Aggregate
    subjects expose scalar-decodable fields or elements, not whole-value scalars.
*/
fn evaluate_const_fact_expression_at(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    parameter_values: &HashMap<String, impl Clone + Into<ConstScalarValue>>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    subject: Option<ConstFactSubject<'_>>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    macro_rules! evaluate_child {
        ($expression:expr $(,)?) => {
            evaluate_const_fact_expression_at(
                syntax,
                $expression,
                parameter_values,
                selection,
                subject,
                warnings,
            )
        };
    }
    if let Some(ConstFactSubject::Fields(fields)) = subject
        && let Some(value) = const_field_projection(syntax, expression, fields, &mut |index| {
            evaluate_child!(index)
        })?
    {
        return Ok(value
            .and_then(node_subject_leaf)
            .map(ConstScalarValue::into_fact_value));
    }
    if let Some(value) = evaluate_anonymous_numeric_expression(syntax, expression)? {
        return Ok(Some(ConstFactValue::Anonymous(value)));
    }
    let warning_start = warnings.len();
    let result = (|| match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(literal) => {
            let value = integer_literal_value(literal)
                .ok_or_else(|| "integer operand has no exact const value".to_string())?;
            let Some(landing) = literal.landing() else {
                return Ok(Some(ConstFactValue::Integer(value)));
            };
            if landing.domain != numerics::arithmetic::ArithmeticDomain::Exact {
                return Ok(None);
            }
            let integer_type = landing.landed_type;
            validate_declared_integer(integer_type, &value)?;
            Ok(Some(ConstFactValue::DeclaredInteger {
                value,
                integer_type,
            }))
        }
        ExpressionNode::Boolean(value) => Ok(Some(ConstFactValue::Boolean(*value))),
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => {
            let members = syntax.expressions.identifier_path_members(path.to_owned());
            let name = members.joined_path_in(syntax);
            if let Some(subject) = subject.filter(|subject| subject.matches(syntax, expression)) {
                return Ok(subject.scalar().map(ConstScalarValue::into_fact_value));
            }
            if let Some(value) = parameter_values.get(&name) {
                return Ok(Some(value.clone().into().into_fact_value()));
            }
            if let Some(selection) = selection {
                let Some(first) = members.first() else {
                    return Ok(None);
                };
                let mut span = first.source_span();
                span.span.end = members
                    .last()
                    .expect("name has a first member")
                    .source_span()
                    .span
                    .end;
                let identifier = Identifier::new(&name, span);
                let Some(definition) = selection
                    .select(syntax, &identifier)
                    .map_err(|diagnostic| diagnostic.message)?
                else {
                    return Ok(None);
                };
                let Ok(value) =
                    canonicalize_selected_declaration_value(syntax, &definition, Some(selection))
                else {
                    return Ok(None);
                };
                return Ok(
                    ConstScalarValue::from_canonical(&value).map(ConstScalarValue::into_fact_value)
                );
            }
            Ok(None)
        }
        ExpressionNode::Member(member) => {
            let Some(fields) = subject.and_then(ConstFactSubject::fields) else {
                return Ok(None);
            };
            if member.case_variant.is_some()
                || !subject.is_some_and(|subject| subject.matches(syntax, member.receiver))
            {
                return Ok(None);
            }
            Ok(fields
                .iter()
                .find(|(name, _)| name.as_str() == member.member.as_str(syntax))
                .map(|(_, value)| value.clone().into_fact_value()))
        }
        ExpressionNode::Indexed(indexed) => {
            let Some(ConstSubjectBinding::Elements(elements)) =
                subject.and_then(ConstFactSubject::binding)
            else {
                return Ok(None);
            };
            if !subject.is_some_and(|subject| subject.matches(syntax, indexed.collection)) {
                return Ok(None);
            }
            let Some(index_value) = evaluate_child!(indexed.index)? else {
                return Ok(None);
            };
            let Some(index) = index_value.into_integer(syntax, warnings)? else {
                return Ok(None);
            };
            let Ok(index) = usize::try_from(index) else {
                return Ok(None);
            };
            Ok(elements
                .get(index)
                .cloned()
                .flatten()
                .map(|value| value.into_fact_value()))
        }
        ExpressionNode::Unary(unary) => {
            match unary.operator {
                UnaryOperator::LogicalNot => Ok(evaluate_child!(unary.operand)?.and_then(
                    |value| match value {
                        ConstFactValue::Boolean(value) => Some(ConstFactValue::Boolean(!value)),
                        _ => None,
                    },
                )),
                UnaryOperator::BitwiseNot | UnaryOperator::Dereference => Ok(None),
            }
        }
        ExpressionNode::Binary(binary) => {
            if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
                return Ok(None);
            }
            let builtin = has_builtin_const_operator(syntax, binary);
            if !builtin
                && matches!(
                    subject.and_then(ConstFactSubject::scalar),
                    Some(ConstScalarValue::Integer(_))
                )
            {
                return Ok(None);
            }
            validate_anonymous_remainder(syntax, binary)?;
            let Some(left) = evaluate_child!(binary.left)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_child!(binary.right)? else {
                return Ok(None);
            };
            if !builtin
                && authored_operator_can_bind(syntax, binary.operator, &left, &right, selection)
            {
                return Ok(None);
            }
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

/*
    Discharge `N in Domain` when `N` is a concrete const parameter and the
    domain is defined by evaluable boolean facts over the subject. Machine-call facts
    stay on the concrete record for typed build-time evaluation.
*/
pub(crate) fn evaluate_const_membership_fact(
    syntax: &SyntaxTrees,
    membership: &syntax_trees::item::ProofMembershipFact,
    parameter_values: &HashMap<String, BigInt>,
    parameter_types: &HashMap<String, LandedIntegerType>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<bool>, String> {
    if !membership.domain_arguments.is_empty() {
        return Ok(None);
    }
    let ExpressionNode::Name {
        members: value_path,
        application: value_path_application,
    } = syntax.expressions.expression(membership.value)
    else {
        return Ok(None);
    };
    if value_path_application.is_some() {
        return Ok(None);
    }
    let [parameter_name] = syntax
        .expressions
        .identifier_path_members(value_path.to_owned())
    else {
        return Ok(None);
    };
    let Some(value) = parameter_values.get(parameter_name.as_str(syntax)).cloned() else {
        return Ok(None);
    };
    let Some(integer_type) = parameter_types.get(parameter_name.as_str(syntax)).copied() else {
        return Ok(None);
    };
    let members = syntax.items.identifier_path_members(membership.domain);
    let domain_path = members.joined_path_in(syntax);
    evaluate_named_const_domain(
        syntax,
        &domain_path,
        &integer_type.name(),
        ConstSubjectBinding::Scalar(ConstScalarValue::DeclaredInteger {
            value,
            integer_type,
        }),
        &mut Vec::new(),
        members
            .first()
            .map(|member| member.source_span())
            .unwrap_or_default(),
        selection,
        warnings,
    )
    .map(|proof| proof.map(|proof| proof.holds()))
}

/*
    The subject operand one selected domain replay binds for its carrier: a
    scalar payload for whole-value operands, a record or variant carrier's
    scalar-decodable field ledger for `subject.<name>` projection, or an array
    carrier's scalar-decodable element ledger for `subject[<index>]` projection.
    Aggregate carriers never bind a whole-subject scalar — whole-aggregate
    comparisons, deeper member chains, and slots without a scalar leaf stay
    undecided and their facts keep the declaration fenced.
*/
#[derive(Clone)]
enum ConstSubjectBinding<'a> {
    Scalar(ConstScalarValue),
    /*
        A record carrier's scalar-decodable field ledger, beside the selected
        declaration's authored properties that carrier bounds are checked
        against.
    */
    Record {
        fields: &'a [(String, ConstScalarValue)],
        properties: syntax_trees::item::DataProperties,
    },
    Elements(&'a [Option<ConstScalarValue>]),
    /*
        The scalar-decodable payload field ledger of a variant carrier's
        statically selected case, with the selected declaration's properties.
    */
    Variant {
        fields: &'a [(String, ConstScalarValue)],
        properties: syntax_trees::item::DataProperties,
    },
}

/*
    The lexical value scope of one predicate. Unavailable fields or generic
    subjects still shadow outer constants and index bindings with the same name.
*/
#[derive(Clone, Copy)]
enum ConstFactSubject<'a> {
    Value {
        binding: Option<&'a ConstSubjectBinding<'a>>,
    },
    /*
        Bare lexical fields in a data or active-case predicate. The complete
        nodes retain shadowing and compositional member/index projections.
    */
    Fields(&'a [(&'a str, &'a CanonicalConstNode)]),
}

impl<'a> ConstFactSubject<'a> {
    fn matches(self, syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
        let Self::Value { .. } = self else {
            return false;
        };
        let ExpressionNode::Name {
            members: path,
            application: path_application,
        } = syntax.expressions.expression(expression)
        else {
            return false;
        };
        path_application.is_none()
            && matches!(syntax.expressions.identifier_path_members(path.to_owned()), [name] if name.as_str(syntax) == "self")
    }

    fn binding(self) -> Option<&'a ConstSubjectBinding<'a>> {
        match self {
            Self::Value { binding, .. } => binding,
            Self::Fields(_) => None,
        }
    }

    fn scalar(self) -> Option<ConstScalarValue> {
        match self.binding() {
            Some(ConstSubjectBinding::Scalar(value)) => Some(value.clone()),
            _ => None,
        }
    }

    fn fields(self) -> Option<&'a [(String, ConstScalarValue)]> {
        match self.binding() {
            Some(
                ConstSubjectBinding::Record { fields, .. }
                | ConstSubjectBinding::Variant { fields, .. },
            ) => Some(fields),
            _ => None,
        }
    }
}

fn node_subject_leaf(value: &CanonicalConstNode) -> Option<ConstScalarValue> {
    match value {
        CanonicalConstNode::Integer {
            integer_type,
            value,
        } => Some(ConstScalarValue::DeclaredInteger {
            value: value.clone(),
            integer_type: *integer_type,
        }),
        CanonicalConstNode::Boolean(value) => Some(ConstScalarValue::Boolean(*value)),
        _ => None,
    }
}

/*
    `Some(None)` is a lexical field with an unavailable projection, not an
    unbound name. Keeping that distinction prevents a same-spelled constant
    from proving a predicate about an aggregate field or invalid array slot.
*/
fn const_field_projection<'value>(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    fields: &[(&str, &'value CanonicalConstNode)],
    evaluate_index: &mut impl FnMut(ExpressionHandle) -> Result<Option<ConstFactValue>, String>,
) -> Result<Option<Option<&'value CanonicalConstNode>>, String> {
    use super::values::CanonicalConstNode;
    match syntax.expressions.expression(expression) {
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => {
            let [name] = syntax.expressions.identifier_path_members(path.to_owned()) else {
                return Ok(None);
            };
            Ok(fields
                .iter()
                .find(|(field, _)| *field == name.as_str(syntax))
                .map(|(_, value)| Some(*value)))
        }
        ExpressionNode::Member(member) => {
            let logical_subject = match syntax.expressions.expression(member.receiver) {
                ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } if path_application.is_none() => {
                    matches!(syntax.expressions.identifier_path_members(path.to_owned()), [name]
                    if name.as_str(syntax) == "self")
                }
                _ => false,
            };
            if logical_subject {
                return Ok(Some(if member.case_variant.is_none() {
                    fields
                        .iter()
                        .find(|(field, _)| *field == member.member.as_str(syntax))
                        .map(|(_, value)| *value)
                } else {
                    None
                }));
            }
            let Some(receiver) =
                const_field_projection(syntax, member.receiver, fields, evaluate_index)?
            else {
                return Ok(None);
            };
            let projected = match receiver {
                Some(CanonicalConstNode::Record { fields, .. })
                    if member.case_variant.is_none() =>
                {
                    fields
                        .iter()
                        .find(|(name, _)| name == member.member.as_str(syntax))
                        .map(|(_, value)| value)
                }
                _ => None,
            };
            Ok(Some(projected))
        }
        ExpressionNode::Indexed(indexed) => {
            let Some(collection) =
                const_field_projection(syntax, indexed.collection, fields, evaluate_index)?
            else {
                return Ok(None);
            };
            let Some(CanonicalConstNode::Array { values, .. }) = collection else {
                return Ok(Some(None));
            };
            let mut warnings = Vec::new();
            let index = evaluate_index(indexed.index)?
                .map(|value| value.into_integer(syntax, &mut warnings))
                .transpose()?
                .flatten();
            // An index is an implicit integer landing too.
            if let Some(error) = warnings.iter().find(|warning| warning.is_error()) {
                return Err(error.message.clone());
            }
            let selected = index
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| values.get(index));
            Ok(Some(selected))
        }
        _ => Ok(None),
    }
}

/*
    Default-domain and active-case facts belong to the selected value, not its
    ability to serve as a canonical static index. Close them with the existing
    exact fact evaluator before ordinary declaration encoding. Recursion through
    canonical construction establishes nested values independently; unsupported
    facts remain fenced, and inactive cases contribute no establishment duty.
*/
pub(super) fn prove_const_data_predicates(
    syntax: &SyntaxTrees,
    definition: &syntax_trees::item::DataDefinition,
    value: &CanonicalConstNode,
    substitution: &crate::resolution::declarations::declaration_selection::GenericApplicationSubstitution,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> Result<(), String> {
    use super::values::CanonicalConstNode;
    let (fields, case_name) = match value {
        CanonicalConstNode::Record { fields, .. } => (fields, None),
        CanonicalConstNode::Variant {
            fields, case_name, ..
        } => (fields, Some(case_name.as_str())),
        _ => return Err("data constant lost its completed structural value".to_owned()),
    };
    let variant = case_name.and_then(|case_name| {
        syntax_items(syntax)
            .data_members(definition.members)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str(syntax) == case_name => {
                    Some(variant)
                }
                _ => None,
            })
    });
    if definition.requires_facts.is_empty()
        && variant.is_none_or(|variant| variant.requires_facts.is_empty())
    {
        return Ok(());
    }
    let mut parameter_values = HashMap::<String, ConstScalarValue>::new();
    let mut warnings = Vec::new();
    for parameter in syntax.items.type_parameters(definition.type_parameters) {
        let (TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference }) = parameter.kind
        else {
            continue;
        };
        let argument = substitution
            .get(parameter.name.as_str(syntax))
            .ok_or_else(|| "data constant predicates require closed index bindings".to_owned())?;
        let bound = evaluate_domain_index_argument(
            syntax,
            definition.name.as_str(syntax),
            parameter.name.as_str(syntax),
            type_reference,
            *argument,
            &parameter_values,
            syntax.items.type_parameters(definition.type_parameters),
            selection,
            &mut warnings,
        )?
        .ok_or_else(|| "data constant predicates require decidable index bindings".to_owned())?;
        parameter_values.insert(parameter.name.as_str(syntax).to_owned(), bound);
    }
    let active_fields = fields
        .iter()
        .map(|(name, value)| (name.as_str(), value))
        .collect::<Vec<_>>();
    let members = syntax.items.data_members(definition.members);
    let common_fields = active_fields
        .iter()
        .copied()
        .filter(|(name, _)| {
            members.iter().any(|member| match member {
                DataMember::Field(field) => field.name.as_str(syntax) == *name,
                _ => false,
            })
        })
        .collect::<Vec<_>>();
    for (facts, fields, label) in [
        (
            definition.requires_facts,
            common_fields.as_slice(),
            "default-domain",
        ),
        (
            variant.map_or(HandleSpan::empty(), |variant| variant.requires_facts),
            active_fields.as_slice(),
            "case",
        ),
    ] {
        let subject = Some(ConstFactSubject::Fields(fields));
        for fact in syntax.items.proof_facts(facts) {
            let proved = evaluate_selected_const_fact(
                syntax,
                fact,
                &parameter_values,
                syntax.items.type_parameters(definition.type_parameters),
                subject,
                definition.name.as_str(syntax),
                &mut Vec::new(),
                selection,
                &mut warnings,
            )?;
            match proved {
                Some(ConstFactValue::Boolean(true)) => {}
                Some(ConstFactValue::Boolean(false)) => {
                    return Err(format!(
                        "data constant `{}` violates its {label} predicate",
                        definition.name.as_str(syntax)
                    ));
                }
                _ => {
                    return Err(format!(
                        "data constant `{}` requires declaration-site {label} proof checking",
                        definition.name.as_str(syntax)
                    ));
                }
            }
        }
    }
    Ok(())
}

/*
    Data construction and named-domain establishment evaluate the same proof
    facts with their actual lexical bindings and selected membership owners.
*/
fn evaluate_selected_const_fact(
    syntax: &SyntaxTrees,
    fact: &ProofFact,
    parameter_values: &HashMap<String, ConstScalarValue>,
    parameters: &[syntax_trees::item::TypeParameter],
    subject: Option<ConstFactSubject<'_>>,
    carrier: &str,
    visiting: &mut Vec<(source_text::SourceSpan, String)>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    let membership = match fact {
        ProofFact::Membership(membership) => membership,
        ProofFact::Expression(expression) => {
            return evaluate_const_domain_expression(
                syntax,
                *expression,
                parameter_values,
                subject,
                carrier,
                visiting,
                selection,
                warnings,
            );
        }
    };
    let Some(nested_value) = evaluate_const_fact_expression_at(
        syntax,
        membership.value,
        parameter_values,
        selection,
        subject,
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
    let nested_carrier = nested_carrier.as_ref();
    let members = syntax.items.identifier_path_members(membership.domain);
    let path = members.joined_path_in(syntax);
    let reference = members
        .first()
        .map(|member| member.source_span())
        .unwrap_or_default();
    let holds = if membership.domain_arguments.is_empty() {
        evaluate_named_const_domain(
            syntax,
            &path,
            nested_carrier,
            ConstSubjectBinding::Scalar(nested_value),
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
            ConstSubjectBinding::Scalar(nested_value),
            parameter_values,
            parameters,
            visiting,
            reference,
            selection,
            warnings,
        )?
    };
    Ok(holds.map(|proof| ConstFactValue::Boolean(proof.holds())))
}

/*
    A refuted membership is still a Boolean operand. Keep its selected atom
    separately so direct constant establishment can explain the failed alias
    without turning false into an evaluation error inside a larger predicate.
*/
enum DomainFactProof {
    Holds,
    Refuted { domain: String },
}

impl DomainFactProof {
    fn holds(&self) -> bool {
        matches!(self, Self::Holds)
    }
}

/*
    Evaluate `value in <authored>` against one declared domain. With a constant
    selection the authored path selects its exact owner under module name law —
    the same selection full resolution later assigns the retained fact — so
    same-spelled domains in sibling modules no longer collide and an ambiguous
    or unauthorized occurrence declines instead of guessing. Without a
    selection (header-free probes) a leaf expands against the value carrier
    exactly as the original name-only lookup did.
*/
fn evaluate_const_domain_constraint(
    syntax: &SyntaxTrees,
    domain: &syntax_trees::types::DomainConstraint,
    carrier: &str,
    subject_binding: ConstSubjectBinding<'_>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<DomainFactProof>, String> {
    if domain.arguments.is_empty() {
        evaluate_named_const_domain(
            syntax,
            domain.name.as_str(syntax),
            carrier,
            subject_binding.clone(),
            &mut Vec::new(),
            domain.name.source_span(),
            selection,
            warnings,
        )
    } else {
        evaluate_indexed_const_domain(
            syntax,
            domain.name.as_str(syntax),
            domain.arguments,
            carrier,
            subject_binding.clone(),
            &HashMap::new(),
            &[],
            &mut Vec::new(),
            domain.name.source_span(),
            selection,
            warnings,
        )
    }
}

fn evaluate_named_const_domain(
    syntax: &SyntaxTrees,
    authored: &str,
    carrier: &str,
    subject_binding: ConstSubjectBinding<'_>,
    visiting: &mut Vec<(source_text::SourceSpan, String)>,
    reference: source_text::SourceSpan,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<DomainFactProof>, String> {
    let domain = match selection {
        Some(selection) => selection
            .domain(syntax, authored, reference, Some(carrier))
            .or_else(|| {
                selection
                    .domain_family(syntax, authored, reference, Some(carrier))
                    .map(|(_, definition)| definition)
            }),
        None => {
            let expanded = format!("{carrier}::{authored}");
            let mut matches = syntax.root_items().filter_map(|item| {
                let domain = item.as_domain()?;
                (domain.name.as_str(syntax) == authored || domain.name.as_str(syntax) == expanded)
                    .then_some(domain)
            });
            matches.next().filter(|_| matches.next().is_none())
        }
    };
    let Some(domain) = domain else {
        return Ok(None);
    };
    let Some(index_parameters) = domain_index_parameters(syntax, domain) else {
        return Ok(None);
    };
    if !index_parameters.is_empty() {
        return Err(format!(
            "indexed domain `{authored}` requires {} closed index argument(s), but 0 were supplied",
            index_parameters.len(),
        ));
    }
    let domain_key = module_domain_key(syntax, domain, selection);
    evaluate_selected_domain_facts(
        syntax,
        domain,
        domain_key,
        carrier,
        subject_binding,
        &HashMap::new(),
        visiting,
        selection,
        warnings,
    )
}

/*
    The selected declaration's complete logical path — module prefix plus the
    declared name. This label alone is not declaration identity: distinct
    packages can publish identical logical paths.
*/
fn module_domain_key(
    syntax: &SyntaxTrees,
    domain: &DomainDefinition,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> String {
    match module_path(selection, domain.name.source_span().source_id) {
        Some(module) => format!("{module}::{}", domain.name.as_str(syntax)),
        None => domain.name.as_str(syntax).to_owned(),
    }
}

/// Scalar values with a declared type are copyable. A record or variant
/// takes its multiplicity from the declaration selected for that value.
fn carrier_satisfies_declared_bounds(
    bounds: &syntax_trees::item::DataProperties,
    binding: &ConstSubjectBinding<'_>,
) -> bool {
    let multiplicity = match binding {
        ConstSubjectBinding::Scalar(ConstScalarValue::Integer(_))
        | ConstSubjectBinding::Elements(_) => None,
        ConstSubjectBinding::Scalar(
            ConstScalarValue::DeclaredInteger { .. } | ConstScalarValue::Boolean(_),
        ) => Some(Multiplicity::Unrestricted),
        ConstSubjectBinding::Record { properties, .. }
        | ConstSubjectBinding::Variant { properties, .. } => Some(properties.multiplicity),
    };
    multiplicity.is_some_and(|actual| {
        bounds.multiplicity == Multiplicity::Affine || bounds.multiplicity == actual
    })
}

/*
    Replay one already-selected domain declaration's facts against `value`
    with the subject bound and the application's closed index binders mapped in
    `parameter_values` (empty for a monomorphic domain). Unindexed and closed
    indexed membership share this body so both apply the same carrier check,
    recursion guard, and fact order. Nested indexed memberships reuse the
    same application evaluator with the enclosing binder values; their own
    authored spans select declarations in the fact author's source context.
*/
fn evaluate_selected_domain_facts(
    syntax: &SyntaxTrees,
    domain: &DomainDefinition,
    domain_key: String,
    carrier: &str,
    subject_binding: ConstSubjectBinding<'_>,
    parameter_values: &HashMap<String, ConstScalarValue>,
    visiting: &mut Vec<(source_text::SourceSpan, String)>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<DomainFactProof>, String> {
    if visiting
        .iter()
        .any(|(span, name)| *span == domain.name.source_span() && name == &domain_key)
    {
        return Ok(None);
    }
    if !domain.authored_routes.is_empty() || domain.access_policy.is_some() {
        return Ok(None);
    }
    let domain_target_name = match syntax.type_references.type_reference(domain.target_type) {
        TypeReferenceNode::Named(name) => Some(name.as_str(syntax)),
        _ => None,
    };
    let carrier_parameter = syntax_items(syntax)
        .type_parameters(domain.type_parameters)
        .first()
        .filter(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Type)
                && domain_target_name == Some(parameter.name.as_str(syntax))
        });
    let binding = if let Some(parameter) = carrier_parameter {
        if parameter.bounds != syntax_trees::item::DataProperties::default()
            && !carrier_satisfies_declared_bounds(&parameter.bounds, &subject_binding)
        {
            return Ok(None);
        }
        None
    } else {
        let domain_carrier = match syntax.type_references.type_reference(domain.target_type) {
            TypeReferenceNode::Named(name) => name.as_str(syntax).to_owned(),
            TypeReferenceNode::FixedArray { .. } => {
                match syntax_type_identity(syntax, domain.target_type) {
                    Ok(identity) => identity,
                    Err(_) => return Ok(None),
                }
            }
            _ => return Ok(None),
        };
        if domain_carrier != carrier {
            return Err(format!(
                "domain `{domain_key}` has carrier `{domain_carrier}`, but the const value has carrier `{carrier}`",
            ));
        }
        Some(&subject_binding)
    };
    let subject = Some(ConstFactSubject::Value { binding });
    let warning_start = warnings.len();
    visiting.push((domain.name.source_span(), domain_key));
    let result = (|| {
        if let Some(alias) = &domain.alias {
            if alias.constituents.is_empty() {
                return Ok(None);
            }
            for constituent in &alias.constituents {
                let members = syntax.items.identifier_path_members(*constituent);
                let Some(first) = members.first() else {
                    return Ok(None);
                };
                let path = members.joined_path_in(syntax);
                match evaluate_named_const_domain(
                    syntax,
                    &path,
                    carrier,
                    subject_binding.clone(),
                    visiting,
                    first.source_span(),
                    selection,
                    warnings,
                )? {
                    Some(DomainFactProof::Holds) => {}
                    Some(refuted) => return Ok(Some(refuted)),
                    None => return Ok(None),
                }
            }
        }
        for fact in syntax.items.proof_facts(domain.facts) {
            let holds = evaluate_selected_const_fact(
                syntax,
                fact,
                parameter_values,
                syntax.items.type_parameters(domain.type_parameters),
                subject,
                carrier,
                visiting,
                selection,
                warnings,
            )?;
            let Some(ConstFactValue::Boolean(holds)) = holds else {
                return Ok(None);
            };
            if !holds {
                return Ok(Some(DomainFactProof::Refuted {
                    domain: module_domain_key(syntax, domain, selection),
                }));
            }
        }
        Ok(Some(DomainFactProof::Holds))
    })();
    visiting.pop();
    if !matches!(result, Ok(Some(_))) {
        warnings.truncate(warning_start);
    }
    result
}

/*
    Bind one evaluated nested-membership operand as the subject payload the next
    selected domain replays. A Boolean operand always selects the `bool`
    carrier; an integer operand keeps the enclosing carrier, which
    the subject-derived expressions inherit. `Ok(None)` leaves the membership on the
    checked-record path rather than binding a guessed payload.
*/
fn nested_membership_operand<'a>(
    syntax: &SyntaxTrees,
    value: ConstFactValue,
    carrier: &'a str,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<(std::borrow::Cow<'a, str>, ConstScalarValue)>, String> {
    match value {
        ConstFactValue::Boolean(value) => Ok(Some((
            std::borrow::Cow::Borrowed("bool"),
            ConstScalarValue::Boolean(value),
        ))),
        ConstFactValue::DeclaredInteger {
            value,
            integer_type,
        } => Ok(Some((
            std::borrow::Cow::Owned(integer_type.name()),
            ConstScalarValue::DeclaredInteger {
                value,
                integer_type,
            },
        ))),
        value => Ok(value.into_integer(syntax, warnings)?.map(|value| {
            (
                std::borrow::Cow::Borrowed(carrier),
                ConstScalarValue::Integer(value),
            )
        })),
    }
}

/*
    Evaluate `value in <authored><args>` — a closed index application of a
    generic domain family — at declaration site. The authored spelling selects
    the family under the same module name law `domain` applies to monomorphic
    domains (a qualified path names its exact owner, module-local precedence
    ranks same-leaf candidates, and relative spellings stay import-gated);
    contested, unreachable, or non-generic owners decline. Every index
    parameter must be an integer or Boolean const binder and every argument
    must close to that exact carrier against the enclosing scope's bindings; the
    family's facts then replay through the same evaluator monomorphic domains
    use, with the subject bound to `value` and each binder mapped to its argument.
    A non-scalar or non-const index parameter,
    an argument that stays open (enclosing binders, module-constant spellings,
    names still owned by resolved selection), and an unprovable fact all
    decline with `None` so the declaration keeps its fence instead of
    discharging against a guessed binding.
*/
fn evaluate_indexed_const_domain(
    syntax: &SyntaxTrees,
    authored: &str,
    argument_span: HandleSpan<TypeReferenceHandle>,
    carrier: &str,
    subject_binding: ConstSubjectBinding<'_>,
    argument_values: &HashMap<String, ConstScalarValue>,
    argument_parameters: &[syntax_trees::item::TypeParameter],
    visiting: &mut Vec<(source_text::SourceSpan, String)>,
    reference: source_text::SourceSpan,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<DomainFactProof>, String> {
    let domain = match selection {
        Some(selection) => selection
            .domain_family(syntax, authored, reference, Some(carrier))
            .map(|(_, definition)| definition),
        None => {
            let expanded = format!("{carrier}::{authored}");
            let mut matches = syntax.root_items().filter_map(|item| match item {
                Item::Domain(domain)
                    if domain.name.as_str(syntax) == authored
                        || domain.name.as_str(syntax) == expanded =>
                {
                    Some(domain)
                }
                _ => None,
            });
            match matches.next() {
                Some(domain)
                    if matches.next().is_none()
                        && !domain.type_parameters.is_empty()
                        && module_path(selection, domain.name.source_span().source_id).is_none() =>
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
    let domain_key = module_domain_key(syntax, domain, selection);
    let Some(index_parameters) = domain_index_parameters(syntax, domain) else {
        return Ok(None);
    };
    let arguments = syntax.type_references.type_reference_handles(argument_span);
    if arguments.len() != index_parameters.len() {
        return Err(format!(
            "indexed domain `{authored}` requires {} closed index argument(s), but {} were supplied",
            index_parameters.len(),
            arguments.len(),
        ));
    }
    let mut parameter_values = HashMap::new();
    for (parameter, argument) in index_parameters.iter().zip(arguments.iter()) {
        let (TypeParameterKind::Const {
            type_reference: parameter_type,
        }
        | TypeParameterKind::Value {
            type_reference: parameter_type,
        }) = parameter.kind
        else {
            return Ok(None);
        };
        let Some(bound) = evaluate_domain_index_argument(
            syntax,
            authored,
            parameter.name.as_str(syntax),
            parameter_type,
            *argument,
            argument_values,
            argument_parameters,
            selection,
            warnings,
        )?
        else {
            return Ok(None);
        };
        let required = syntax_type_identity(syntax, parameter_type)?;
        let bound = match bound {
            ConstScalarValue::Integer(bound) => {
                if required == "bool" {
                    return Err(format!(
                        "index argument for `{authored}::{}` is an integer, expected `bool`",
                        parameter.name.as_str(syntax),
                    ));
                }
                let Some(integer_type) =
                    crate::resolution::declarations::generic_data::const_integer_type(
                        syntax,
                        parameter_type,
                        selection,
                    )
                else {
                    return Ok(None);
                };
                validate_declared_integer(integer_type, &bound)?;
                ConstScalarValue::DeclaredInteger {
                    value: bound,
                    integer_type,
                }
            }
            ConstScalarValue::DeclaredInteger {
                value,
                integer_type,
            } => {
                if Some(integer_type)
                    != crate::resolution::declarations::generic_data::const_integer_type(
                        syntax,
                        parameter_type,
                        selection,
                    )
                {
                    return Err(format!(
                        "index argument for `{authored}::{}` has canonical type `{}`, expected `{required}`",
                        parameter.name.as_str(syntax),
                        integer_type.name(),
                    ));
                }
                validate_declared_integer(integer_type, &value)?;
                ConstScalarValue::DeclaredInteger {
                    value,
                    integer_type,
                }
            }
            ConstScalarValue::Boolean(_) if required != "bool" => {
                return Err(format!(
                    "index argument for `{authored}::{}` has canonical type `bool`, expected `{required}`",
                    parameter.name.as_str(syntax),
                ));
            }
            ConstScalarValue::Boolean(_) => bound,
        };
        parameter_values.insert(parameter.name.as_str(syntax).to_owned(), bound);
    }
    evaluate_selected_domain_facts(
        syntax,
        domain,
        domain_key,
        carrier,
        subject_binding,
        &parameter_values,
        visiting,
        selection,
        warnings,
    )
}

/*
    Evaluate one supplied index argument of a domain-family application to the
    scalar its const binder receives, under the same admissible forms the
    closed-index canonicalizer accepts: literal and canonical leaves, the
    caller's bound index parameters, exactly selected constant declarations,
    and the scoped integer ledger. `Ok(None)` means the argument cannot be
    decided at this point — an unresolved name, a spelling ordinary resolution
    still owns, or a non-scalar canonical value — so the application stays
    fenced rather than binding a guessed value.
*/
fn evaluate_domain_index_argument(
    syntax: &SyntaxTrees,
    family_name: &str,
    parameter_name: &str,
    parameter_type: TypeReferenceHandle,
    argument: TypeReferenceHandle,
    argument_values: &HashMap<String, ConstScalarValue>,
    argument_parameters: &[syntax_trees::item::TypeParameter],
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstScalarValue>, String> {
    let integer_type = crate::resolution::declarations::generic_data::const_integer_type(
        syntax,
        parameter_type,
        selection,
    );
    let boolean_type = matches!(
        syntax.type_references.type_reference(parameter_type),
        TypeReferenceNode::Named(name) if name.as_str(syntax) == "bool"
    );
    if integer_type.is_none() && !boolean_type {
        return Ok(None);
    }
    match syntax.type_references.type_reference(argument) {
        TypeReferenceNode::Named(name) => {
            match ConstScalarSpelling::from_bare_name(name.as_str(syntax)) {
                Some(ConstScalarSpelling::Canonical(atom)) => {
                    let required = syntax_type_identity(syntax, parameter_type)?;
                    if atom.type_name != required {
                        return Err(format!(
                            "index argument for `{family_name}::{parameter_name}` has canonical type `{}`, expected `{required}`",
                            atom.type_name,
                        ));
                    }
                    return Ok(ConstScalarValue::from_canonical(&atom));
                }
                Some(ConstScalarSpelling::Integer(value)) => {
                    if let Some(integer_type) = integer_type {
                        validate_declared_integer(integer_type, &value)?;
                        return Ok(Some(ConstScalarValue::DeclaredInteger {
                            value,
                            integer_type,
                        }));
                    }
                    return Ok(Some(ConstScalarValue::Integer(value)));
                }
                Some(ConstScalarSpelling::Boolean(value)) => {
                    return Ok(Some(ConstScalarValue::Boolean(value)));
                }
                None => {}
            }
            if let Some(value) = argument_values.get(name.as_str(syntax)) {
                let Some(syntax_trees::item::TypeParameter { kind, .. }) = argument_parameters
                    .iter()
                    .find(|parameter| parameter.name.as_str(syntax) == name.as_str(syntax))
                else {
                    return Ok(None);
                };
                let type_reference = match kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => *type_reference,
                    _ => return Ok(None),
                };
                let actual = syntax_type_identity(syntax, type_reference)?;
                let required = syntax_type_identity(syntax, parameter_type)?;
                if actual != required {
                    return Err(format!(
                        "index argument for `{family_name}::{parameter_name}` has carrier `{actual}`, expected `{required}`"
                    ));
                }
                return Ok(Some(value.clone()));
            }
            if let Some(selection) = selection
                && let Some(definition) = selection
                    .select(syntax, name)
                    .map_err(|diagnostic| diagnostic.message)?
            {
                let value = crate::resolution::declarations::generic_data::canonicalize_selected_const_definition(
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
            Ok(None)
        }
        TypeReferenceNode::ConstExpression(expression) => {
            if syntax.const_expression_contains_name(*expression)
                || super::anonymous::requires_const_operator_selection(syntax, *expression)
            {
                return Ok(None);
            }
            if boolean_type {
                return Ok(None);
            }
            let value =
                crate::resolution::declarations::generic_data::evaluate_const_argument_expression(
                    syntax,
                    *expression,
                    selection,
                    &HashMap::new(),
                    &HashSet::new(),
                    integer_type,
                    warnings,
                )
                .and_then(
                    crate::resolution::declarations::generic_data::EvaluatedConst::into_concrete,
                )
                .map_err(|reason| {
                    format!("index argument expression for `{family_name}` is invalid: {reason}")
                })?;
            let integer_type =
                integer_type.ok_or("const index lost its declared integer carrier")?;
            validate_declared_integer(integer_type, &value)?;
            Ok(Some(ConstScalarValue::Integer(value)))
        }
        _ => Ok(None),
    }
}

/*
    The shared fence for constrained const declarations whose carrier's
    constraints cannot be proved at declaration site.
*/
const CONSTRAINED_CONST_FENCE: &str = "constrained const declarations require declaration-site proof checking before they can publish compatibility identity";

/*
    One Domain constraint discharged over a bound const subject: `Holds`
    passes silently, `Refuted` fails naming the caller's subject, and an
    undecided fact fences rather than guessing the binding.
*/
fn prove_const_domain_constraint(
    syntax: &SyntaxTrees,
    constraint: &TypeConstraintNode,
    carrier: &str,
    subject_binding: ConstSubjectBinding<'_>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
    subject_label: &str,
) -> Result<(), String> {
    let TypeConstraintNode::Domain(domain) = constraint else {
        return Ok(());
    };
    let holds = evaluate_const_domain_constraint(
        syntax,
        domain,
        carrier,
        subject_binding,
        selection,
        warnings,
    )?;
    match holds {
        Some(DomainFactProof::Holds) => Ok(()),
        Some(DomainFactProof::Refuted {
            domain: failed_domain,
        }) => Err(format!(
            "domain constraint `{}` for {subject_label} is false; failed domain `{failed_domain}`",
            domain.name.as_str(syntax),
        )),
        None => Err(CONSTRAINED_CONST_FENCE.to_owned()),
    }
}

/*
    Discharge the `Domain` constraints a `Constrained` leaf carrier declares —
    applied to each leaf a constrained field or
    element type names as an initializer canonicalizes through
    `canonicalize_const_expression`. Aggregate leaves bind the subject through their
    scalar-decodable members exactly as the declaration path does; a leaf
    without a bindable payload, an unselected owner, or an undecided fact stays
    fenced rather than publishing a guessed identity. Constraints of other
    kinds keep their own validation sites.
*/
pub(in crate::resolution::declarations::generic_data) fn prove_const_leaf_domain_constraints(
    syntax: &SyntaxTrees,
    constraints: &[TypeConstraintNode],
    value: &CanonicalConstNode,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> Result<(), String> {
    use super::values::CanonicalConstNode;
    if !constraints
        .iter()
        .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
    {
        return Ok(());
    }
    let record_fields: Vec<(String, ConstScalarValue)>;
    let array_elements: Vec<Option<ConstScalarValue>>;
    let variant_fields: Vec<(String, ConstScalarValue)>;
    let (carrier, subject_binding) = match value {
        CanonicalConstNode::Integer {
            integer_type,
            value,
        } => (
            integer_type.name(),
            ConstSubjectBinding::Scalar(ConstScalarValue::DeclaredInteger {
                value: value.clone(),
                integer_type: *integer_type,
            }),
        ),
        CanonicalConstNode::Boolean(value) => (
            "bool".to_owned(),
            ConstSubjectBinding::Scalar(ConstScalarValue::Boolean(*value)),
        ),
        CanonicalConstNode::Record {
            type_name,
            properties,
            fields,
        } => {
            record_fields = fields
                .iter()
                .filter_map(|(name, field)| {
                    node_subject_leaf(field).map(|value| (name.clone(), value))
                })
                .collect();
            (
                type_name.clone(),
                ConstSubjectBinding::Record {
                    fields: &record_fields,
                    properties: *properties,
                },
            )
        }
        CanonicalConstNode::Array { type_name, values } => {
            array_elements = values.iter().map(node_subject_leaf).collect();
            (
                type_name.clone(),
                ConstSubjectBinding::Elements(&array_elements),
            )
        }
        CanonicalConstNode::Variant {
            type_name,
            properties,
            fields,
            ..
        } => {
            variant_fields = fields
                .iter()
                .filter_map(|(name, field)| {
                    node_subject_leaf(field).map(|value| (name.clone(), value))
                })
                .collect();
            (
                type_name.clone(),
                ConstSubjectBinding::Variant {
                    fields: &variant_fields,
                    properties: *properties,
                },
            )
        }
        _ => return Err(CONSTRAINED_CONST_FENCE.to_owned()),
    };
    let mut warnings = Vec::new();
    for constraint in constraints {
        prove_const_domain_constraint(
            syntax,
            constraint,
            &carrier,
            subject_binding.clone(),
            selection,
            &mut warnings,
            "the const leaf value",
        )?;
    }
    Ok(())
}

/*
    Find declaration predicates through the exact selected member types. This
    only selects a required check; it grants no value or membership. Recursion
    rejoins source-owned declarations rather than guessing from a leaf spelling.
*/
pub(crate) fn const_type_requires_declaration_proof(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<source_text::SourceSpan>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> bool {
    use syntax_trees::item::DataMember;
    let origin = syntax_type_references(syntax).generic_application_origin(type_reference);
    if origin.is_valid() && origin != type_reference {
        return const_type_requires_declaration_proof(syntax, origin, visiting, selection);
    }
    fn named_data(
        syntax: &SyntaxTrees,
        name: &Identifier,
        visiting: &mut Vec<source_text::SourceSpan>,
        selection: Option<
            &crate::resolution::declarations::declaration_selection::ConstantSelection,
        >,
    ) -> bool {
        let Ok(data) = super::values::selected_data(syntax, name, selection) else {
            return false;
        };
        if visiting.contains(&data.name.source_span()) {
            return false;
        }
        visiting.push(data.name.source_span());
        let value_facts = super::super::template_type_equation_offsets(syntax, data, selection)
            .map_or(true, |offsets| offsets.len() != data.requires_facts.len());
        let requires_proof =
            value_facts
                || syntax.tables.items.data_members(data.members).iter().any(
                    |member| match member {
                        DataMember::Field(field) => const_type_requires_declaration_proof(
                            syntax,
                            field.type_reference,
                            visiting,
                            selection,
                        ),
                        DataMember::Variant(variant) => {
                            !variant.requires_facts.is_empty()
                                || syntax_items(syntax)
                                    .data_payload_fields(variant.payload)
                                    .iter()
                                    .any(|field| {
                                        const_type_requires_declaration_proof(
                                            syntax,
                                            field.type_reference,
                                            visiting,
                                            selection,
                                        )
                                    })
                        }
                        _ => false,
                    },
                );
        visiting.pop();
        requires_proof
    }
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            syntax_type_references(syntax)
                .constraints(*constraints)
                .iter()
                .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
                || const_type_requires_declaration_proof(syntax, *base_type, visiting, selection)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            const_type_requires_declaration_proof(syntax, *element_type, visiting, selection)
        }
        TypeReferenceNode::Named(name) => named_data(syntax, name, visiting, selection),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            syntax_type_references(syntax)
                .type_reference_handles(*arguments)
                .iter()
                .any(|argument| {
                    const_type_requires_declaration_proof(syntax, *argument, visiting, selection)
                })
                || named_data(syntax, base_name, visiting, selection)
        }
        _ => false,
    }
}

/*
    Evaluate one fact expression inside a selected domain declaration with
    the subject bound to the checked value only for a fixed carrier. A generic
    carrier leaves it unavailable rather than assuming an instance's algebra.
    `parameter_values` carries the
    application's closed index bindings so a family fact like `subject < N`
    resolves its binder exactly; a monomorphic domain passes an empty map.
*/
fn evaluate_const_domain_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    parameter_values: &HashMap<String, ConstScalarValue>,
    subject: Option<ConstFactSubject<'_>>,
    carrier: &str,
    visiting: &mut Vec<(source_text::SourceSpan, String)>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    if let Some(value) = evaluate_anonymous_numeric_expression(syntax, expression)? {
        return Ok(Some(ConstFactValue::Anonymous(value)));
    }
    let warning_start = warnings.len();
    let result = (|| match syntax.expressions.expression(expression) {
        ExpressionNode::Membership(membership) => {
            let Some(value) = evaluate_const_fact_expression_at(
                syntax,
                membership.value,
                parameter_values,
                selection,
                subject,
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
            let nested_carrier = nested_carrier.as_ref();
            let members = syntax_expressions(syntax).identifier_path_members(membership.domain);
            let path = members.joined_path_in(syntax);
            evaluate_named_const_domain(
                syntax,
                &path,
                nested_carrier,
                ConstSubjectBinding::Scalar(value),
                visiting,
                members
                    .first()
                    .map(|member| member.source_span())
                    .unwrap_or_default(),
                selection,
                warnings,
            )
            .map(|result| result.map(|proof| ConstFactValue::Boolean(proof.holds())))
        }
        ExpressionNode::Binary(binary) => {
            if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
                return Ok(None);
            }
            let builtin = has_builtin_const_operator(syntax, binary);
            if !builtin
                && matches!(
                    subject.and_then(ConstFactSubject::scalar),
                    Some(ConstScalarValue::Integer(_))
                )
            {
                return Ok(None);
            }
            validate_anonymous_remainder(syntax, binary)?;
            let Some(left) = evaluate_const_domain_expression(
                syntax,
                binary.left,
                parameter_values,
                subject,
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
                parameter_values,
                subject,
                carrier,
                visiting,
                selection,
                warnings,
            )?
            else {
                return Ok(None);
            };
            if !builtin
                && authored_operator_can_bind(syntax, binary.operator, &left, &right, selection)
            {
                return Ok(None);
            }
            evaluate_const_fact_binary(syntax, expression, binary.operator, left, right, warnings)
                .map(Some)
        }
        _ => evaluate_const_fact_expression_at(
            syntax,
            expression,
            parameter_values,
            selection,
            subject,
            warnings,
        ),
    })();
    if !matches!(result, Ok(Some(_))) {
        warnings.truncate(warning_start);
    }
    result
}
