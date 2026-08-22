use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::StatementNode;

/// A conservative value interval (both ends optional).
#[derive(Clone, Copy)]
pub(super) struct Bounds {
    low: Option<i64>,
    high: Option<i64>,
    symbol: psi_symbols::SymbolHandle,
    length: Option<i64>,
    capacity: Option<i64>,
}

impl Bounds {
    const UNKNOWN: Bounds = Bounds {
        low: None,
        high: None,
        symbol: psi_symbols::SymbolHandle::invalid(),
        length: None,
        capacity: None,
    };
    fn point(value: i64) -> Bounds {
        Bounds {
            low: Some(value),
            high: Some(value),
            symbol: psi_symbols::SymbolHandle::invalid(),
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
        ExpressionNode::String(literal) => Bounds::sequence(literal.as_bytes().len()),
        ExpressionNode::Integer(value) => value
            .text()
            .parse::<i64>()
            .map(Bounds::point)
            .unwrap_or(Bounds::UNKNOWN),
        ExpressionNode::Mutable(inner) => value_bounds(program, machine, state, *inner),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            // RAW keeps the Constrained shell that carries the declared
            // range (the unwrapping variant strips it).
            let Some(handle) =
                crate::places::declared_place_type_raw(program, machine, Some(state), expression)
            else {
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
                match crate::arithmetic_domains::range_constraint_interval(program, handle) {
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
                    ExpressionNode::String(literal) => {
                        Some(Bounds::sequence(literal.as_bytes().len()))
                    }
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
    expression: ExpressionHandle,
) -> Truth {
    let bounds = bounds_eval(program, valuation, expression);
    match (bounds.low, bounds.high) {
        (Some(low), _) if low >= 1 => Truth::True,
        (_, Some(high)) if high <= 0 => Truth::False,
        _ => Truth::Unknown,
    }
}

fn bounds_eval(
    program: &TypedTrees,
    valuation: &[(&str, Bounds)],
    expression: ExpressionHandle,
) -> Bounds {
    use psi_typed_trees::expression::BinaryOperator;
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
                // Omitted fields read the ZII zero at construction.
                .unwrap_or(Bounds::point(0))
        }
        ExpressionNode::Integer(value) => value
            .text()
            .parse::<i64>()
            .map(Bounds::point)
            .unwrap_or(Bounds::UNKNOWN),
        ExpressionNode::Member(member) if matches!(member.member.as_str(), "len" | "capacity") => {
            let measure = match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path) => program
                    .expression_table
                    .name_path_members(path.members)
                    .last()
                    .and_then(|name| {
                        match valuation.iter().find(|(field, _)| *field == name.as_str()) {
                            Some((_, bounds)) => match member.member.as_str() {
                                "len" => bounds.length,
                                "capacity" => bounds.capacity,
                                _ => None,
                            },
                            // An omitted sequence field has the ZII empty value.
                            None => Some(0),
                        }
                    }),
                _ => None,
            };
            measure.map(Bounds::point).unwrap_or(Bounds::UNKNOWN)
        }
        ExpressionNode::Binary(binary) => {
            let left = bounds_eval(program, valuation, binary.left);
            let right = bounds_eval(program, valuation, binary.right);
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
        ExpressionNode::Mutable(inner) => bounds_eval(program, valuation, *inner),
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
