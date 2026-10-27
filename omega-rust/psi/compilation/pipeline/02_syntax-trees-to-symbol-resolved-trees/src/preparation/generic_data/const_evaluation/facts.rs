//! Declaration-site domain discharge shares the closed fact evaluator used by
//! generic data. Initializer probes determine values, not membership: ordinary
//! constant validation still relies on this gate before publishing identity.
//! Scalar replay carries declaration selection through every recursive operand;
//! lexical subjects and binders precede selected constants, whose canonical
//! values retain their carrier and policy before arithmetic.
//! Exact declaration selection, complete index binding and establishment
//! obligations therefore precede fact replay. A generic carrier does not supply
//! a selected algebra for the subject; only carrier-independent facts can close here.
use super::super::{
    ConstDefinition, Diagnostic, ExpressionHandle, ExpressionNode, HashMap, HashSet, Item,
    ProofFact, SyntaxTrees, TypeConstraintNode, TypeParameterKind, TypeReferenceHandle,
    TypeReferenceNode,
};
use arena::HandleSpan;
use syntax_trees::expression::UnaryOperator;
use syntax_trees::identifier::Identifier;
use syntax_trees::identifier::JoinedSyntaxPath;

use crate::preparation::generic_data::const_evaluation::validate_anonymous_remainder;
use crate::preparation::generic_data::evaluate_const_fact_binary;
use crate::preparation::generic_data::integer_literal_value;
use crate::preparation::generic_data::{ConstFactValue, ConstScalarSpelling, ConstScalarValue};

use super::anonymous::{
    authored_operator_can_bind, evaluate_anonymous_numeric_expression, has_builtin_const_operator,
};
use super::arguments::validate_declared_integer;
use numerics::literals::LandedIntegerType;

/// Evaluate a proof expression exactly when every operand is known at generic
/// instantiation time. `None` means the fact still depends on a runtime field
/// and must remain on the synthesized record.
/// Integer synthesis callers and mixed domain binders share the same fact
/// operations without copying their environments or encoding Booleans as 0/1.
pub(crate) fn evaluate_const_fact_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, impl Copy + Into<ConstScalarValue>>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    evaluate_const_fact_expression_at(
        syntax,
        expression,
        const_values,
        parameter_values,
        selection,
        None,
        warnings,
    )
}

