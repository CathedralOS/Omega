//! Invocation-owned exact storage projections; no live-flow facts or destination type guesses.

use super::contextual_projection::contextual_canonical_place_from_expression;
use super::member_resolution::effective_member_symbol;

use super::declared_symbols::first_valid_name_path_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::BinaryOperator;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalPlace {
    pub root: facts::PlaceRoot,
    pub segments: Vec<facts::PlaceSegment>,
}

impl CanonicalPlace {
    pub fn extend_segments(&mut self, segments: &[facts::PlaceSegment]) {
        self.segments.extend(segments.iter().copied());
    }

    /// Whether this place names at least one segment and every segment is a
    /// positional step — a field or a fixed index — so the place denotes a
    /// statically located storage position rather than a computed traversal.
    pub fn has_only_positional_segments(&self) -> bool {
        !self.segments.is_empty()
            && self.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                )
            })
    }

    /// The canonical label diagnostics quote for this place.
    pub fn label(&self, program: &dyn crate::CheckingInput) -> String {
        facts::canonical_place_label_from_parts(program, self.root, &self.segments)
    }
}

/// Locate a fixed element through builtin collection windows. Windows change
/// an ordinal, not the number of collection dimensions: `a[2..][1]` denotes
/// `a[3]`. Each selector's builtin meaning is established before its geometry
/// is read. Unknown endpoints decline this exact-coordinate query; they never
/// become layout constants or bounds evidence.
pub fn canonical_collection_element_place(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    collection: ExpressionHandle,
    element_index: usize,
) -> Option<CanonicalPlace> {
    if !super::place_has_builtin_coordinates(program, machine, state, collection) {
        return None;
    }
    // A window has a slice shape but no manufactured type-table row. Its
    // authenticated builtin selector above establishes that shape; ordinary
    // named collections retain their declaration-backed type reference.
    if super::subslice_expression(program, collection).is_none() {
        let collection_type = super::declared_place_type_raw(program, machine, state, collection)?;
        if !super::collection_operations::type_reference_is_collection(program, collection_type) {
            return None;
        }
    }
    let mut place = canonical_place_from_expression(program, collection)?;
    place.segments.push(facts::PlaceSegment::FixedIndex {
        index: element_index,
    });
    let mut normalized = Vec::with_capacity(place.segments.len());
    let mut window: Option<(usize, Option<usize>)> = None;
    for segment in place.segments {
        let range = match segment {
            facts::PlaceSegment::FixedRange { start, end } => Some((start, Some(end))),
            facts::PlaceSegment::Index { expression } => {
                let range = program.expression_table.range_node(expression)?;
                let ordinal = |bound| {
                    program
                        .closed_integer_expression_value_in(bound, machine.symbol)?
                        .to_u64()
                        .and_then(|value| usize::try_from(value).ok())
                };
                let start = if range.start.is_valid() {
                    ordinal(range.start)?
                } else {
                    0
                };
                let end = if range.end.is_valid() {
                    let end = ordinal(range.end)?;
                    Some(if range.end_inclusive {
                        end.checked_add(1)?
                    } else {
                        end
                    })
                } else {
                    None
                };
                Some((start, end))
            }
            _ => None,
        };
        if let Some((start, end)) = range {
            if end.is_some_and(|end| start > end) {
                return None;
            }
            window = Some(match window {
                None => (start, end),
                Some((base, limit)) => {
                    let start = base.checked_add(start)?;
                    let end = match end {
                        Some(end) => Some(base.checked_add(end)?),
                        None => None,
                    };
                    if limit.is_some_and(|limit| start > limit)
                        || limit.zip(end).is_some_and(|(limit, end)| end > limit)
                    {
                        return None;
                    }
                    (start, end.or(limit))
                }
            });
            continue;
        }
        let segment = match (window.take(), segment) {
            (Some((base, limit)), facts::PlaceSegment::FixedIndex { index }) => {
                let index = base.checked_add(index)?;
                if limit.is_some_and(|limit| index >= limit) {
                    return None;
                }
                facts::PlaceSegment::FixedIndex { index }
            }
            (Some(_), _) => return None,
            (None, segment) => segment,
        };
        normalized.push(segment);
    }
    if window.is_some() {
        return None;
    }
    place.segments = normalized;
    Some(place)
}

