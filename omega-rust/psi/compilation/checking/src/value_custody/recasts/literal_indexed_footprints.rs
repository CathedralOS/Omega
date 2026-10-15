//! Literal-indexed recast footprints and fixed-array terminals.

use crate::value_custody::recasts::raw_byte_region::{ByteRegionFootprint, byte_region_footprint};
use crate::value_custody::recasts::recast_judgments::{judge_scalar_recast, strip_mutable};
use crate::value_custody::recasts::record_eligibility::{
    closed_fact_free_record_symbol_is_eligible, direct_phantom_lifetime_record_symbol,
};
use crate::value_custody::recasts::record_representation::{
    representation_is_exactly_tiled, type_representation,
};
use crate::value_custody::recasts::representation_types::exact_primitive_type;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCastExpression, TableIndexedExpression,
};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Canonical checked footprint for the first precise indexed-recast loan rung.
///
/// The collection expression names the fixed byte-array place. `start..end`
/// covers the complete validated target footprint in byte ordinals. For a
/// target family it is the union of all alternatives at this same captured
/// start, not a selected physical layout. This carrier
/// is produced only by replaying the ordinary recast judgment for a direct
/// authored borrow occurrence; consumers must not reconstruct it from cast
/// syntax alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedLiteralIndexedRecastFootprint {
    collection: ExpressionHandle,
    start: usize,
    end: usize,
}

impl ValidatedLiteralIndexedRecastFootprint {
    pub const fn collection(self) -> ExpressionHandle {
        self.collection
    }

    pub const fn start(self) -> usize {
        self.start
    }

    pub const fn end(self) -> usize {
        self.end
    }
}

/// The shared admission preamble of both indexed-recast walks: the borrow
/// access note, the recast-form cast unwrap, the scalar recast judgment,
/// and the indexed-source unwrap.
#[allow(clippy::too_many_arguments)]
fn literal_indexed_recast_preamble<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    viewed_type: TypeReferenceHandle,
    access: language_core::ReferenceAccess,
) -> Option<(
    &'program TableCastExpression,
    ExpressionHandle,
    &'program TableIndexedExpression,
)> {
    let spelled_borrow_exclusive = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => Some(borrow.access.is_exclusive()),
        _ => None,
    };
    let initializer = strip_mutable(program, expression);
    let cast = program.expression_table.cast_node(initializer)?;
    if !cast.form.is_recast() {
        return None;
    }

    let mut diagnostics = Vec::new();
    judge_scalar_recast(
        program,
        machine,
        state,
        statement_index,
        cast,
        initializer,
        viewed_type,
        access.is_exclusive(),
        spelled_borrow_exclusive,
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return None;
    }

    let source = strip_mutable(program, cast.value);
    let indexed = program.expression_table.indexed_node(source)?;
    Some((cast, source, indexed))
}

/// A slice cast target pins the footprint to its recast byte region: the
/// region must name the same collection and start offset as `indexed`.
/// `Some(None)` means the target is not a slice — the caller falls back to
/// the literal target-size arm.
fn slice_backed_recast_region<'program>(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    cast: &'program TableCastExpression,
    source: ExpressionHandle,
    indexed: &'program TableIndexedExpression,
) -> Option<Option<super::raw_byte_region::SliceRecastByteRegion>> {
    if !matches!(
        program
            .type_reference_table
            .type_reference(cast.target_type),
        TypeReferenceNode::Slice { .. }
    ) {
        return Some(None);
    }
    let region = super::slice_recast_byte_region(program, machine, state, expression)?;
    if region.source != source
        || region.collection != indexed.collection
        || region.start != indexed.index
    {
        return None;
    }
    Some(Some(region))
}

/// Replay every recast alternative and retain one conservative literal-index
/// byte footprint for borrow overlap.
///
/// This excludes runtime or singleton-range offsets and whole-array sources.
/// An interior slice covers the complete literal suffix of its byte collection;
/// its element layout and exact tiling are checked by the ordinary recast
/// judgment. Aggregate targets are limited to one closed,
/// recursively fact-free fixed record or recursively nonzero literal fixed
/// arrays ending in either an exact primitive or that same closed-record
/// subset. Other shapes remain under their existing conservative loan fence
/// even when the broader recast judgment can validate their representation.
pub fn validate_literal_indexed_recast_footprint(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    viewed_type: TypeReferenceHandle,
    access: language_core::ReferenceAccess,
) -> Option<ValidatedLiteralIndexedRecastFootprint> {
    let (cast, source, indexed) = literal_indexed_recast_preamble(
        program,
        machine,
        state,
        statement_index,
        expression,
        viewed_type,
        access,
    )?;
    let offset = program.expression_table.integer_node(indexed.index)?;
    let start = usize::try_from(offset.value_i64()?).ok()?;
    let end = if let Some(region) =
        slice_backed_recast_region(program, machine, state, expression, cast, source, indexed)?
    {
        usize::try_from(region.extent).ok()?
    } else {
        start.checked_add(maximum_recast_footprint(
            program,
            machine,
            state,
            indexed.collection,
            cast.target_type,
        )?)?
    };

    let footprint = i64::try_from(end.checked_sub(start)?).ok()?;
    let ByteRegionFootprint::Contained =
        byte_region_footprint(program, machine, state, statement_index, source, footprint)
    else {
        return None;
    };
    Some(ValidatedLiteralIndexedRecastFootprint {
        collection: indexed.collection,
        start,
        end,
    })
}

