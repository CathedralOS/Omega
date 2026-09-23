use diagnostics::Diagnostic;
use language_semantics::declaration_selection::CollectionMeasure;
use std::collections::BTreeSet;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// A conservative value interval (both ends optional).
#[derive(Clone, Copy)]
pub(super) struct Bounds {
    low: Option<i64>,
    high: Option<i64>,
    symbol: symbols::SymbolHandle,
    length: Option<i64>,
    capacity: Option<i64>,
}

impl Bounds {
    const UNKNOWN: Bounds = Bounds {
        low: None,
        high: None,
        symbol: symbols::SymbolHandle::invalid(),
        length: None,
        capacity: None,
    };
    fn point(value: i64) -> Bounds {
        Bounds {
            low: Some(value),
            high: Some(value),
            symbol: symbols::SymbolHandle::invalid(),
            length: None,
            capacity: None,
        }
    }

    fn sequence(byte_length: usize) -> Bounds {
        let measure = i64::try_from(byte_length).ok();
        Bounds {
            length: measure,
            capacity: measure,
            ..Bounds::UNKNOWN
        }
    }
}

pub(super) enum Truth {
    True,
    False,
    Unknown,
}

/// Slice 9: a literal field VALUE's sound interval -- an integer literal is
/// a point; a Name/Member place with a declared range contributes that
/// range intersected with its primitive width; anything else is unknown.
pub(super) fn value_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Bounds {
    match program.expression_table.expression(expression) {
        ExpressionNode::String(literal) => Bounds::sequence(literal.len()),
        ExpressionNode::Integer(value) => value
            .text()
            .parse::<i64>()
            .map(Bounds::point)
            .unwrap_or(Bounds::UNKNOWN),
        ExpressionNode::Borrow(inner) => value_bounds(program, machine, state, inner.target),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            // RAW keeps the Constrained shell that carries the declared
            // range (the unwrapping variant strips it).
            let Some(handle) = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                expression,
            ) else {
                let mut bounds = Bounds::UNKNOWN;
                if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
                {
                    bounds.symbol = path.symbol;
                    if let Some(local) = local_initializer_bounds(program, state, expression) {
                        bounds.low = local.low;
                        bounds.high = local.high;
                        bounds.length = local.length;
                        bounds.capacity = local.capacity;
                    }
                }
                return bounds;
            };
            let mut bounds =
                match crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                    program, handle,
                ) {
                    Some(interval) => Bounds {
                        low: interval.low,
                        high: interval.high,
                        ..Bounds::UNKNOWN
                    },
                    None => Bounds::UNKNOWN,
                };
            if let ExpressionNode::Name(path) = program.expression_table.expression(expression) {
                bounds.symbol = path.symbol;
                if let Some(local) = local_initializer_bounds(program, state, expression) {
                    bounds.low = local.low;
                    bounds.high = local.high;
                    bounds.length = local.length;
                    bounds.capacity = local.capacity;
                }
            }
            bounds
        }
        _ => Bounds::UNKNOWN,
    }
}

fn local_initializer_bounds(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Bounds> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let name = program
        .expression_table
        .name_path_members(path.members)
        .last()?
        .as_str();
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local)
                if (path.symbol.is_valid() && local.symbol == path.symbol)
                    || local.name.as_str() == name =>
            {
                match program.expression_table.expression(local.initial_value) {
                    ExpressionNode::Integer(value) => {
                        value.text().parse::<i64>().ok().map(Bounds::point)
                    }
                    ExpressionNode::String(literal) => Some(Bounds::sequence(literal.len())),
                    _ => None,
                }
            }
            _ => None,
        })
}

/// Fold a `where` fact over the field-value intervals. Comparisons yield a
/// TRI-STATE truth encoded as bounds ([1,1] true / [0,0] false / [0,1]
/// unknown) so `&&`/`||` compose; arithmetic uses saturating interval ops.
pub(super) fn bounds_fold(
    program: &TypedTrees,
    valuation: &[(&str, Bounds)],
    declared: &BTreeSet<&str>,
    expression: ExpressionHandle,
) -> Truth {
    let bounds = bounds_eval(program, valuation, declared, expression);
    match (bounds.low, bounds.high) {
        (Some(low), _) if low >= 1 => Truth::True,
        (_, Some(high)) if high <= 0 => Truth::False,
        _ => Truth::Unknown,
    }
}

