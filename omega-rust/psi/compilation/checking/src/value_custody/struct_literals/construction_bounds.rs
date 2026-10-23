//! Constructor predicates consume the exact field valuation and declared scope.
//! Source literals use ordered captured operands through the ordinary structural
//! judge, before the constructed result can donate its own invariant. Entry zero
//! provisioning has no executed occurrence: its conservative fold supplies only
//! independently permitted primitive zeros. Neither route grants qualification
//! from the presence of a clause.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::JoinedPath;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

/// A conservative value interval (both ends optional).
#[derive(Clone, Copy)]
pub(super) struct Bounds {
    interval: crate::proof_contracts::arithmetic_domains::Interval,
    length: Option<i64>,
    capacity: Option<i64>,
    float_primitive: Option<PrimitiveType>,
    float_interval: Option<(f64, f64)>,
}

impl Bounds {
    const UNKNOWN: Bounds = Bounds {
        interval: crate::proof_contracts::arithmetic_domains::Interval::UNBOUNDED,
        length: None,
        capacity: None,
        float_primitive: None,
        float_interval: None,
    };
    fn point(value: i64) -> Bounds {
        Bounds {
            interval: crate::proof_contracts::arithmetic_domains::Interval::constant(value),
            ..Bounds::UNKNOWN
        }
    }
    fn floating(primitive: PrimitiveType, interval: Option<(f64, f64)>) -> Bounds {
        Bounds {
            float_primitive: Some(primitive),
            float_interval: interval,
            ..Bounds::UNKNOWN
        }
    }

    fn zero(primitive: Option<PrimitiveType>) -> Bounds {
        match primitive {
            Some(primitive @ (PrimitiveType::F32 | PrimitiveType::F64)) => {
                Bounds::floating(primitive, Some((0.0, 0.0)))
            }
            Some(primitive) if primitive.is_fixed_integer() || primitive == PrimitiveType::Bool => {
                Bounds::point(0)
            }
            _ => Bounds::UNKNOWN,
        }
    }
}

pub(super) enum Truth {
    True,
    False,
    Unknown,
}

/// Fold a `where` fact over the field-value intervals. Comparisons yield a
/// TRI-STATE truth encoded as bounds ([1,1] true / [0,0] false / [0,1]
/// unknown) so `&&`/`||` compose. Arithmetic drops unrepresentable interval
/// endpoints: saturating a mathematical expression would invent a proof.
pub(super) fn bounds_fold(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    valuation: &[(symbols::SymbolHandle, Bounds)],
    declared: &[symbols::SymbolHandle],
    expression: ExpressionHandle,
) -> Truth {
    let bounds = bounds_eval(program, owner, valuation, declared, expression);
    match (bounds.interval.low, bounds.interval.high) {
        (Some(low), _) if low >= 1 => Truth::True,
        (_, Some(high)) if high <= 0 => Truth::False,
        _ => Truth::Unknown,
    }
}

/// Judge the active case's predicates at the actual zero field valuation.
/// Entry provisioning has no source evaluation to observe. The scalar fold
/// judges actual zeros, never the clause's presence as a positive certificate.
/// Only declared primitive fields have a scalar zero; aggregate observations,
/// membership and proof applications require their own evidence and decline.
pub fn case_predicates_admit_zero(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    variant: &typed_trees::data::DataVariant,
) -> bool {
    let common = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            typed_trees::data::DataMember::Variant(_) => None,
        });
    let fields = common.chain(program.data_payload_fields(variant));
    let mut declared = Vec::new();
    let mut valuation = Vec::new();
    for field in fields {
        declared.push(field.symbol);
        let bounds = Bounds::zero(program.primitive_type_reference(field.type_reference));
        valuation.push((field.symbol, bounds));
    }
    program
        .proof_facts
        .span_or_empty(variant.where_facts)
        .iter()
        .all(|fact| {
            fact.as_expression().is_some_and(|expression| {
                matches!(
                    bounds_fold(
                        program,
                        definition.symbol,
                        &valuation,
                        &declared,
                        *expression
                    ),
                    Truth::True
                )
            })
        })
}

