//! Literal-indexed recast footprints and fixed-array terminals.

use crate::value_custody::recasts::raw_byte_region::{
    InteriorByteRegion, interior_byte_region_source,
};
use crate::value_custody::recasts::recast_judgments::{judge_scalar_recast, strip_mutable};
use crate::value_custody::recasts::record_eligibility::{
    closed_fact_free_record_symbol_is_eligible, direct_phantom_lifetime_record_symbol,
};
use crate::value_custody::recasts::record_representation::{
    representation_is_exactly_tiled, shared_projection_type_representation,
};
use crate::value_custody::recasts::representation_types::exact_primitive_type;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Canonical checked footprint for the first precise indexed-recast loan rung.
///
/// The collection expression names the fixed byte-array place. `start..end`
/// is the complete validated target footprint in byte ordinals. This carrier
/// is produced only by replaying the ordinary recast judgment for a direct
/// reference-local initializer; consumers must not reconstruct it from cast
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

/// Replay the canonical recast judgment and retain one exact literal-index
/// byte footprint for borrow overlap.
///
/// This deliberately excludes runtime or singleton-range offsets, slices, and
/// whole-array sources. Aggregate targets are limited to one closed,
/// recursively fact-free fixed record or recursively nonzero literal fixed
/// arrays ending in either an exact primitive or that same closed-record
/// subset. Other shapes remain under their existing conservative loan fence
/// even when the broader recast judgment can validate their representation.
pub fn validate_literal_indexed_recast_footprint(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    local: &typed_trees::statement::TableLocalData,
) -> Option<ValidatedLiteralIndexedRecastFootprint> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    let spelled_borrow_exclusive = match program.expression_table.expression(local.initial_value) {
        ExpressionNode::Borrow(borrow) => Some(borrow.access.is_exclusive()),
        _ => None,
    };
    let initializer = strip_mutable(program, local.initial_value);
    let ExpressionNode::Cast(cast) = program.expression_table.expression(initializer) else {
        return None;
    };
    if !cast.form.is_recast() {
        return None;
    }

    let mut diagnostics = Vec::new();
    judge_scalar_recast(
        program,
        machine,
        state,
        cast,
        initializer,
        *referee,
        access.is_exclusive(),
        spelled_borrow_exclusive,
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return None;
    }

    let source = strip_mutable(program, cast.value);
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return None;
    };
    let ExpressionNode::Integer(offset) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    let start = usize::try_from(offset.value_i64()?).ok()?;
    let target_size = literal_indexed_recast_target_size(program, cast.target_type)?;
    let end = start.checked_add(target_size)?;

    let InteriorByteRegion::Bounded {
        offset,
        region_length,
    } = interior_byte_region_source(program, machine, state, source)
    else {
        return None;
    };
    if usize::try_from(offset).ok()? != start || end > usize::try_from(region_length).ok()? {
        return None;
    }

    Some(ValidatedLiteralIndexedRecastFootprint {
        collection: indexed.collection,
        start,
        end,
    })
}

pub(crate) fn literal_indexed_recast_target_size(
    program: &TypedTrees,
    target_type: TypeReferenceHandle,
) -> Option<usize> {
    if let Some(primitive) = exact_primitive_type(program, target_type)
        .filter(|primitive| *primitive != PrimitiveType::Bool)
    {
        return primitive.scalar_byte_size();
    }

    if let Some(terminal) = literal_fixed_array_target_terminal(program, target_type) {
        return shared_projection_type_representation(program, target_type).and_then(
            |representation| {
                let eligible = representation.size > 0
                    && match terminal {
                        LiteralFixedArrayTerminal::ExactPrimitive => {
                            representation_is_exactly_tiled(&representation)
                        }
                        LiteralFixedArrayTerminal::ClosedRecord => true,
                    };
                eligible.then_some(representation.size)
            },
        );
    }

    if direct_phantom_lifetime_record_symbol(program, target_type).is_some() {
        return shared_projection_type_representation(program, target_type)
            .map(|representation| representation.size)
            .filter(|size| *size > 0);
    }

    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(target_type)
    else {
        return None;
    };
    if !closed_fact_free_record_symbol_is_eligible(program, *symbol) {
        return None;
    }

    shared_projection_type_representation(program, target_type)
        .map(|representation| representation.size)
        .filter(|size| *size > 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiteralFixedArrayTerminal {
    ExactPrimitive,
    ClosedRecord,
}

fn literal_fixed_array_target_terminal(
    program: &TypedTrees,
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