fn bounds_eval(
    program: &TypedTrees,
    valuation: &[(&str, Bounds)],
    declared: &BTreeSet<&str>,
    expression: ExpressionHandle,
) -> Bounds {
    use typed_trees::expression::BinaryOperator;
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let Some(last) = program
                .expression_table
                .name_path_members(path.members)
                .last()
            else {
                return Bounds::UNKNOWN;
            };
            valuation
                .iter()
                .find(|(name, _)| *name == last.as_str())
                .map(|(_, bounds)| *bounds)
                // Omitted declared fields read the ZII zero at construction;
                // a name bound to nothing in scope is unprovable, not zero.
                .unwrap_or_else(|| {
                    if declared.contains(last.as_str()) {
                        Bounds::point(0)
                    } else {
                        Bounds::UNKNOWN
                    }
                })
        }
        ExpressionNode::Integer(value) => value
            .text()
            .parse::<i64>()
            .map(Bounds::point)
            .unwrap_or(Bounds::UNKNOWN),
        ExpressionNode::Member(member)
            if CollectionMeasure::from_authored_spelling(member.member.as_str()).is_some() =>
        {
            let measure = match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path) => program
                    .expression_table
                    .name_path_members(path.members)
                    .last()
                    .and_then(|name| {
                        match valuation.iter().find(|(field, _)| *field == name.as_str()) {
                            Some((_, bounds)) => {
                                match CollectionMeasure::from_authored_spelling(
                                    member.member.as_str(),
                                ) {
                                    Some(CollectionMeasure::Length) => bounds.length,
                                    Some(CollectionMeasure::Capacity) => bounds.capacity,
                                    None => None,
                                }
                            }
                            // An omitted declared sequence field has the
                            // ZII empty value; a receiver outside the scope
                            // has no measure at all.
                            None => declared.contains(name.as_str()).then_some(0),
                        }
                    }),
                _ => None,
            };
            measure.map(Bounds::point).unwrap_or(Bounds::UNKNOWN)
        }
        ExpressionNode::Binary(binary) => {
            let left = bounds_eval(program, valuation, declared, binary.left);
            let right = bounds_eval(program, valuation, declared, binary.right);
            match binary.operator {
                BinaryOperator::Add => Bounds {
                    low: left.low.zip(right.low).map(|(a, b)| a.saturating_add(b)),
                    high: left.high.zip(right.high).map(|(a, b)| a.saturating_add(b)),
                    ..Bounds::UNKNOWN
                },
                BinaryOperator::Subtract => Bounds {
                    low: left.low.zip(right.high).map(|(a, b)| a.saturating_sub(b)),
                    high: left.high.zip(right.low).map(|(a, b)| a.saturating_sub(b)),
                    ..Bounds::UNKNOWN
                },
                BinaryOperator::Multiply => match (left.low, left.high, right.low, right.high) {
                    (Some(a), Some(b), Some(c), Some(d)) => {
                        let products = [
                            a.saturating_mul(c),
                            a.saturating_mul(d),
                            b.saturating_mul(c),
                            b.saturating_mul(d),
                        ];
                        Bounds {
                            low: products.iter().min().copied(),
                            high: products.iter().max().copied(),
                            ..Bounds::UNKNOWN
                        }
                    }
                    _ => Bounds::UNKNOWN,
                },
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
        ExpressionNode::Borrow(inner) => bounds_eval(program, valuation, declared, inner.target),
        _ => Bounds::UNKNOWN,
    }
}

/// `left OP right` decided from interval ends: definitely true when every
/// left value relates to every right value; definitely false when none do.
fn compare(left: Bounds, right: Bounds, relates: fn(i64, i64) -> bool) -> Truth {
    if let (Some(left_high), Some(right_low)) = (left.high, right.low)
        && relates(left_high, right_low)
    {
        return Truth::True;
    }
    if let (Some(left_low), Some(right_high)) = (left.low, right.high)
        && !relates(left_low, right_high)
    {
        return Truth::False;
    }
    Truth::Unknown
}

fn equality(left: Bounds, right: Bounds, wants_equal: bool) -> Truth {
    // Equal iff both are the SAME point; definitely unequal iff the
    // intervals are disjoint.
    let same_point = left.low == left.high
        && right.low == right.high
        && left.low.is_some()
        && left.low == right.low;
    let same_symbol = left.symbol.is_valid() && left.symbol == right.symbol;
    let disjoint = matches!((left.high, right.low), (Some(a), Some(b)) if a < b)
        || matches!((right.high, left.low), (Some(a), Some(b)) if a < b);
    match (same_point || same_symbol, disjoint, wants_equal) {
        (true, _, true) | (_, true, false) => Truth::True,
        (true, _, false) | (_, true, true) => Truth::False,
        _ => Truth::Unknown,
    }
}