fn bounds_eval(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    valuation: &[(symbols::SymbolHandle, Bounds)],
    declared: &[symbols::SymbolHandle],
    expression: ExpressionHandle,
) -> Bounds {
    use typed_trees::expression::BinaryOperator;
    // Embedding observes its operand's completed integer. The recursive fold
    // retains every selected runtime policy below this proof-only boundary.
    if let Some((_, argument)) =
        crate::proof_contracts::proof_embeddings::integer_embedding_argument(program, expression)
    {
        return bounds_eval(program, owner, valuation, declared, argument);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid() {
                return Bounds::UNKNOWN;
            }
            valuation
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)
                .map(|(_, bounds)| *bounds)
                // Omitted declared fields read the ZII zero at construction;
                // a name bound to nothing in scope is unprovable, not zero.
                .unwrap_or_else(|| {
                    if declared.contains(&path.symbol) {
                        Bounds::point(0)
                    } else {
                        Bounds::UNKNOWN
                    }
                })
        }
        ExpressionNode::Boolean(value) => Bounds::point(i64::from(*value)),
        ExpressionNode::Float(literal) => {
            let primitive = match literal.landing() {
                Some(numerics::literals::FloatFormat::F32) => PrimitiveType::F32,
                Some(numerics::literals::FloatFormat::F64) => PrimitiveType::F64,
                None => return Bounds::UNKNOWN,
            };
            let value = literal.landed_f64();
            Bounds::floating(primitive, (!value.is_nan()).then_some((value, value)))
        }
        ExpressionNode::Integer(value) => value
            .decimal_value_i128()
            .and_then(|value| i64::try_from(value).ok())
            .map(Bounds::point)
            .unwrap_or(Bounds::UNKNOWN),
        ExpressionNode::Member(member)
            if CollectionMeasure::from_authored_spelling(member.member.as_str()).is_some() =>
        {
            let measure = match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path) if path.symbol.is_valid() => {
                    match valuation.iter().find(|(field, _)| *field == path.symbol) {
                        Some((_, bounds)) => {
                            match CollectionMeasure::from_authored_spelling(member.member.as_str())
                            {
                                Some(CollectionMeasure::Length) => bounds.length,
                                Some(CollectionMeasure::Capacity) => bounds.capacity,
                                None => None,
                            }
                        }
                        // An omitted declared sequence field has the
                        // ZII empty value; a receiver outside the scope
                        // has no measure at all.
                        None => declared.contains(&path.symbol).then_some(0),
                    }
                }
                _ => None,
            };
            measure.map(Bounds::point).unwrap_or(Bounds::UNKNOWN)
        }
        ExpressionNode::Binary(binary) => {
            let operand_type = |expression| {
                crate::value_custody::expression_types::declaration_expression_result_type_reference(
                    program,
                    owner,
                    &|expression| {
                        crate::proof_contracts::proof_embeddings::expression_type_reference(
                            program, expression,
                        )
                    },
                    expression,
                )
            };
            let references = [binary.left, binary.right].map(operand_type);
            let left = bounds_eval(program, owner, valuation, declared, binary.left);
            let right = bounds_eval(program, owner, valuation, declared, binary.right);
            // An IEEE operand never enters integer interval or symbol-reflexivity
            // rules. Unknown ordered bounds may include NaN; x == x is not a proof.
            // Literal landing and live enclosures come from the ordinary producer,
            // while the selected comparison alone grants its order semantics.
            if left.float_primitive.is_some() || right.float_primitive.is_some() {
                if left.float_primitive != right.float_primitive
                    || !crate::has_builtin_or_sealed_float_comparison_meaning(
                        program,
                        owner,
                        expression,
                        binary.operator,
                        &references,
                    )
                {
                    return Bounds::UNKNOWN;
                }
                return tri(float_comparison(left, right, binary.operator));
            }
            if let Some(spelling) =
                crate::proof_contracts::parameter_expression_meaning::spelled_binary_operator(
                    binary.operator,
                )
                && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner,
                    expression,
                    spelling,
                    &references,
                )
            {
                return Bounds::UNKNOWN;
            }
            match binary.operator {
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply => {
                    let interval = if crate::proof_integer_expression(program, expression)
                        || crate::anonymous_integer_math_literal(program, expression).is_some()
                    {
                        match binary.operator {
                            BinaryOperator::Add => left.interval.add(right.interval),
                            BinaryOperator::Subtract => left.interval.subtract(right.interval),
                            _ => left.interval.multiply(right.interval),
                        }
                    } else {
                        let Some(reference) = operand_type(expression) else {
                            return Bounds::UNKNOWN;
                        };
                        let Some(primitive) = program.primitive_type_reference(reference) else {
                            return Bounds::UNKNOWN;
                        };
                        let policy = program.arithmetic_domain_for_type_reference(reference);
                        let Some(kind) =
                            crate::checked_integer_binary_kind(binary.operator, policy)
                        else {
                            return Bounds::UNKNOWN;
                        };
                        let range = |bounds: Bounds| {
                            Some(facts::IntegerRange {
                                minimum: numerics::bignum::BigInt::from_i64(bounds.interval.low?),
                                maximum: numerics::bignum::BigInt::from_i64(bounds.interval.high?),
                            })
                        };
                        let Some((left, right)) = range(left).zip(range(right)) else {
                            return Bounds::UNKNOWN;
                        };
                        let Some(result) = crate::proof_contracts::arithmetic_domains::selected_integer_binary_interval(kind, primitive, primitive, left, right) else { return Bounds::UNKNOWN; };
                        crate::proof_contracts::arithmetic_domains::Interval {
                            low: result.minimum.to_i64(),
                            high: result.maximum.to_i64(),
                        }
                    };
                    Bounds {
                        interval,
                        ..Bounds::UNKNOWN
                    }
                }
                BinaryOperator::LessOrEqual => tri(compare(left, right, |a, b| a <= b)),
                BinaryOperator::Less => tri(compare(left, right, |a, b| a < b)),
                BinaryOperator::GreaterOrEqual => tri(compare(right, left, |a, b| a <= b)),
                BinaryOperator::Greater => tri(compare(right, left, |a, b| a < b)),
                BinaryOperator::Equal => tri(equality(left, right, true)),
                BinaryOperator::NotEqual => tri(equality(left, right, false)),
                BinaryOperator::And => tri(truth_and(to_truth(left), to_truth(right))),
                BinaryOperator::Or => tri(truth_or(to_truth(left), to_truth(right))),
                _ => Bounds::UNKNOWN,
            }
        }
        ExpressionNode::Borrow(inner) => {
            bounds_eval(program, owner, valuation, declared, inner.target)
        }
        _ => Bounds::UNKNOWN,
    }
}