/// Replay facts with the selected domain's lexical subject, if any. Aggregate
/// subjects expose scalar-decodable fields or elements, not whole-value scalars.
fn evaluate_const_fact_expression_at(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, impl Copy + Into<ConstScalarValue>>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    subject: Option<ConstFactSubject<'_>>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    // Data predicates bind bare fields, including non-scalar fields that must
    // shadow outer constants. Project through their actual completed tree;
    // an unavailable projection supplies no proof and never falls back to a name.
    if let Some(ConstFactSubject::Fields(fields)) = subject
        && let Some(value) = const_field_projection(syntax, expression, fields, &mut |index| {
            evaluate_const_fact_expression_at(
                syntax,
                index,
                const_values,
                parameter_values,
                selection,
                subject,
                warnings,
            )
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
            let value = integer_literal_value(literal).ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            })?;
            let Some(landing) = literal.landing() else {
                return Ok(Some(ConstFactValue::Integer(value)));
            };
            if landing.domain != numerics::arithmetic::ArithmeticDomain::Exact {
                return Ok(None);
            }
            let integer_type = landing.landed_type;
            validate_declared_integer(integer_type, value)?;
            Ok(Some(ConstFactValue::DeclaredInteger {
                value,
                integer_type,
            }))
        }
        ExpressionNode::Boolean(value) => Ok(Some(ConstFactValue::Boolean(*value))),
        ExpressionNode::Name(path) => {
            let members = syntax.expressions.identifier_path_members(*path);
            let name = members.joined_path_in(syntax);
            if let Some(subject) = subject.filter(|subject| subject.matches(syntax, expression)) {
                return Ok(subject.scalar().map(ConstScalarValue::into_fact_value));
            }
            if let Some(value) = parameter_values.get(&name) {
                return Ok(Some((*value).into().into_fact_value()));
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
                    // Selected lookup is authoritative: an unresolved name
                    // must remain a checked fact, never consult a spelling ledger.
                    return Ok(None);
                };
                // Canonicalization preserves declared carrier and policy. An
                // opaque declaration remains the ordinary validator's obligation;
                // it supplies no scalar proof here, including on probe failure.
                let Ok(value) = super::values::canonicalize_selected_declaration_value(
                    syntax,
                    &definition,
                    Some(selection),
                ) else {
                    return Ok(None);
                };
                return Ok(
                    ConstScalarValue::from_canonical(&value).map(ConstScalarValue::into_fact_value)
                );
            }
            crate::preparation::generic_data::module_constants::reject_module_constant_selection(
                syntax,
                &name,
                members
                    .first()
                    .map(|member| member.source_span())
                    .unwrap_or_default(),
            )?;
            let Some(value) = const_values.get(&name).copied() else {
                return Ok(None);
            };
            // The lexical ledger caches payloads, not integer meanings. Rejoin
            // its unique declaration before a fact can use the payload in an
            // operation or as a nested membership subject.
            let mut definitions = syntax.root_items().filter_map(|item| match item {
                Item::Const(definition)
                    if super::qualified_const_name(syntax, definition) == name =>
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
            let Some(integer_type) =
                super::arguments::const_integer_type(syntax, definition.type_reference)
            else {
                return Ok(None);
            };
            validate_declared_integer(integer_type, value)?;
            Ok(Some(ConstFactValue::DeclaredInteger {
                value,
                integer_type,
            }))
        }
        // A record carrier's subject answers only `subject.<name>` reads against
        // its scalar-decodable field ledger. Nested receivers, case-payload
        // reads and fields without a scalar leaf stay undecided so facts
        // reading them keep the declaration fenced.
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
                .map(|(_, value)| value.into_fact_value()))
        }
        // An array carrier's subject answers only `subject[<index>]` reads against
        // its scalar-decodable element ledger: the index must close to an
        // in-bounds integer and the element must hold a scalar leaf.
        ExpressionNode::Indexed(indexed) => {
            let Some(ConstSubjectBinding::Elements(elements)) =
                subject.and_then(ConstFactSubject::binding)
            else {
                return Ok(None);
            };
            if !subject.is_some_and(|subject| subject.matches(syntax, indexed.collection)) {
                return Ok(None);
            }
            let Some(index_value) = evaluate_const_fact_expression_at(
                syntax,
                indexed.index,
                const_values,
                parameter_values,
                selection,
                subject,
                warnings,
            )?
            else {
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
                .copied()
                .flatten()
                .map(|value| value.into_fact_value()))
        }
        ExpressionNode::Unary(unary) => match unary.operator {
            UnaryOperator::LogicalNot => Ok(evaluate_const_fact_expression_at(
                syntax,
                unary.operand,
                const_values,
                parameter_values,
                selection,
                subject,
                warnings,
            )?
            .and_then(|value| match value {
                ConstFactValue::Boolean(value) => Some(ConstFactValue::Boolean(!value)),
                _ => None,
            })),
            UnaryOperator::BitwiseNot => Ok(None),
        },
        ExpressionNode::Binary(binary) => {
            // `has_builtin_const_operator` answers before operand carriers
            // are known: an authored spelling on an incompatible carrier
            // (the core `Nat` `<=` and `-` machines) makes it decline facts
            // the authored machine could never bind. A subject-bound replay
            // keeps that opaque answer — its scalar binding is carrier-lossy
            // — while closed operands let `authored_operator_can_bind`
            // decide by carrier.
            let builtin = has_builtin_const_operator(syntax, binary);
            if !builtin && subject.and_then(ConstFactSubject::scalar).is_some() {
                return Ok(None);
            }
            validate_anonymous_remainder(syntax, binary)?;
            let Some(left) = evaluate_const_fact_expression_at(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                selection,
                subject,
                warnings,
            )?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_fact_expression_at(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                selection,
                subject,
                warnings,
            )?
            else {
                return Ok(None);
            };
            if !builtin && authored_operator_can_bind(syntax, binary.operator, &left, &right) {
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

/// Discharge `N in Domain` when `N` is a concrete const parameter and the
/// domain is defined by evaluable boolean facts over the subject. Machine-call facts
/// stay on the concrete record for typed build-time evaluation.
pub(crate) fn evaluate_const_membership_fact(
    syntax: &SyntaxTrees,
    membership: &syntax_trees::item::ProofMembershipFact,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    parameter_type_names: &HashMap<String, String>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
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
    let Some(value) = parameter_values.get(parameter_name.as_str(syntax)).copied() else {
        return Ok(None);
    };
    let Some(parameter_type) = parameter_type_names.get(parameter_name.as_str(syntax)) else {
        return Ok(None);
    };
    let members = syntax.items.identifier_path_members(membership.domain);
    let domain_path = members.joined_path_in(syntax);
    evaluate_named_const_domain(
        syntax,
        &domain_path,
        parameter_type,
        ConstSubjectBinding::Scalar(ConstScalarValue::Integer(value)),
        const_values,
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

/// The subject operand one selected domain replay binds for its carrier: a
/// scalar payload for whole-value operands, a record or variant carrier's
/// scalar-decodable field ledger for `subject.<name>` projection, or an array
/// carrier's scalar-decodable element ledger for `subject[<index>]` projection.
/// Aggregate carriers never bind a whole-subject scalar — whole-aggregate
/// comparisons, deeper member chains, and slots without a scalar leaf stay
/// undecided and their facts keep the declaration fenced.
#[derive(Clone, Copy)]
enum ConstSubjectBinding<'a> {
    Scalar(ConstScalarValue),
    Record(&'a [(String, ConstScalarValue)]),
    Elements(&'a [Option<ConstScalarValue>]),
    /// The scalar-decodable payload field ledger of a variant carrier's
    /// statically selected case.
    Variant(&'a [(String, ConstScalarValue)]),
}

/// The lexical value scope of one predicate. Unavailable fields or generic
/// subjects still shadow outer constants and index bindings with the same name.
#[derive(Clone, Copy)]
enum ConstFactSubject<'a> {
    Value {
        name: &'a str,
        binding: Option<ConstSubjectBinding<'a>>,
    },
    /// Bare lexical fields in a data or active-case predicate. The complete
    /// nodes retain shadowing and compositional member/index projections.
    Fields(&'a [(&'a str, &'a super::values::CanonicalConstNode)]),
}

impl<'a> ConstFactSubject<'a> {
    fn matches(self, syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
        let Self::Value {
            name: subject_name, ..
        } = self
        else {
            return false;
        };
        let ExpressionNode::Name(path) = syntax.expressions.expression(expression) else {
            return false;
        };
        matches!(syntax.expressions.identifier_path_members(*path), [name] if name.as_str(syntax) == subject_name)
    }

    fn binding(self) -> Option<ConstSubjectBinding<'a>> {
        match self {
            Self::Value { binding, .. } => binding,
            Self::Fields(_) => None,
        }
    }

    fn scalar(self) -> Option<ConstScalarValue> {
        match self.binding() {
            Some(ConstSubjectBinding::Scalar(value)) => Some(value),
            _ => None,
        }
    }

    fn fields(self) -> Option<&'a [(String, ConstScalarValue)]> {
        match self.binding() {
            Some(ConstSubjectBinding::Record(fields) | ConstSubjectBinding::Variant(fields)) => {
                Some(fields)
            }
            _ => None,
        }
    }
}

fn node_subject_leaf(value: &super::values::CanonicalConstNode) -> Option<ConstScalarValue> {
    match value {
        super::values::CanonicalConstNode::Integer { type_name, value } => {
            LandedIntegerType::from_name(type_name).map(|integer_type| {
                ConstScalarValue::DeclaredInteger {
                    value: *value,
                    integer_type,
                }
            })
        }
        super::values::CanonicalConstNode::Boolean(value) => {
            Some(ConstScalarValue::Boolean(*value))
        }
        _ => None,
    }
}

/// `Some(None)` is a lexical field with an unavailable projection, not an
/// unbound name. Keeping that distinction prevents a same-spelled constant
/// from proving a predicate about an aggregate field or invalid array slot.
fn const_field_projection<'value>(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    fields: &[(&str, &'value super::values::CanonicalConstNode)],
    evaluate_index: &mut impl FnMut(ExpressionHandle) -> Result<Option<ConstFactValue>, String>,
) -> Result<Option<Option<&'value super::values::CanonicalConstNode>>, String> {
    use super::values::CanonicalConstNode;
    match syntax.expressions.expression(expression) {
        ExpressionNode::Name(path) => {
            let [name] = syntax.expressions.identifier_path_members(*path) else {
                return Ok(None);
            };
            Ok(fields
                .iter()
                .find(|(field, _)| *field == name.as_str(syntax))
                .map(|(_, value)| Some(*value)))
        }
        ExpressionNode::Member(member) => {
            // This projector is entered only with one data predicate's own
            // completed field ledger. Its logical subject is not a runtime
            // value or an outer lexical name; unavailable members stay fenced.
            let logical_subject = match syntax.expressions.expression(member.receiver) {
                ExpressionNode::Name(path) => {
                    matches!(syntax.expressions.identifier_path_members(*path), [name]
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
            let selected = evaluate_index(indexed.index)?
                .map(|value| value.into_integer(syntax, &mut warnings))
                .transpose()?
                .flatten()
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| values.get(index));
            Ok(Some(selected))
        }
        _ => Ok(None),
    }
}

/// Default-domain and active-case facts belong to the selected value, not its
/// ability to serve as a canonical static index. Close them with the existing
/// exact fact evaluator before ordinary declaration encoding. Recursion through
/// canonical construction establishes nested values independently; unsupported
/// facts remain fenced, and inactive cases contribute no establishment duty.
pub(super) fn prove_const_data_predicates(
    syntax: &SyntaxTrees,
    definition: &syntax_trees::item::DataDefinition,
    value: &super::values::CanonicalConstNode,
    substitution: &crate::preparation::declaration_selection::GenericApplicationSubstitution,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
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
        syntax
            .items
            .data_members(definition.members)
            .iter()
            .find_map(|member| match member {
                syntax_trees::item::DataMember::Variant(variant)
                    if variant.name.as_str(syntax) == case_name =>
                {
                    Some(variant)
                }
                _ => None,
            })
    });
    if definition.where_facts.is_empty()
        && variant.is_none_or(|variant| variant.where_facts.is_empty())
    {
        return Ok(());
    }
    let const_values =
        crate::preparation::generic_data::module_constants::lexical_integer_const_values(syntax);
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
            &const_values,
            &parameter_values,
            syntax.items.type_parameters(definition.type_parameters),
            selection,
            &mut warnings,
        )?
        .ok_or_else(|| "data constant predicates require decidable index bindings".to_owned())?;
        parameter_values.insert(parameter.name.as_str(syntax).to_owned(), bound);
    }
    // A type-wide predicate may read only common fields. The active case's
    // predicate additionally sees its payload; siblings never enter the binding.
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
                syntax_trees::item::DataMember::Field(field) => field.name.as_str(syntax) == *name,
                _ => false,
            })
        })
        .collect::<Vec<_>>();
    for (facts, fields, label) in [
        (
            definition.where_facts,
            common_fields.as_slice(),
            "default-domain",
        ),
        (
            variant.map_or(HandleSpan::empty(), |variant| variant.where_facts),
            active_fields.as_slice(),
            "case",
        ),
    ] {
        let subject = Some(ConstFactSubject::Fields(fields));
        for fact in syntax.items.proof_facts(facts) {
            let proved = evaluate_selected_const_fact(
                syntax,
                fact,
                &const_values,
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

/// Data construction and named-domain establishment evaluate the same proof
/// facts with their actual lexical bindings and selected membership owners.
fn evaluate_selected_const_fact(
    syntax: &SyntaxTrees,
    fact: &ProofFact,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, ConstScalarValue>,
    parameters: &[syntax_trees::item::TypeParameter],
    subject: Option<ConstFactSubject<'_>>,
    carrier: &str,
    visiting: &mut Vec<(source::SourceSpan, String)>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstFactValue>, String> {
    let membership = match fact {
        ProofFact::Membership(membership) => membership,
        ProofFact::Expression(expression) => {
            return evaluate_const_domain_expression(
                syntax,
                *expression,
                const_values,
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
        const_values,
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
            ConstSubjectBinding::Scalar(nested_value),
            const_values,
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

/// A refuted membership is still a Boolean operand. Keep its selected atom
/// separately so direct constant establishment can explain the failed alias
/// without turning false into an evaluation error inside a larger predicate.
enum DomainFactProof {
    Holds,
    Refuted { domain: String },
}

impl DomainFactProof {
    fn holds(&self) -> bool {
        matches!(self, Self::Holds)
    }
}

/// Evaluate `value in <authored>` against one declared domain. With a constant
/// selection the authored path selects its exact owner under module name law —
/// the same selection full resolution later assigns the retained fact — so
/// same-spelled domains in sibling modules no longer collide and an ambiguous
/// or unauthorized occurrence declines instead of guessing. Without a
/// selection (header-free probes) a leaf expands against the value carrier
/// exactly as the original name-only lookup did.
fn evaluate_const_domain_constraint(
    syntax: &SyntaxTrees,
    domain: &syntax_trees::types::DomainConstraint,
    carrier: &str,
    subject_binding: ConstSubjectBinding<'_>,
    const_values: &HashMap<String, i128>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<DomainFactProof>, String> {
    if domain.arguments.is_empty() {
        evaluate_named_const_domain(
            syntax,
            domain.name.as_str(syntax),
            carrier,
            subject_binding,
            const_values,
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
            subject_binding,
            const_values,
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
    const_values: &HashMap<String, i128>,
    visiting: &mut Vec<(source::SourceSpan, String)>,
    reference: source::SourceSpan,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
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
        // A `Carry::*` atom is compiler-owned grant evidence, not a selectable
        // domain: membership needs an admitted carry receipt the const subject
        // cannot produce at its declaration site.
        if authored == "Carry::Portable"
            || language_core::CarryPermission::from_name(authored).is_some()
        {
            return Err(format!(
                "domain constraint `{authored}` names a compiler carry permission; const declarations cannot supply its boundary-grant evidence",
            ));
        }
        return Ok(None);
    };
    let Some(index_parameters) =
        crate::preparation::generic_data::domain_index_parameters(syntax, domain)
    else {
        return Ok(None);
    };
    if !index_parameters.is_empty() {
        return Err(format!(
            "indexed domain `{authored}` requires {} closed index argument(s), but 0 were supplied",
            index_parameters.len(),
        ));
    }
    // Keep the logical path for diagnostics; recursive replay additionally
    // retains the selected declaration's source occurrence below.
    let domain_key = module_domain_key(syntax, domain);
    evaluate_selected_domain_facts(
        syntax,
        domain,
        domain_key,
        carrier,
        subject_binding,
        const_values,
        &HashMap::new(),
        visiting,
        selection,
        warnings,
    )
}

/// The selected declaration's complete logical path — module prefix plus the
/// declared name. This label alone is not declaration identity: distinct
/// packages can publish identical logical paths.
fn module_domain_key(
    syntax: &SyntaxTrees,
    domain: &syntax_trees::item::DomainDefinition,
) -> String {
    match crate::preparation::generic_data::module_constants::module_path(
        syntax,
        domain.name.source_span().source_id,
    ) {
        Some(module) => format!("{module}::{}", domain.name.as_str(syntax)),
        None => domain.name.as_str(syntax).to_owned(),
    }
}

/// Whether the concrete const carrier satisfies every requirement a domain
/// declares on its abstract carrier parameter — the checked application
/// evidence the bound calls for. Builtin scalar carriers are `copy` and ride
/// the permissive carry policy; a named `data` carrier answers from its
/// authored `properties`. Any other carrier declines rather than guessing.
fn carrier_satisfies_declared_bounds(
    syntax: &SyntaxTrees,
    carrier: &str,
    bounds: &syntax_trees::item::DataProperties,
) -> bool {
    let properties = primitive_scalar_properties(carrier).or_else(|| {
        syntax.root_items().find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str(syntax) == carrier => {
                Some(definition.properties)
            }
            _ => None,
        })
    });
    let Some(properties) = properties else {
        return false;
    };
    match bounds.multiplicity {
        language_core::Multiplicity::Unrestricted
            if properties.multiplicity != language_core::Multiplicity::Unrestricted =>
        {
            return false;
        }
        language_core::Multiplicity::Linear
            if properties.multiplicity != language_core::Multiplicity::Linear =>
        {
            return false;
        }
        _ => {}
    }
    if let Some(required) = bounds.carry {
        let effective = properties
            .carry
            .unwrap_or(language_core::CarryPolicy::STRICT);
        if !effective.permits(required) {
            return false;
        }
    }
    true
}

/// The declared properties every builtin scalar spelling answers: primitives
/// copy freely and carry every axis permissively.
fn primitive_scalar_properties(name: &str) -> Option<syntax_trees::item::DataProperties> {
    crate::constant::is_builtin_scalar_carrier(name).then_some(syntax_trees::item::DataProperties {
        multiplicity: language_core::Multiplicity::Unrestricted,
        carry: Some(language_core::CarryPolicy::PERMISSIVE),
    })
}

/// Replay one already-selected domain declaration's facts against `value`
/// with the subject bound and the application's closed index binders mapped in
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
    subject_binding: ConstSubjectBinding<'_>,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, ConstScalarValue>,
    visiting: &mut Vec<(source::SourceSpan, String)>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<DomainFactProof>, String> {
    if visiting
        .iter()
        .any(|(span, name)| *span == domain.name.source_span() && name == &domain_key)
    {
        return Ok(None);
    }
    // Predicate truth is not issuance, including through an alias.
    if !domain.authored_routes.is_empty() {
        return Ok(None);
    }
    let domain_target_name = match syntax.type_references.type_reference(domain.target_type) {
        TypeReferenceNode::Named(name) => Some(name.as_str(syntax)),
        _ => None,
    };
    let carrier_parameter = syntax
        .items
        .type_parameters(domain.type_parameters)
        .first()
        .filter(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Type)
                && domain_target_name == Some(parameter.name.as_str(syntax))
        });
    let binding = if let Some(parameter) = carrier_parameter {
        // An authored bound on the abstract carrier is application evidence:
        // check the concrete carrier's declared properties instead of
        // declining outright. An unsatisfied or unprovable bound keeps the
        // fence, and the subject still binds nothing, so facts that read the
        // abstract carrier keep needing typed application checking while
        // closed carrier-independent predicates reuse ordinary replay.
        if parameter.bounds != syntax_trees::item::DataProperties::default()
            && !carrier_satisfies_declared_bounds(syntax, carrier, &parameter.bounds)
        {
            return Ok(None);
        }
        None
    } else {
        let domain_carrier = match syntax.type_references.type_reference(domain.target_type) {
            TypeReferenceNode::Named(name) => name.as_str(syntax).to_owned(),
            // A fixed-array target's normalized identity matches the canonical
            // carrier label `[element; length]` byte for byte.
            TypeReferenceNode::FixedArray { .. } => {
                match crate::preparation::generic_data::syntax_type_identity(
                    syntax,
                    domain.target_type,
                ) {
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
        Some(subject_binding)
    };
    let subject = domain.subject.as_ref().map(|name| ConstFactSubject::Value {
        name: name.as_str(syntax),
        binding,
    });
    let warning_start = warnings.len();
    visiting.push((domain.name.source_span(), domain_key));
    let result = (|| {
        if let Some(alias) = &domain.alias {
            if alias.constituents.is_empty() {
                return Ok(None);
            }
            // Discharge conjunction obligations without rewriting the alias or
            // manufacturing normalized identity. The typed normalizer still
            // expands its retained constituents and checks publication legality.
            // Each edge is selected in its author's source, never the caller's.
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
                    subject_binding,
                    const_values,
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
                const_values,
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
                    domain: module_domain_key(syntax, domain),
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

/// Bind one evaluated nested-membership operand as the subject payload the next
/// selected domain replays. A Boolean operand always selects the `bool`
/// carrier; an integer operand keeps the enclosing carrier, which
/// the subject-derived expressions inherit. `Ok(None)` leaves the membership on the
/// checked-record path rather than binding a guessed payload.
fn nested_membership_operand<'a>(
    syntax: &SyntaxTrees,
    value: ConstFactValue,
    carrier: &'a str,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<(&'a str, ConstScalarValue)>, String> {
    match value {
        ConstFactValue::Boolean(value) => Ok(Some(("bool", ConstScalarValue::Boolean(value)))),
        ConstFactValue::DeclaredInteger {
            value,
            integer_type,
        } => Ok(Some((
            integer_type.name(),
            ConstScalarValue::DeclaredInteger {
                value,
                integer_type,
            },
        ))),
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
/// use, with the subject bound to `value` and each binder mapped to its argument.
/// A non-scalar or non-const index parameter,
/// an argument that stays open (enclosing binders, module-constant spellings,
/// names still owned by resolved selection), and an unprovable fact all
/// decline with `None` so the declaration keeps its fence instead of
/// discharging against a guessed binding.
fn evaluate_indexed_const_domain(
    syntax: &SyntaxTrees,
    authored: &str,
    argument_span: HandleSpan<TypeReferenceHandle>,
    carrier: &str,
    subject_binding: ConstSubjectBinding<'_>,
    const_values: &HashMap<String, i128>,
    argument_values: &HashMap<String, ConstScalarValue>,
    argument_parameters: &[syntax_trees::item::TypeParameter],
    visiting: &mut Vec<(source::SourceSpan, String)>,
    reference: source::SourceSpan,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<DomainFactProof>, String> {
    let domain = match selection {
        Some(selection) => selection
            .domain_family(syntax, authored, reference, Some(carrier))
            .map(|(_, definition)| definition),
        None => {
            // Source-free canonicalization has no module custody, so the
            // fallback may only select an unmoduled family declared exactly
            // once — the same restriction unindexed discharge applies.
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
    let Some(index_parameters) =
        crate::preparation::generic_data::domain_index_parameters(syntax, domain)
    else {
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
        let bound = match bound {
            ConstScalarValue::Integer(bound) => {
                if required == "bool" {
                    return Err(format!(
                        "index argument for `{authored}::{}` is an integer, expected `bool`",
                        parameter.name.as_str(syntax),
                    ));
                }
                crate::preparation::generic_data::validate_syntax_integer_range(&required, bound)?;
                let Some(integer_type) = LandedIntegerType::from_name(&required) else {
                    return Ok(None);
                };
                ConstScalarValue::DeclaredInteger {
                    value: bound,
                    integer_type,
                }
            }
            ConstScalarValue::DeclaredInteger {
                value,
                integer_type,
            } => {
                if integer_type.name() != required {
                    return Err(format!(
                        "index argument for `{authored}::{}` has canonical type `{}`, expected `{required}`",
                        parameter.name.as_str(syntax),
                        integer_type.name(),
                    ));
                }
                validate_declared_integer(integer_type, value)?;
                bound
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
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ConstScalarValue>, String> {
    let integer_type = crate::preparation::generic_data::const_integer_type(syntax, parameter_type);
    let boolean_type = matches!(
        syntax.type_references.type_reference(parameter_type),
        TypeReferenceNode::Named(name) if name.as_str(syntax) == "bool"
    );
    // A constrained binder cannot borrow its base carrier's eligibility:
    // its qualification still needs separate evidence.
    if integer_type.is_none() && !boolean_type {
        return Ok(None);
    }
    match syntax.type_references.type_reference(argument) {
        TypeReferenceNode::Named(name) => {
            match ConstScalarSpelling::from_bare_name(name.as_str(syntax)) {
                Some(ConstScalarSpelling::Canonical(atom)) => {
                    let required = crate::preparation::generic_data::syntax_type_identity(
                        syntax,
                        parameter_type,
                    )?;
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
                        validate_declared_integer(integer_type, value)?;
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
                // Forwarding binds a declared value, not an anonymous integer.
                // Retain the caller telescope until its carrier is checked;
                // reducing to i128 first would let `u64` silently become `u8`.
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
                let actual =
                    crate::preparation::generic_data::syntax_type_identity(syntax, type_reference)?;
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
            if let Some(value) = const_values.get(name.as_str(syntax)) {
                crate::preparation::generic_data::module_constants::reject_module_constant_selection(
                    syntax,
                    name.as_str(syntax),
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
                    ) && super::qualified_const_name(syntax, definition)
                        == name.as_str(syntax) =>
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
                name.as_str(syntax),
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
            if syntax.const_expression_contains_name(*expression)
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

/// One Domain constraint discharged over a bound const subject: `Holds`
/// passes silently, `Refuted` fails naming the caller's subject, and an
/// undecided fact fences rather than guessing the binding.
fn prove_const_domain_constraint(
    syntax: &SyntaxTrees,
    constraint: &TypeConstraintNode,
    carrier: &str,
    subject_binding: ConstSubjectBinding<'_>,
    const_values: &HashMap<String, i128>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
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
        const_values,
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

/// Discharge the `Domain` constraints a `Constrained` leaf carrier declares —
/// applied to each leaf a constrained field or
/// element type names as an initializer canonicalizes through
/// `canonicalize_const_expression`. Aggregate leaves bind the subject through their
/// scalar-decodable members exactly as the declaration path does; a leaf
/// without a bindable payload, an unselected owner, or an undecided fact stays
/// fenced rather than publishing a guessed identity. Constraints of other
/// kinds keep their own validation sites.
pub(in crate::preparation::generic_data) fn prove_const_leaf_domain_constraints(
    syntax: &SyntaxTrees,
    constraints: &[TypeConstraintNode],
    value: &super::values::CanonicalConstNode,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
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
        CanonicalConstNode::Integer { type_name, value } => {
            let Some(integer_type) = LandedIntegerType::from_name(type_name) else {
                return Err(CONSTRAINED_CONST_FENCE.to_owned());
            };
            (
                type_name.clone(),
                ConstSubjectBinding::Scalar(ConstScalarValue::DeclaredInteger {
                    value: *value,
                    integer_type,
                }),
            )
        }
        CanonicalConstNode::Boolean(value) => (
            "bool".to_owned(),
            ConstSubjectBinding::Scalar(ConstScalarValue::Boolean(*value)),
        ),
        CanonicalConstNode::Record { type_name, fields } => {
            record_fields = fields
                .iter()
                .filter_map(|(name, field)| {
                    node_subject_leaf(field).map(|value| (name.clone(), value))
                })
                .collect();
            (
                type_name.clone(),
                ConstSubjectBinding::Record(&record_fields),
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
            type_name, fields, ..
        } => {
            variant_fields = fields
                .iter()
                .filter_map(|(name, field)| {
                    node_subject_leaf(field).map(|value| (name.clone(), value))
                })
                .collect();
            (
                type_name.clone(),
                ConstSubjectBinding::Variant(&variant_fields),
            )
        }
        _ => return Err(CONSTRAINED_CONST_FENCE.to_owned()),
    };
    let const_values =
        crate::preparation::generic_data::module_constants::lexical_integer_const_values(syntax);
    let mut warnings = Vec::new();
    for constraint in constraints {
        prove_const_domain_constraint(
            syntax,
            constraint,
            &carrier,
            subject_binding,
            &const_values,
            selection,
            &mut warnings,
            "the const leaf value",
        )?;
    }
    Ok(())
}

/// Find declaration predicates through the exact selected member types. This
/// only selects a required check; it grants no value or membership. Recursion
/// rejoins source-owned declarations rather than guessing from a leaf spelling.
pub(crate) fn const_type_requires_declaration_proof(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<source::SourceSpan>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
) -> bool {
    use syntax_trees::item::DataMember;
    let origin = syntax
        .type_references
        .generic_application_origin(type_reference);
    if origin.is_valid() && origin != type_reference {
        return const_type_requires_declaration_proof(syntax, origin, visiting, selection);
    }
    fn named_data(
        syntax: &SyntaxTrees,
        name: &Identifier,
        visiting: &mut Vec<source::SourceSpan>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    ) -> bool {
        let Ok(data) = super::values::selected_data(syntax, name, selection) else {
            return false;
        };
        if visiting.contains(&data.name.source_span()) {
            return false;
        }
        visiting.push(data.name.source_span());
        let requires_proof =
            !data.where_facts.is_empty()
                || syntax.tables.items.data_members(data.members).iter().any(
                    |member| match member {
                        DataMember::Field(field) => const_type_requires_declaration_proof(
                            syntax,
                            field.type_reference,
                            visiting,
                            selection,
                        ),
                        DataMember::Variant(variant) => {
                            !variant.where_facts.is_empty()
                                || syntax
                                    .tables
                                    .items
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
            syntax
                .type_references
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
            syntax
                .type_references
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

/// A pure literal does not request initializer evaluation. Its selected data,
/// active-case and qualified-member predicates still owe the same construction
/// discharge as an evaluated value, even in an unused private declaration.
pub(crate) fn prove_const_literal_predicates(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
) -> Result<(), String> {
    if crate::constant::requires_const_initializer_evaluation(syntax, definition) {
        return Ok(());
    }
    if definition
        .qualification
        .as_ref()
        .is_some_and(|qualification| {
            qualification.destination == definition.type_reference
                && qualification.initializer == definition.value
        })
    {
        return super::super::canonicalize_selected_declared_const_definition(
            syntax, definition, selection,
        )
        .map(|_| ());
    }
    if !const_type_requires_declaration_proof(
        syntax,
        definition.type_reference,
        &mut Vec::new(),
        selection,
    ) {
        return Ok(());
    }
    super::canonicalize_const_expression(
        syntax,
        definition.type_reference,
        definition.value,
        selection,
        &crate::preparation::declaration_selection::GenericApplicationSubstitution::new(),
    )
    .map(|_| ())
}

/// Evaluate one fact expression inside a selected domain declaration with
/// the subject bound to the checked value only for a fixed carrier. A generic
/// carrier leaves it unavailable rather than assuming an instance's algebra.
/// `parameter_values` carries the
/// application's closed index bindings so a family fact like `subject < N`
/// resolves its binder exactly; a monomorphic domain passes an empty map.
fn evaluate_const_domain_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, ConstScalarValue>,
    subject: Option<ConstFactSubject<'_>>,
    carrier: &str,
    visiting: &mut Vec<(source::SourceSpan, String)>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
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
                const_values,
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
            let members = syntax
                .expressions
                .identifier_path_members(membership.domain);
            let path = members.joined_path_in(syntax);
            evaluate_named_const_domain(
                syntax,
                &path,
                nested_carrier,
                ConstSubjectBinding::Scalar(value),
                const_values,
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
            let builtin = has_builtin_const_operator(syntax, binary);
            if !builtin && subject.and_then(ConstFactSubject::scalar).is_some() {
                return Ok(None);
            }
            validate_anonymous_remainder(syntax, binary)?;
            let Some(left) = evaluate_const_domain_expression(
                syntax,
                binary.left,
                const_values,
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
                const_values,
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
            if !builtin && authored_operator_can_bind(syntax, binary.operator, &left, &right) {
                return Ok(None);
            }
            evaluate_const_fact_binary(syntax, expression, binary.operator, left, right, warnings)
                .map(Some)
        }
        _ => evaluate_const_fact_expression_at(
            syntax,
            expression,
            const_values,
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