fn to_truth(bounds: Bounds) -> Truth {
    match (bounds.low, bounds.high) {
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
            low: Some(0),
            high: Some(1),
            ..Bounds::UNKNOWN
        },
    }
}

/// R2 rung 2b: fold every default-domain fact at the LITERAL's field
/// valuation. Field names read the literal's integer value (omitted -> 0);
/// literals, `+ - *`, comparisons, and `&&`/`||` fold. A fact that fails
/// refuses naming it; a fact that cannot fold (a runtime-valued field)
/// refuses as unverifiable.
pub(super) fn validate_literal_default_domain(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if data_definition.where_facts.is_empty() {
        return;
    }
    // A type-wide fact may read only the data's common fields; case payloads
    // are indexed by the active case and are never in scope here.
    let declared = common_field_names(program, data_definition);
    fold_literal_facts(
        program,
        machine,
        state,
        literal,
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
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
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
    let mut declared = common_field_names(program, data_definition);
    declared.extend(
        program
            .data_payload_fields(variant)
            .iter()
            .map(|field| field.name.as_str()),
    );
    let mut out_of_scope = Vec::new();
    for fact in program.proof_facts.span_or_empty(variant.where_facts) {
        if let typed_trees::domain::ProofFact::Expression(expression) = fact {
            collect_out_of_scope_leaf_names(program, &declared, *expression, &mut out_of_scope);
        }
    }
    if !out_of_scope.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` case `{case_name}` constraint names `{}`, which is not in scope: \
             a case `where` fact may read only the case's payload bindings and \
             the data's common fields (a sibling case's payload is never in scope)",
            literal.type_name.as_str(),
            out_of_scope.join("`, `"),
        )));
        return;
    }
    fold_literal_facts(
        program,
        machine,
        state,
        literal,
        variant.where_facts,
        &declared,
        &format!("data `{}` case `{case_name}`", literal.type_name.as_str()),
        "the case constraint",
        "case",
        diagnostics,
    );
}

/// The names of `data`'s common (non-case) fields.
fn common_field_names<'a>(
    program: &'a TypedTrees,
    data_definition: &'a DataDefinition,
) -> BTreeSet<&'a str> {
    program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field.name.as_str()),
            typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect()
}

/// Collect the leaf names one fact expression consults that are not declared
/// in scope: `Name` leaves and the receiver of a `len`/`capacity` measure,
/// mirroring the leaf shapes `bounds_eval` resolves against the literal's
/// field valuation. Other leaf kinds carry no field name.
fn collect_out_of_scope_leaf_names(
    program: &TypedTrees,
    declared: &BTreeSet<&str>,
    expression: ExpressionHandle,
    out_of_scope: &mut Vec<String>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some(last) = program
                .expression_table
                .name_path_members(path.members)
                .last()
                && !declared.contains(last.as_str())
                && !out_of_scope.iter().any(|name| name == last.as_str())
            {
                out_of_scope.push(last.as_str().to_owned());
            }
        }
        ExpressionNode::Member(member)
            if CollectionMeasure::from_authored_spelling(member.member.as_str()).is_some() =>
        {
            collect_out_of_scope_leaf_names(program, declared, member.receiver, out_of_scope);
        }
        ExpressionNode::Binary(binary) => {
            collect_out_of_scope_leaf_names(program, declared, binary.left, out_of_scope);
            collect_out_of_scope_leaf_names(program, declared, binary.right, out_of_scope);
        }
        ExpressionNode::Borrow(inner) => {
            collect_out_of_scope_leaf_names(program, declared, inner.target, out_of_scope);
        }
        ExpressionNode::Unary(unary) => {
            collect_out_of_scope_leaf_names(program, declared, unary.operand, out_of_scope);
        }
        _ => {}
    }
}

fn fold_literal_facts(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
    declared: &BTreeSet<&str>,
    subject: &str,
    fact_label: &str,
    fact_adjective: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Slice 9: each field's value resolves to an INTERVAL -- an integer
    // literal is a point; a place with a declared `[a..=b]` range (a ranged
    // parameter, a range-refined field) contributes its DECLARED interval
    // (declared ranges always hold); anything else is unknown.
    let mut valuation: Vec<(&str, Bounds)> = Vec::new();
    for field in program.expression_table.struct_fields(literal.fields) {
        let value = value_bounds(program, machine, state, field.value);
        valuation.push((field.name.as_str(), value));
    }
    for fact in program.proof_facts.span_or_empty(facts) {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                match bounds_fold(program, &valuation, declared, *expression) {
                    Truth::True => {}
                    Truth::False => diagnostics.push(Diagnostic::error(format!(
                        "{subject} literal violates {fact_label}: a `where` \
                         fact evaluates FALSE at this construction (ch12: construction is \
                         the gate)"
                    ))),
                    Truth::Unknown => diagnostics.push(Diagnostic::error(format!(
                        "{subject} literal cannot prove {fact_label}: a \
                         `where`-mentioned field's value is neither a literal nor a \
                         declared-range place whose interval decides the fact -- spell a \
                         literal, or constrain the value's declared range"
                    ))),
                }
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                let field_name = membership_field_name(program, membership.value);
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
                        crate::proof_contracts::proof_facts::domain_admits_empty_bytes(
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
                    diagnostics.push(Diagnostic::error(format!(
                        "{subject} literal cannot prove {fact_adjective} fact: \
                         field `{}` is not known to satisfy domain `{}` at construction",
                        field_name.unwrap_or("<unknown>"),
                        membership_domain_label(program, membership.domain),
                    )));
                }
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                diagnostics.push(Diagnostic::error(format!(
                    "{subject} literal cannot prove {fact_adjective} proposition `{}` at construction",
                    application.name.as_str(),
                )));
            }
        }
    }
}

fn membership_field_name(program: &TypedTrees, value: ExpressionHandle) -> Option<&str> {
    let ExpressionNode::Name(path) = program.expression_table.expression(value) else {
        return None;
    };
    program
        .expression_table
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str())
}

fn membership_domain_label(
    program: &TypedTrees,
    domain: arena::HandleSpan<typed_trees::name::Identifier>,
) -> String {
    program
        .domain_path_members(domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

/// Whether a declared literal field omitted by the author reads a composed
/// zero-initialized value. Primitive scalars mint a constant-zero leaf, a
/// closed array whose elements zero recursively mints a zero array, and a
/// field-only record mints a record of recursively zero fields. Everything
/// else — sums and mixed contents, erased members, byte sequences, slices,
/// references, providers — has no single composed zero form and must be
/// spelled by the author.
pub fn zero_initialized_field_supported(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    fn visit(
        program: &TypedTrees,
        type_reference: typed_trees::types::TypeReferenceHandle,
        visiting: &mut Vec<symbols::SymbolHandle>,
    ) -> bool {
        let Some(reference) =
            crate::value_custody::places::unwrapped_type_reference(program, type_reference)
        else {
            return false;
        };
        fn scalar_leaf(
            program: &TypedTrees,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            let Some(reference) =
                crate::value_custody::places::unwrapped_type_reference(program, type_reference)
            else {
                return false;
            };
            if program.primitive_type_reference(reference).is_some() {
                return true;
            }
            match program.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
                    scalar_leaf(program, *element_type)
                }
                _ => false,
            }
        }

        if program.primitive_type_reference(reference).is_some() {
            return true;
        }
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
                // A closed array's zero is one scalar constant leaf per
                // roster element; elements that only zero as structures have
                // no scalar-leaf spelling.
                scalar_leaf(program, *element_type)
            }
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => {
                if visiting.contains(symbol) {
                    // An owned field cycle cannot compose a finite zero.
                    return false;
                }
                let Some(data) = program
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *symbol)
                else {
                    return false;
                };
                let members = program.data_members(data);
                if members
                    .iter()
                    .any(|member| !matches!(member, typed_trees::data::DataMember::Field(_)))
                {
                    return false;
                }
                visiting.push(*symbol);
                let supported = members.iter().all(|member| {
                    let typed_trees::data::DataMember::Field(field) = member else {
                        return false;
                    };
                    !field.relevance.is_erased() && visit(program, field.type_reference, visiting)
                });
                visiting.pop();
                supported
            }
            _ => false,
        }
    }
    visit(program, type_reference, &mut Vec::new())
}
