//! Invocation-owned exact storage projections; no live-flow facts or destination type guesses.

use super::contextual_projection::contextual_canonical_place_from_expression;
use super::member_resolution::effective_member_symbol;

use super::declared_symbols::first_valid_name_path_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

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

pub fn index_place_segment(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> facts::PlaceSegment {
    if let ExpressionNode::Range(range) = program.expression_table.expression(expression)
        && let Some((start, end)) = fixed_half_open_range(program, range)
    {
        return facts::PlaceSegment::FixedRange { start, end };
    }
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
        .map(|index| facts::PlaceSegment::FixedIndex { index })
        .unwrap_or(facts::PlaceSegment::Index { expression })
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
        usize::try_from(
            program
                .expression_table
                .constant_integer_value(range.start)?,
        )
        .ok()?
    } else {
        0
    };
    if !range.end.is_valid() {
        return None;
    }
    let end = usize::try_from(program.expression_table.constant_integer_value(range.end)?).ok()?;
    let end = if range.end_inclusive {
        end.checked_add(1)?
    } else {
        end
    };
    Some((start, end))
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