pub fn index_place_segment(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> facts::PlaceSegment {
    if let ExpressionNode::Range(range) = program.expression_table.expression(expression)
        && let Some((start, end)) = fixed_half_open_range(program, range)
    {
        return facts::PlaceSegment::FixedRange { start, end };
    }
    // A selector that every execution fixes to one ordinal is that element.
    // The same store then retires and lowers the slot a literal would: a
    // parameter required equal to 0, a call that ensures that result, an
    // immutable binding of either, or a field of such a value. A wider
    // interval remains a runtime index and cannot name one home.
    element_place_segment(program, expression)
}

/// A point selector's place segment. A range is not a point: callers that
/// only fold element ordinals keep a range expression as an index segment.
pub fn element_place_segment(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> facts::PlaceSegment {
    exact_fixed_element(program, expression, 0)
        .map(|index| facts::PlaceSegment::FixedIndex { index })
        .unwrap_or(facts::PlaceSegment::Index { expression })
}

fn exact_fixed_element(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<usize> {
    if depth >= 64 || !expression.is_valid() {
        return None;
    }
    if let Some(value) = program.expression_table.constant_integer_value(expression) {
        return usize::try_from(value).ok();
    }
    if let Some(value) =
        crate::value_custody::literals::closed_record_integer_projection(program, expression)
    {
        return value
            .value
            .to_i64()
            .and_then(|element| usize::try_from(element).ok());
    }
    let bindings = match_arm_bindings(program, expression).unwrap_or_default();
    if let Some(value) = folded_coordinate(program, expression, &bindings, 0) {
        return usize::try_from(value).ok();
    }
    if let Some(value) = standing_fixed_element(program, expression) {
        return usize::try_from(value).ok();
    }
    if let Some(initializer) = immutable_name_initializer(program, expression) {
        return exact_fixed_element(program, initializer, depth + 1);
    }
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    let initializer = immutable_name_initializer(program, member.receiver)?;
    let literal = program.expression_table.struct_literal_node(initializer)?;
    let field = program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .find(|field| field.field_symbol == member.member_symbol)?;
    exact_fixed_element(program, field.value, depth + 1)
}

fn standing_fixed_element(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<i64> {
    let states = states_naming(program, expression);
    if states.is_empty() {
        return None;
    }
    let mut element = None;
    for state_symbol in states {
        let machine_symbol = program.symbols.get(state_symbol).parent;
        let (machine, state) = program.machine_state(machine_symbol, state_symbol)?;
        let (lower, upper) = crate::standing_integer_bounds(program, machine, state, expression)?;
        let lower = lower?;
        if upper != Some(lower) {
            return None;
        }
        if element.is_some_and(|prior| prior != lower) {
            return None;
        }
        element = Some(lower);
    }
    element
}

fn immutable_name_initializer(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid() || path.head_symbol != path.symbol {
        return None;
    }
    let declaration = program.symbols.get(path.symbol);
    if declaration.kind != SymbolKind::Local {
        return None;
    }
    let state_symbol = declaration.parent;
    let machine_symbol = program.symbols.get(state_symbol).parent;
    let (_, state) = program.machine_state(machine_symbol, state_symbol)?;
    let (_, local) = program.state_local_declaration(state, path.symbol)?;
    if local.is_mutable || !local.initial_value.is_valid() {
        return None;
    }
    Some(local.initial_value)
}

/// Normalize an expression-only range when its half-open bounds do not need
/// collection metadata. An omitted start is zero; an omitted end remains
/// dynamic here because the collection length is resolved by the range checker,
/// not by the place algebra. Inclusive syntax is normalized once by advancing
/// its last included ordinal.
fn fixed_half_open_range(
    program: &dyn crate::CheckingInput,
    range: &typed_trees::expression::TableRangeExpression,
) -> Option<(usize, usize)> {
    let start = if range.start.is_valid() {
        range_coordinate(program, range.start)?
    } else {
        0
    };
    if !range.end.is_valid() {
        return None;
    }
    let mut end = range_coordinate(program, range.end)?;
    if range.end_inclusive {
        end = end.checked_add(1)?;
    }
    Some((start, end))
}

/// A half-open window coordinate. Literals fold as before. A selected match
/// arm additionally substitutes its integer pattern for the subject, and an
/// ensured call result substitutes that exact integer. `which + 2` in
/// `match which { 0 -> items[0..which + 2] }` is therefore the same fixed
/// window as `items[0..2]`. A wildcard arm assigns no pattern, so its
/// endpoint stays a runtime index.
fn range_coordinate(
    program: &dyn crate::CheckingInput,
    endpoint: ExpressionHandle,
) -> Option<usize> {
    let value = if let Some(value) = program.expression_table.constant_integer_value(endpoint) {
        value
    } else {
        let bindings = match_arm_bindings(program, endpoint)?;
        folded_coordinate(program, endpoint, &bindings, 0)?
    };
    usize::try_from(value).ok()
}

/// Bindings from value arms that contain `endpoint`. Contradictory patterns
/// for one subject leave the coordinate unknown. Place identity does not make
/// a match observation survive a call: every coordinate occurrence must retain
/// an inert evaluation prefix from the selected arm's entry. A call's own
/// ensured result still folds independently of these match substitutions.
fn match_arm_bindings(
    program: &dyn crate::CheckingInput,
    endpoint: ExpressionHandle,
) -> Option<Vec<(ExpressionHandle, i64)>> {
    let mut bindings = Vec::new();
    for state_symbol in states_naming(program, endpoint) {
        let machine_symbol = program.symbols.get(state_symbol).parent;
        let Some((_, state)) = program.machine_state(machine_symbol, state_symbol) else {
            continue;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            for (_, root) in program.tables.expression_roots(statement) {
                collect_arm_bindings(program, root, endpoint, &mut bindings);
            }
        }
    }
    let mut consistent = Vec::new();
    for (subject, value) in bindings {
        if let Some((_, prior)) = consistent
            .iter()
            .find(|(existing, _)| same_match_subject(program, *existing, subject))
        {
            if *prior != value {
                return None;
            }
            continue;
        }
        consistent.push((subject, value));
    }
    Some(consistent)
}

fn collect_arm_bindings(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    endpoint: ExpressionHandle,
    bindings: &mut Vec<(ExpressionHandle, i64)>,
) {
    let mut nodes = Vec::new();
    crate::expression_traversal::collect_expression_tree(program, root, &mut nodes);
    for node in nodes {
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(node) else {
            continue;
        };
        let mut patterns = Vec::new();
        for arm in program.expression_table.match_arms(dispatch.arms) {
            let MatchPattern::Value(pattern) = arm.pattern else {
                continue;
            };
            // The subject is captured once, before the ordered pattern tests.
            // Failed patterns execute; their arm values do not. A pattern can
            // change storage that a later literal arm's coordinate reads.
            patterns.push(pattern);
            let Some(value) = program
                .expression_table
                .integer_node(pattern)
                .and_then(|literal| literal.value_i64())
            else {
                continue;
            };
            if crate::expression_traversal::expression_has_inert_prefix(
                program, &patterns, arm.value, endpoint,
            ) {
                bindings.push((dispatch.subject, value));
            }
        }
    }
}

fn states_naming(
    program: &dyn crate::CheckingInput,
    endpoint: ExpressionHandle,
) -> Vec<SymbolHandle> {
    let mut nodes = Vec::new();
    crate::expression_traversal::collect_expression_tree(program, endpoint, &mut nodes);
    let mut states = Vec::new();
    for node in nodes {
        let ExpressionNode::Name(path) = program.expression_table.expression(node) else {
            continue;
        };
        if !path.head_symbol.is_valid() {
            continue;
        }
        let declaration = program.symbols.get(path.head_symbol);
        if !matches!(declaration.kind, SymbolKind::Parameter | SymbolKind::Local) {
            continue;
        }
        if !states.contains(&declaration.parent) {
            states.push(declaration.parent);
        }
    }
    states
}

fn folded_coordinate(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    bindings: &[(ExpressionHandle, i64)],
    depth: usize,
) -> Option<i64> {
    if depth >= 64 || !expression.is_valid() {
        return None;
    }
    let mut matched = bindings
        .iter()
        .filter(|(subject, _)| same_match_subject(program, *subject, expression))
        .map(|(_, value)| *value);
    if let Some(first) = matched.next() {
        return matched.all(|value| value == first).then_some(first);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal.value_i64(),
        ExpressionNode::Borrow(borrow) => {
            folded_coordinate(program, borrow.target, bindings, depth + 1)
        }
        ExpressionNode::Binary(binary) => {
            let left = folded_coordinate(program, binary.left, bindings, depth + 1)?;
            let right = folded_coordinate(program, binary.right, bindings, depth + 1)?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                BinaryOperator::Multiply => left.checked_mul(right),
                BinaryOperator::Divide => (right != 0).then(|| left.checked_div(right)).flatten(),
                BinaryOperator::Modulo => (right != 0).then(|| left.checked_rem(right)).flatten(),
                _ => None,
            }
        }
        ExpressionNode::Call(call) => {
            let (low, high) = crate::normal_return_integer_bounds(program, call)?;
            match (low, high) {
                (Some(low), Some(high)) if low == high => Some(low),
                _ => None,
            }
        }
        _ => None,
    }
}

fn same_match_subject(
    program: &dyn crate::CheckingInput,
    subject: ExpressionHandle,
    expression: ExpressionHandle,
) -> bool {
    if subject == expression {
        return true;
    }
    let Some(subject_place) = canonical_place_from_expression(program, subject) else {
        return false;
    };
    canonical_place_from_expression(program, expression).is_some_and(|place| place == subject_place)
}

pub fn push_field_place_segments(
    program: &dyn crate::CheckingInput,
    segments: &mut Vec<facts::PlaceSegment>,
    symbol: SymbolHandle,
) {
    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
        segments.push(facts::PlaceSegment::Case { variant });
    }
    segments.push(facts::PlaceSegment::Field { symbol });
}

pub fn canonical_place_from_expression(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => canonical_place_from_expression(program, inner.target),
        // A borrow recast re-views the operand's bytes at the same address:
        // the recast place is the operand's place. Value casts compute a new
        // rvalue and keep the expression root.
        ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            canonical_place_from_expression(program, cast.value)
        }
        ExpressionNode::Name(path) => {
            // Constants and case names produce values, not storage owned by
            // their declarations. A borrow captures this materialized copy;
            // two uses of one declaration must not become the same home.
            if name_materializes_value(program, expression) {
                return Some(CanonicalPlace {
                    root: facts::PlaceRoot::Expression(expression),
                    segments: Vec::new(),
                });
            }
            let root_symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            let mut place = CanonicalPlace {
                root: facts::PlaceRoot::Symbol(root_symbol),
                segments: Vec::new(),
            };
            let members = program.expression_table.name_path_members(path.members);
            for (offset, retained) in program
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .skip(1)
                .copied()
                .enumerate()
            {
                let symbol = members
                    .get(offset + 1)
                    .and_then(|member| {
                        super::contextual_projection::resolve_member_symbol_from_place(
                            program,
                            &place,
                            member.as_str(),
                            None,
                            retained,
                        )
                    })
                    .unwrap_or(retained);
                push_field_place_segments(program, &mut place.segments, symbol);
            }
            Some(place)
        }
        ExpressionNode::Member(member) => {
            let mut place = canonical_place_from_expression(program, member.receiver)?;
            let symbol = effective_member_symbol(program, member.receiver, member);
            push_field_place_segments(program, &mut place.segments, symbol);
            Some(place)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut place = canonical_place_from_expression(program, indexed.collection)?;
            place
                .segments
                .push(index_place_segment(program, indexed.index));
            Some(place)
        }
        _ => Some(CanonicalPlace {
            root: facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        }),
    }
}

pub(super) fn name_materializes_value(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    crate::exact_case_reference_owner(program, expression).is_some()
        || matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && program.symbols.get(path.symbol).kind == SymbolKind::Const
        )
}

pub fn canonical_place_from_expression_in_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let mut place = contextual_canonical_place_from_expression(
        program,
        state_symbol,
        statement_index,
        expression,
    )
    .or_else(|| canonical_place_from_expression(program, expression))?;
    if let Some(receiver) = place.root.as_expression()
        && let Some(state) = program.state_by_symbol(state_symbol)
        && let Some((_, saved)) = super::structural_scalar_field::saved_dispatch_subject(
            program,
            state,
            statement_index,
            receiver,
        )
    {
        // Guards and payload receivers contain parser copies of one authored
        // dispatch subject. Its retained result has one identity; another
        // independently authored call does not join this place.
        place.root = facts::PlaceRoot::Expression(saved);
    }
    Some(place)
}
