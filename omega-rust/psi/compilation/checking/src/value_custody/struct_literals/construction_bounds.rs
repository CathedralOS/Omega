//! Constructor predicates consume the exact field valuation and declared scope.
//! Source literals supply their observed operands; entry zero provisioning
//! supplies primitive zeros and leaves unsupported aggregate facts unknown.
//! Both use one predicate fold, so a clause cannot become either a blanket
//! refusal or a qualification granted without establishing its facts.

use super::observations::ConstructionObservations;
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
    symbol: symbols::SymbolHandle,
    length: Option<i64>,
    capacity: Option<i64>,
    float_primitive: Option<PrimitiveType>,
    float_interval: Option<(f64, f64)>,
}

impl Bounds {
    const UNKNOWN: Bounds = Bounds {
        interval: crate::proof_contracts::arithmetic_domains::Interval::UNBOUNDED,
        symbol: symbols::SymbolHandle::invalid(),
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

/// Bounds belong to the field's evaluated value. The shared observation walk
/// retains prefix assignments and guards only across non-overlapping writes.
/// A sibling operand's later effects cannot change an already copied scalar.
fn value_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    expression: ExpressionHandle,
    destination: Option<PrimitiveType>,
) -> Bounds {
    if let Some(primitive @ (PrimitiveType::F32 | PrimitiveType::F64)) = destination {
        let environment = observations.environment(expression);
        let captured = crate::proof_contracts::arithmetic_domains::capture_float_assignment(
            program,
            machine,
            Some(state),
            &environment,
            Some(primitive),
            expression,
        );
        return Bounds::floating(
            primitive,
            captured.map(|value| (value.minimum, value.maximum)),
        );
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::String(literal) => return Bounds::sequence(literal.len()),
        ExpressionNode::Integer(value) => {
            return value
                .text()
                .parse::<i64>()
                .map(Bounds::point)
                .unwrap_or(Bounds::UNKNOWN);
        }
        ExpressionNode::Borrow(inner) => {
            return value_bounds(
                program,
                machine,
                state,
                observations,
                inner.target,
                destination,
            );
        }
        _ => {}
    }
    let environment = observations.environment(expression);
    let mut diagnostics = Vec::new();
    let interval = crate::proof_contracts::arithmetic_domains::validate_arithmetic_domains(
        program,
        machine,
        Some(state),
        expression,
        &environment,
        None,
        numerics::arithmetic::ArithmeticDomain::Exact,
        "construction value",
        &mut diagnostics,
    );
    let mut bounds = if diagnostics.is_empty() {
        Bounds {
            interval,
            ..Bounds::UNKNOWN
        }
    } else {
        Bounds::UNKNOWN
    };
    if let Some(path) = program.expression_table.name_path_node(expression) {
        bounds.symbol = path.symbol;
        // Sequence measures are not integer-value intervals. Preserve the
        // literal size only for an exact immutable declaration, never by name
        // matching or by replaying a mutable local's obsolete initializer.
        if let Some((_, local)) = program.state_local_declaration(state, path.symbol)
            && !local.is_mutable
            && let ExpressionNode::String(literal) =
                program.expression_table.expression(local.initial_value)
        {
            let sequence = Bounds::sequence(literal.len());
            bounds.length = sequence.length;
            bounds.capacity = sequence.capacity;
        }
    }
    bounds
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
/// Entry provisioning uses the same constructor fold, not the presence of a
/// clause or the conservative zero-gate flag as a positive certificate.
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
            .text()
            .parse::<i64>()
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
    let same_symbol = left.symbol.is_valid() && left.symbol == right.symbol;
    let disjoint = matches!((left.interval.high, right.interval.low), (Some(a), Some(b)) if a < b)
        || matches!((right.interval.high, left.interval.low), (Some(a), Some(b)) if a < b);
    match (same_point || same_symbol, disjoint, wants_equal) {
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

/// R2 rung 2b: fold every default-domain fact at the LITERAL's field
/// valuation. Field names read the literal's integer value (omitted -> 0);
/// literals, `+ - *`, comparisons, and `&&`/`||` fold. A fact that fails
/// refuses naming it; a fact that cannot fold (a runtime-valued field)
/// refuses as unverifiable.
pub(super) fn validate_literal_default_domain(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
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
    fold_literal_facts(
        program,
        machine,
        state,
        observations,
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
    observations: &ConstructionObservations<'_>,
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
        observations,
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

/// Collect the leaf names one fact expression consults that are not declared
/// in scope: `Name` leaves and the receiver of a `len`/`capacity` measure,
/// mirroring the leaf shapes `bounds_eval` resolves against the literal's
/// field valuation. Other leaf kinds carry no field name.
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
                && !declared.contains(&path.symbol)
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    literal: &TableStructLiteral,
    owner: symbols::SymbolHandle,
    facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
    declared: &[symbols::SymbolHandle],
    subject: &str,
    fact_label: &str,
    fact_adjective: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Snapshot each field at its own source observation, including operand effects.
    let mut valuation = Vec::new();
    for field in program.expression_table.struct_fields(literal.fields) {
        let destination =
            super::selected_construction_field_type(program, literal, field.field_symbol)
                .and_then(|reference| program.primitive_type_reference(reference));
        let value = value_bounds(
            program,
            machine,
            state,
            observations,
            field.value,
            destination,
        );
        valuation.push((field.field_symbol, value));
    }
    // Omitted IEEE fields contribute their format's zero, never an integer
    // stand-in. Other omitted values retain their existing zero/measure rules.
    for symbol in declared {
        if !valuation.iter().any(|(field, _)| field == symbol) {
            let primitive = super::selected_construction_field_type(program, literal, *symbol)
                .and_then(|reference| program.primitive_type_reference(reference));
            if matches!(primitive, Some(PrimitiveType::F32 | PrimitiveType::F64)) {
                valuation.push((*symbol, Bounds::zero(primitive)));
            }
        }
    }
    for fact in program.proof_facts.span_or_empty(facts) {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                match bounds_fold(program, owner, &valuation, declared, *expression) {
                    Truth::True => {}
                    Truth::False => diagnostics.push(Diagnostic::error(format!(
                        "{subject} literal violates {fact_label}: a `where` \
                         fact evaluates FALSE at this construction (ch12: construction is \
                         the gate)"
                    ))),
                    Truth::Unknown => diagnostics.push(Diagnostic::error(format!(
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

fn membership_domain_label(
    program: &dyn crate::CheckingInput,
    domain: arena::HandleSpan<typed_trees::name::Identifier>,
) -> String {
    program.domain_path_members(domain).joined_path()
}