/// Admission for the runtime-indexed sibling of the literal footprint.
///
/// The preamble replays the same recast judgment as the literal footprint.
/// Fixed targets owe `high(index) + size(target) <= region_length` and retain
/// their conservative indexed source. An unsized slice instead covers all
/// remaining bytes; its validated collection is returned as a whole-place loan.
/// This deliberately overapproximates the dynamic suffix so an unknown offset
/// cannot grant access to another byte inside that suffix. `None` keeps the source under its historical
/// no-loan fence: unbounded offsets, non-byte interiors, and unsupported
/// target shapes mint no loan at all.
pub fn validate_runtime_indexed_recast_source(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    viewed_type: TypeReferenceHandle,
    access: language_core::ReferenceAccess,
) -> Option<ExpressionHandle> {
    let (cast, source, indexed) = literal_indexed_recast_preamble(
        program,
        machine,
        state,
        statement_index,
        expression,
        viewed_type,
        access,
    )?;
    // Literal indices keep the exact `FixedRange` footprint arm; this gate
    // owns only runtime or otherwise non-literal offsets.
    if matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Integer(_)
    ) {
        return None;
    }

    // The judgment above independently checks bounds and exact tiling of
    // this complete suffix. Source syntax identifies its backing, not a
    // guessed one-element footprint or a newly assumed representation.
    if let Some(region) =
        slice_backed_recast_region(program, machine, state, expression, cast, source, indexed)?
    {
        return Some(region.collection);
    }

    let target_size = i64::try_from(maximum_recast_footprint(
        program,
        machine,
        state,
        indexed.collection,
        cast.target_type,
    )?)
    .ok()?;
    let ByteRegionFootprint::Contained = byte_region_footprint(
        program,
        machine,
        state,
        statement_index,
        source,
        target_size,
    ) else {
        return None;
    };
    Some(source)
}

/// Loan overlap may conservatively retain the largest footprint after all
/// same-occurrence representation judgments pass. This result must not escape
/// as a concrete size/offset recipe for execution or target realization.
fn maximum_recast_footprint(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    collection: ExpressionHandle,
    target_type: TypeReferenceHandle,
) -> Option<usize> {
    let backing = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        collection,
    )?;
    let targets = crate::layout_checking_targets(program, [backing, target_type]).ok()?;
    let mut maximum = None;
    for target in targets {
        let alternative = crate::LayoutCheckingContext::new(program, target);
        let size = literal_indexed_recast_target_size(&alternative, target_type)?;
        maximum = Some(maximum.map_or(size, |previous: usize| previous.max(size)));
    }
    maximum
}

pub(crate) fn literal_indexed_recast_target_size(
    program: &dyn crate::CheckingInput,
    target_type: TypeReferenceHandle,
) -> Option<usize> {
    if let Some(primitive) = exact_primitive_type(program, target_type)
        .filter(|primitive| *primitive != PrimitiveType::Bool)
    {
        return primitive.scalar_byte_size();
    }

    if let Some(terminal) = literal_fixed_array_target_terminal(program, target_type) {
        return type_representation(program, target_type).and_then(|representation| {
            let eligible = representation.size > 0
                && match terminal {
                    LiteralFixedArrayTerminal::ExactPrimitive => {
                        representation_is_exactly_tiled(&representation)
                    }
                    LiteralFixedArrayTerminal::ClosedRecord => true,
                };
            eligible.then_some(representation.size)
        });
    }

    if direct_phantom_lifetime_record_symbol(program, target_type).is_some() {
        return type_representation(program, target_type)
            .map(|representation| representation.size)
            .filter(|size| *size > 0);
    }

    let symbol = &program
        .type_reference_table
        .type_reference(target_type)
        .as_named_symbol()?;
    if !closed_fact_free_record_symbol_is_eligible(program, *symbol) {
        return None;
    }

    type_representation(program, target_type)
        .map(|representation| representation.size)
        .filter(|size| *size > 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiteralFixedArrayTerminal {
    ExactPrimitive,
    ClosedRecord,
}

fn literal_fixed_array_target_terminal(
    program: &dyn crate::CheckingInput,
    target_type: TypeReferenceHandle,
) -> Option<LiteralFixedArrayTerminal> {
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(target_type)
    else {
        return None;
    };
    if *length == 0 {
        return None;
    }

    match program.type_reference_table.type_reference(*element_type) {
        TypeReferenceNode::Named { symbol, .. } => {
            if exact_primitive_type(program, *element_type).is_some_and(|primitive| {
                primitive != PrimitiveType::Bool && primitive.scalar_byte_size().is_some()
            }) {
                return Some(LiteralFixedArrayTerminal::ExactPrimitive);
            }
            closed_fact_free_record_symbol_is_eligible(program, *symbol)
                .then_some(LiteralFixedArrayTerminal::ClosedRecord)
        }
        TypeReferenceNode::FixedArray { .. } => {
            literal_fixed_array_target_terminal(program, *element_type)
        }
        _ => None,
    }
}