/// Only ordered enclosures enter this judgment. Endpoint comparisons preserve
/// IEEE order (including equal signed zeros); no integer complement or arithmetic
/// approximation supplies a missing enclosure, format or selected operator.
fn float_comparison(
    left: Bounds,
    right: Bounds,
    operator: typed_trees::expression::BinaryOperator,
) -> Truth {
    use typed_trees::expression::BinaryOperator;
    let Some(((left_low, left_high), (right_low, right_high))) =
        left.float_interval.zip(right.float_interval)
    else {
        return Truth::Unknown;
    };
    let relation = |true_for_all: bool, false_for_all: bool| {
        if true_for_all {
            Truth::True
        } else if false_for_all {
            Truth::False
        } else {
            Truth::Unknown
        }
    };
    match operator {
        BinaryOperator::LessOrEqual => relation(left_high <= right_low, left_low > right_high),
        BinaryOperator::Less => relation(left_high < right_low, left_low >= right_high),
        BinaryOperator::GreaterOrEqual => relation(left_low >= right_high, left_high < right_low),
        BinaryOperator::Greater => relation(left_low > right_high, left_high <= right_low),
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            let equal = left_low == left_high && right_low == right_high && left_low == right_low;
            let disjoint = left_high < right_low || right_high < left_low;
            if operator == BinaryOperator::Equal {
                relation(equal, disjoint)
            } else {
                relation(disjoint, equal)
            }
        }
        _ => Truth::Unknown,
    }
}

/// `left OP right` decided from interval ends: definitely true when every
/// left value relates to every right value; definitely false when none do.
fn compare(left: Bounds, right: Bounds, relates: fn(i64, i64) -> bool) -> Truth {
    if let (Some(left_high), Some(right_low)) = (left.interval.high, right.interval.low)
        && relates(left_high, right_low)
    {
        return Truth::True;
    }
    if let (Some(left_low), Some(right_high)) = (left.interval.low, right.interval.high)
        && !relates(left_low, right_high)
    {
        return Truth::False;
    }
    Truth::Unknown
}

fn equality(left: Bounds, right: Bounds, wants_equal: bool) -> Truth {
    // Equal iff both are the SAME point; definitely unequal iff the
    // intervals are disjoint.
    let same_point = left.interval.low == left.interval.high
        && right.interval.low == right.interval.high
        && left.interval.low.is_some()
        && left.interval.low == right.interval.low;
    let disjoint = matches!((left.interval.high, right.interval.low), (Some(a), Some(b)) if a < b)
        || matches!((right.interval.high, left.interval.low), (Some(a), Some(b)) if a < b);
    match (same_point, disjoint, wants_equal) {
        (true, _, true) | (_, true, false) => Truth::True,
        (true, _, false) | (_, true, true) => Truth::False,
        _ => Truth::Unknown,
    }
}

