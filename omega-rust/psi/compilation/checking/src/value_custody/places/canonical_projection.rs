//! Invocation-owned exact storage projections; no live-flow facts or destination type guesses.

use super::contextual_projection::contextual_canonical_place_from_expression;
use super::member_resolution::effective_member_symbol;

use super::declared_symbols::first_valid_name_path_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use symbols::{SymbolHandle, SymbolKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalPlace {
    pub root: facts::PlaceRoot,
    pub segments: Vec<facts::PlaceSegment>,
}

impl CanonicalPlace {
    /// Order exact structural identities for construction-time batch joins.
    /// This matches structural equality, including full handle generations;
    /// it does not establish aliasing, overlap or equality of runtime indices.
    pub fn compare_identity(&self, other: &Self) -> std::cmp::Ordering {
        use facts::{PlaceRoot, PlaceSegment};
        let root_key = |root| match root {
            PlaceRoot::Unknown => (0, 0, 0, 0, 0),
            PlaceRoot::InvocationOutput(_) => (5, 0, 0, 0, 0),
            PlaceRoot::Symbol(handle) => (1, handle.arena_index(), handle.generation(), 0, 0),
            PlaceRoot::Expression(handle) => (2, handle.arena_index(), handle.generation(), 0, 0),
            PlaceRoot::TypeReference(handle) => {
                (3, handle.arena_index(), handle.generation(), 0, 0)
            }
            PlaceRoot::InvocationReferent {
                parameter,
                type_reference,
            } => (
                4,
                parameter.arena_index(),
                parameter.generation(),
                type_reference.arena_index(),
                type_reference.generation(),
            ),
        };
        let segment_key = |segment| match segment {
            PlaceSegment::Field { symbol } => (0, symbol.arena_index(), symbol.generation(), 0, 0),
            PlaceSegment::Case { variant } => {
                (1, variant.arena_index(), variant.generation(), 0, 0)
            }
            PlaceSegment::FixedIndex { index } => (2, 0, 0, index, 0),
            PlaceSegment::FixedRange { start, end } => (3, 0, 0, start, end),
            PlaceSegment::OpaqueRange => (4, 0, 0, 0, 0),
            PlaceSegment::Index { expression } => {
                (5, expression.arena_index(), expression.generation(), 0, 0)
            }
        };
        root_key(self.root)
            .cmp(&root_key(other.root))
            .then_with(|| match (self.root, other.root) {
                (PlaceRoot::InvocationOutput(left), PlaceRoot::InvocationOutput(right)) => {
                    left.compare_identity(right)
                }
                _ => std::cmp::Ordering::Equal,
            })
            .then_with(|| {
                self.segments
                    .iter()
                    .copied()
                    .map(segment_key)
                    .cmp(other.segments.iter().copied().map(segment_key))
            })
    }

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
    // Inert singleton snapshots can name a fixed element. Executable calls
    // retain their selector occurrence even when the completed value is known;
    // later captured-value proofs establish geometry without erasing effects.
    // A wider interval keeps its conservative runtime coordinate.
    element_place_segment(program, expression)
}

/// A point selector's place segment. A range is not a point: callers that
/// only fold element ordinals keep a range expression as an index segment.
pub fn element_place_segment(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> facts::PlaceSegment {
    // A singleton postcondition describes the completed coordinate; it does
    // not erase the invocation that produces it. Keep executable selectors
    // represented until their operand owner has evaluated them. Closed
    // constants retain their ordinary compile-time meaning, and reading an
    // immutable stored snapshot still has no invocation to repeat.
    if program
        .closed_integer_expression_value(expression)
        .is_none()
    {
        let mut nodes = Vec::new();
        crate::expression_traversal::collect_expression_tree(program, expression, &mut nodes);
        if nodes.iter().any(|node| {
            matches!(
                program.expression_table.expression(*node),
                ExpressionNode::Call(_)
            )
        }) {
            return facts::PlaceSegment::Index { expression };
        }
    }
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
    if let Some(value) = program.closed_integer_expression_value(expression) {
        return value.to_u64().and_then(|value| usize::try_from(value).ok());
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
    if let Some(value) = folded_coordinate(program, expression, &bindings) {
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
    let value = if let Some(value) = program.closed_integer_expression_value(endpoint) {
        value.to_i64()?
    } else {
        let bindings = match_arm_bindings(program, endpoint)?;
        folded_coordinate(program, endpoint, &bindings)?
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
) -> Option<i64> {
    if !expression.is_valid() {
        return None;
    }
    let mut states = states_naming(program, expression);
    if states.is_empty() {
        states.push(SymbolHandle::invalid());
    }
    let mut result = None;
    for state_symbol in states {
        let machine_symbol = if state_symbol.is_valid() {
            program.symbols.get(state_symbol).parent
        } else {
            SymbolHandle::invalid()
        };
        // Captured match subjects and ensured singleton results supply values,
        // not replacement arithmetic. The shared evaluator retains the original
        // carrier/policy and exact anonymous rational subexpressions around them.
        let value = program
            .integer_expression_value_with_bindings(expression, machine_symbol, &mut |operand| {
                let mut matched = bindings
                    .iter()
                    .filter(|(subject, _)| same_match_subject(program, *subject, operand))
                    .map(|(_, value)| *value);
                let value = if let Some(first) = matched.next() {
                    matched.all(|value| value == first).then_some(first)?
                } else {
                    let call = program.expression_table.call_node(operand)?;
                    let (Some(low), Some(high)) = crate::normal_return_integer_bounds(
                        program,
                        machine_symbol,
                        operand,
                        call,
                    )?
                    else {
                        return None;
                    };
                    (low == high).then_some(low)?
                };
                let type_reference = if state_symbol.is_valid() {
                    let (machine, state) = program.machine_state(machine_symbol, state_symbol)?;
                    crate::expression_result_type_reference(program, machine, state, operand)?
                } else {
                    let call = program.expression_table.call_node(operand)?;
                    crate::call_target_return_type(
                        program,
                        crate::call_target_symbol(program, call),
                    )?
                };
                Some((numerics::bignum::BigInt::from_i64(value), type_reference))
            })?
            .to_i64()?;
        if result.is_some_and(|prior| prior != value) {
            return None;
        }
        result = Some(value);
    }
    result
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
        ExpressionNode::SavedSubject(saved) => {
            program.saved_value_source(saved)?;
            let anchor = program
                .expression_table
                .value_capture(saved.capture)
                .value_anchor;
            if !program.expression_table.expression_is_valid(anchor) {
                return None;
            }
            Some(CanonicalPlace {
                root: facts::PlaceRoot::Expression(anchor),
                segments: Vec::new(),
            })
        }
        ExpressionNode::Borrow(inner) => canonical_place_from_expression(program, inner.target),
        // A borrow recast re-views the operand's bytes at the same address:
        // the recast place is the operand's place. Same-carrier qualification
        // casts preserve the operand's storage place; value conversions
        // compute a new rvalue and keep the expression root.
        ExpressionNode::Cast(cast) if cast_preserves_place(program, cast) => {
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

pub(super) fn cast_preserves_place(
    program: &dyn crate::CheckingInput,
    cast: &checked_trees::expression::TableCastExpression,
) -> bool {
    if cast.form.is_recast() {
        return true;
    }
    let target = if cast.result_type.is_valid() {
        cast.result_type
    } else {
        cast.target_type
    };
    if !target.is_valid() {
        return false;
    }
    let Some(target_carrier) = program.unwrapped_type_reference(target) else {
        return false;
    };
    let Some(operand_pos) = super::member_resolution::expression_type_position(program, cast.value)
    else {
        return false;
    };
    match operand_pos {
        super::member_resolution::MemberPosition::Reference(operand_ref) => {
            carriers_match(program, operand_ref, target_carrier)
        }
        super::member_resolution::MemberPosition::Declaration(symbol) => {
            match program.type_reference_table.type_reference(target_carrier) {
                typed_trees::types::TypeReferenceNode::Named { symbol: s, .. } => *s == symbol,
                typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
                    *base_symbol == symbol
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn carriers_match(
    program: &dyn crate::CheckingInput,
    left: typed_trees::types::TypeReferenceHandle,
    right: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let Some(left) = program.unwrapped_type_reference(left) else {
        return false;
    };
    let Some(right) = program.unwrapped_type_reference(right) else {
        return false;
    };
    if left == right {
        return true;
    }
    match (
        program.type_reference_table.type_reference(left),
        program.type_reference_table.type_reference(right),
    ) {
        (
            typed_trees::types::TypeReferenceNode::Named {
                symbol: s1,
                name: n1,
            },
            typed_trees::types::TypeReferenceNode::Named {
                symbol: s2,
                name: n2,
            },
        ) => {
            if s1.is_valid() && s2.is_valid() {
                s1 == s2
            } else {
                n1 == n2
            }
        }
        (
            typed_trees::types::TypeReferenceNode::Generic {
                base_symbol: s1,
                base_name: n1,
                ..
            },
            typed_trees::types::TypeReferenceNode::Generic {
                base_symbol: s2,
                base_name: n2,
                ..
            },
        ) => {
            if s1.is_valid() && s2.is_valid() {
                s1 == s2
            } else {
                n1 == n2
            }
        }
        (
            typed_trees::types::TypeReferenceNode::Unit,
            typed_trees::types::TypeReferenceNode::Unit,
        ) => true,
        _ => false,
    }
}

/// Whether either place resolver could root `expression` at `symbol`. Both
/// descend borrows, recasts, members and indexes to one leaf; only a storage
/// `Name` leaf yields a symbol root, and a resolved path names exactly its
/// own first symbol. An unresolved path falls back to state-scoped spelling,
/// so it remains possible. A `false` answer is exact: callers asking many
/// expressions for one root skip the full resolution of every other place.
pub fn place_root_may_be_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    let mut leaf = expression;
    loop {
        if !leaf.is_valid() {
            return false;
        }
        leaf = match program.expression_table.expression(leaf) {
            ExpressionNode::Borrow(inner) => inner.target,
            ExpressionNode::Cast(cast) if cast_preserves_place(program, cast) => cast.value,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            ExpressionNode::Name(path) => {
                return !name_materializes_value(program, leaf)
                    && first_valid_name_path_symbol(path, &program.expression_table)
                        .is_none_or(|root| root == symbol);
            }
            _ => return false,
        };
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
