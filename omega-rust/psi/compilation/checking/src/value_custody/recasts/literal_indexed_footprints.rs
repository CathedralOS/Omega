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

/// Replay the canonical recast judgment and retain one exact literal-index
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
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    viewed_type: TypeReferenceHandle,
    access: language_core::ReferenceAccess,
) -> Option<ValidatedLiteralIndexedRecastFootprint> {
    let spelled_borrow_exclusive = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => Some(borrow.access.is_exclusive()),
        _ => None,
    };
    let initializer = strip_mutable(program, expression);
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
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return None;
    };
    let ExpressionNode::Integer(offset) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    let start = usize::try_from(offset.value_i64()?).ok()?;
    let end = if matches!(
        program
            .type_reference_table
            .type_reference(cast.target_type),
        TypeReferenceNode::Slice { .. }
    ) {
        let region = super::slice_recast_byte_region(program, machine, state, expression)?;
        if region.source != source
            || region.collection != indexed.collection
            || region.start != indexed.index
        {
            return None;
        }
        usize::try_from(region.extent).ok()?
    } else {
        start.checked_add(literal_indexed_recast_target_size(
            program,
            cast.target_type,
        )?)?
    };

    let InteriorByteRegion::Bounded {
        offset,
        region_length,
    } = interior_byte_region_source(program, machine, state, statement_index, source)
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
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    viewed_type: TypeReferenceHandle,
    access: language_core::ReferenceAccess,
) -> Option<ExpressionHandle> {
    let spelled_borrow_exclusive = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => Some(borrow.access.is_exclusive()),
        _ => None,
    };
    let initializer = strip_mutable(program, expression);
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
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return None;
    };
    // Literal indices keep the exact `FixedRange` footprint arm; this gate
    // owns only runtime or otherwise non-literal offsets.
    if matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Integer(_)
    ) {
        return None;
    }

    if matches!(
        program
            .type_reference_table
            .type_reference(cast.target_type),
        TypeReferenceNode::Slice { .. }
    ) {
        // The judgment above independently checks bounds and exact tiling of
        // this complete suffix. Source syntax identifies its backing, not a
        // guessed one-element footprint or a newly assumed representation.
        let region = super::slice_recast_byte_region(program, machine, state, expression)?;
        if region.source != source
            || region.collection != indexed.collection
            || region.start != indexed.index
        {
            return None;
        }
        return Some(region.collection);
    }

    let target_size = i64::try_from(literal_indexed_recast_target_size(
        program,
        cast.target_type,
    )?)
    .ok()?;
    let InteriorByteRegion::Bounded {
        offset,
        region_length,
    } = interior_byte_region_source(program, machine, state, statement_index, source)
    else {
        return None;
    };
    if offset.checked_add(target_size)? > region_length {
        return None;
    }

    Some(source)
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

#[cfg(test)]
mod tests {
    fn literal_footprint(
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<super::ValidatedLiteralIndexedRecastFootprint> {
        let typed_trees::types::TypeReferenceNode::Reference {
            referee, access, ..
        } = program
            .type_reference_table
            .type_reference(local.type_reference)
        else {
            return None;
        };
        super::validate_literal_indexed_recast_footprint(
            program,
            machine,
            state,
            0,
            local.initial_value,
            *referee,
            *access,
        )
    }
    fn runtime_source(
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        let typed_trees::types::TypeReferenceNode::Reference {
            referee, access, ..
        } = program
            .type_reference_table
            .type_reference(local.type_reference)
        else {
            return None;
        };
        super::validate_runtime_indexed_recast_source(
            program,
            machine,
            state,
            0,
            local.initial_value,
            *referee,
            *access,
        )
    }

    use super::*;

    fn region_source(extent: u32, offset: &str, target: &str) -> TypedTrees {
        crate::front_end::typed_program(&format!(
            "data Storage where k <= 3 {{ bytes: [u8; {extent}]; k: u32; }}
             machine run(storage: &mut Storage) {{
                 let view: &mut [{target}] = &mut storage.bytes[{offset}] as &mut [{target}];
             }}"
        ))
    }

    fn local(
        program: &TypedTrees,
    ) -> (
        &typed_trees::machine::Machine,
        &typed_trees::state::State,
        &typed_trees::statement::TableLocalData,
    ) {
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let typed_trees::statement::StatementNode::LocalData(local) =
            &program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("source loan declaration");
        };
        (machine, state, local)
    }

    #[test]
    fn interior_slice_loans_cover_exact_literal_suffix_and_conservative_runtime_backing() {
        let program = region_source(7, "1", "u16");
        let (machine, state, local) = local(&program);
        let region =
            super::super::slice_recast_byte_region(&program, machine, state, local.initial_value)
                .unwrap();
        let footprint = literal_footprint(&program, machine, state, local)
            .expect("six suffix bytes tile three u16 elements");
        assert_eq!(footprint.collection(), region.collection);
        assert_eq!((footprint.start(), footprint.end()), (1, 7));
        assert!(runtime_source(&program, machine, state, local).is_none());

        let program = region_source(8, "storage.k * 2", "u16");
        let (machine, state, local) = self::local(&program);
        let region =
            super::super::slice_recast_byte_region(&program, machine, state, local.initial_value)
                .unwrap();
        assert_eq!(
            runtime_source(&program, machine, state, local),
            Some(region.collection)
        );
        assert_ne!(
            region.collection, region.source,
            "the loan covers the collection, not one selected byte"
        );
        assert!(literal_footprint(&program, machine, state, local).is_none());
    }

    #[test]
    fn interior_slice_loans_require_independent_extent_tiling_and_validity() {
        for (extent, offset, target) in [
            (7, "2", "u16"),
            (7, "storage.k * 2", "u16"),
            (8, "storage.k * 4", "u16"),
            (8, "storage.k * 2", "bool"),
        ] {
            let program = region_source(extent, offset, target);
            let (machine, state, local) = local(&program);
            assert!(literal_footprint(&program, machine, state, local).is_none());
            assert!(runtime_source(&program, machine, state, local).is_none());
        }
    }
}