fn to_truth(bounds: Bounds) -> Truth {
    match (bounds.interval.low, bounds.interval.high) {
        (Some(low), _) if low >= 1 => Truth::True,
        (_, Some(high)) if high <= 0 => Truth::False,
        _ => Truth::Unknown,
    }
}

fn truth_and(left: Truth, right: Truth) -> Truth {
    match (left, right) {
        (Truth::False, _) | (_, Truth::False) => Truth::False,
        (Truth::True, Truth::True) => Truth::True,
        _ => Truth::Unknown,
    }
}

fn truth_or(left: Truth, right: Truth) -> Truth {
    match (left, right) {
        (Truth::True, _) | (_, Truth::True) => Truth::True,
        (Truth::False, Truth::False) => Truth::False,
        _ => Truth::Unknown,
    }
}

fn tri(truth: Truth) -> Bounds {
    match truth {
        Truth::True => Bounds::point(1),
        Truth::False => Bounds::point(0),
        Truth::Unknown => Bounds {
            interval: crate::proof_contracts::arithmetic_domains::Interval {
                low: Some(0),
                high: Some(1),
            },
            ..Bounds::UNKNOWN
        },
    }
}

/// Prove the owner's default domain on the ordered operand snapshots before
/// producing the aggregate. Field omission retains its independent zero gate.
pub(super) fn validate_literal_default_domain(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    statement_index: usize,
    literal: &TableStructLiteral,
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if data_definition.where_facts.is_empty() {
        return;
    }
    // A type-wide fact may read only the data's common fields; case payloads
    // are indexed by the active case and are never in scope here.
    let declared = common_field_symbols(program, data_definition);
    validate_literal_facts(
        program,
        machine,
        state,
        expression,
        statement_index,
        literal,
        data_definition.symbol,
        data_definition.where_facts,
        &declared,
        &format!("data `{}`", literal.type_name.as_str()),
        "the default domain",
        "default-domain",
        diagnostics,
    );
}

/// CASE-CONSTRAINTS (ch12 active-case indexed constraint): the SELECTED
/// case's `where` facts fold over the same literal field valuation -- a case
/// literal's named fields ARE the payload bindings, and omitted payload
/// fields read the ZII zero exactly like omitted common fields.
pub(super) fn validate_literal_case_constraints(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    statement_index: usize,
    literal: &TableStructLiteral,
    data_definition: &DataDefinition,
    case_name: &str,
    variant: &typed_trees::data::DataVariant,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if variant.where_facts.is_empty() {
        return;
    }
    // In scope for a case's facts: that case's payload bindings plus the
    // data's common fields. A sibling case's payload -- or a name bound to
    // nothing at all -- is not a field the literal can supply and must not
    // silently read the ZII zero.
    let mut declared = common_field_symbols(program, data_definition);
    declared.extend(
        program
            .data_payload_fields(variant)
            .iter()
            .map(|field| field.symbol),
    );
    let mut out_of_scope = Vec::new();
    for fact in program.proof_facts.span_or_empty(variant.where_facts) {
        if let Some(expression) = fact.as_expression() {
            collect_out_of_scope_leaf_names(program, &declared, *expression, &mut out_of_scope);
        }
    }
    if !out_of_scope.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` case `{case_name}` constraint names `{}`, which is not in scope: \
             a case `where` fact may read the case's payload bindings, common fields and \
             lexically available contract names (a sibling case's payload is never in scope)",
            literal.type_name.as_str(),
            out_of_scope.join("`, `"),
        )));
        return;
    }
    validate_literal_facts(
        program,
        machine,
        state,
        expression,
        statement_index,
        literal,
        data_definition.symbol,
        variant.where_facts,
        &declared,
        &format!("data `{}` case `{case_name}`", literal.type_name.as_str()),
        "the case constraint",
        "case",
        diagnostics,
    );
}

/// The identities of `data`'s common (non-case) fields.
fn common_field_symbols(
    program: &dyn crate::CheckingInput,
    data_definition: &DataDefinition,
) -> Vec<symbols::SymbolHandle> {
    program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field.symbol),
            typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect()
}

/// Resolved field roots belong to the common or active payload roster.
/// Descendant member selectors have their own declared owners; lexical names
/// retain their resolved identity instead of being mistaken for omitted fields.
fn collect_out_of_scope_leaf_names(
    program: &dyn crate::CheckingInput,
    declared: &[symbols::SymbolHandle],
    expression: ExpressionHandle,
    out_of_scope: &mut Vec<String>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some(last) = program
                .expression_table
                .name_path_members(path.members)
                .last()
                && (!path.symbol.is_valid()
                    || (program.symbols.get(path.symbol).kind == symbols::SymbolKind::Field
                        && !declared.contains(&path.symbol)))
                && !out_of_scope.iter().any(|name| name == last.as_str())
            {
                out_of_scope.push(last.as_str().to_owned());
            }
        }
        node => crate::expression_traversal::children(program, node, |child| {
            collect_out_of_scope_leaf_names(program, declared, child, out_of_scope);
        }),
    }
}

fn validate_literal_facts(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    statement_index: usize,
    literal: &TableStructLiteral,
    owner: symbols::SymbolHandle,
    facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
    declared: &[symbols::SymbolHandle],
    subject: &str,
    fact_label: &str,
    fact_adjective: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(definition) = program.data_definition_by_symbol(owner) else {
        return;
    };
    let parameters = declared
        .iter()
        .filter_map(|symbol| {
            let field =
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Field(field) if field.symbol == *symbol => {
                            Some(field)
                        }
                        typed_trees::data::DataMember::Variant(variant) => program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.symbol == *symbol),
                        _ => None,
                    })?;
            Some(typed_trees::signature::StateParameter {
                symbol: field.symbol,
                name: field.name.clone(),
                type_reference: super::selected_construction_field_type(
                    program,
                    expression,
                    literal,
                    field.symbol,
                )
                .unwrap_or_default(),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    let predicates = program
        .proof_facts
        .span_or_empty(facts)
        .iter()
        .filter_map(|fact| fact.as_expression().copied())
        .collect::<Vec<_>>();
    let judgments = crate::proof_contracts::contract_entailment::construction_predicate_judgments(
        program,
        machine,
        state,
        statement_index,
        expression,
        literal,
        &parameters,
        &predicates,
    );
    let mut judgments = judgments.into_iter();
    let mut predicate_diagnostics = Vec::new();
    for fact in program.proof_facts.span_or_empty(facts) {
        match fact {
            typed_trees::domain::ProofFact::Expression(_) => {
                use crate::proof_contracts::contract_entailment::structural_judgment::StructuralJudgment;
                match judgments.next().unwrap_or(StructuralJudgment::Unknown) {
                    StructuralJudgment::Proven => {}
                    StructuralJudgment::Refuted => predicate_diagnostics.push(Diagnostic::error(format!(
                        "{subject} literal violates {fact_label}: a `where` \
                         fact evaluates FALSE at this construction (ch12: construction is \
                         the gate)"
                    ))),
                    StructuralJudgment::Unknown => predicate_diagnostics.push(Diagnostic::error(format!(
                        "{subject} literal cannot prove {fact_label}: a \
                         `where` fact is not established by the values observed at this \
                         construction -- establish the required bounds before constructing the value"
                    ))),
                }
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                let field_name =
                    crate::expression_traversal::name_path_last_member(program, membership.value);
                let authored_value = field_name.and_then(|wanted| {
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .find(|field| field.name.as_str() == wanted)
                        .map(|field| field.value)
                });
                let proven = authored_value.map_or_else(
                    || {
                        crate::proof_contracts::proof_facts::domain_admits_zero_bytes(
                            program,
                            membership.domain_symbol,
                        )
                    },
                    |value| {
                        crate::proof_contracts::proof_facts::string_literal_grants_domain(
                            program,
                            value,
                            membership.domain_symbol,
                        )
                    },
                );
                if !proven {
                    predicate_diagnostics.push(Diagnostic::error(format!(
                        "{subject} literal cannot prove {fact_adjective} fact: \
                         field `{}` is not known to satisfy domain `{}` at construction",
                        field_name.unwrap_or("<unknown>"),
                        membership_domain_label(program, membership.domain),
                    )));
                }
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                predicate_diagnostics.push(Diagnostic::error(format!(
                    "{subject} literal cannot prove {fact_adjective} proposition `{}` at construction",
                    application.name.as_str(),
                )));
            }
        }
    }
    let source_span = program.expression_table.source_span(expression);
    diagnostics.extend(
        predicate_diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.with_source_span(source_span)),
    );
}

fn membership_domain_label(
    program: &dyn crate::CheckingInput,
    domain: arena::HandleSpan<typed_trees::name::Identifier>,
) -> String {
    program.domain_path_members(domain).joined_path()
}
