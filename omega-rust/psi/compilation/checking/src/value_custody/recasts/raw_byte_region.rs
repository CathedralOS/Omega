use diagnostics::Diagnostic;
use std::collections::HashSet;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

type SymbolIdentity = (u32, u32);

enum ByteRegionExtent {
    Fixed(i64),
    Descriptor,
}

struct IndexedByteRegion {
    start: ExpressionHandle,
    element: TypeReferenceHandle,
    extent: ByteRegionExtent,
}

/// Shape supplies geometry only. Admission separately proves the footprint
/// at this occurrence and checks representation facts in the access direction.
fn indexed_byte_region(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
) -> Option<IndexedByteRegion> {
    let indexed = program.expression_table.indexed_node(source)?;
    let collection_type = crate::value_custody::places::declared_place_type(
        program,
        machine,
        Some(state),
        indexed.collection,
    )?;
    let (element, extent) = match program.type_reference_table.type_reference(collection_type) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => (
            *element_type,
            ByteRegionExtent::Fixed(i64::try_from(*length).ok()?),
        ),
        TypeReferenceNode::Slice { element_type } => (*element_type, ByteRegionExtent::Descriptor),
        _ => return None,
    };
    (super::exact_scalar_representation_type(program, element) == Some(PrimitiveType::U8))
        .then_some(IndexedByteRegion {
            start: indexed.index,
            element,
            extent,
        })
}

/// An arbitrary write through a raw view must leave every original byte valid.
/// Source domains are not erased merely because the element carrier is u8.
pub(super) fn byte_region_allows_arbitrary_writes(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
) -> bool {
    let Some(region) = indexed_byte_region(program, machine, state, source) else {
        return false;
    };
    let raw = super::scalar_representation::MutableScalarRepresentationFacts {
        domains: Vec::new(),
        values: super::scalar_representation::ScalarRepresentationSet::ExactBitPatterns(
            super::scalar_representation::full_scalar_bit_patterns(PrimitiveType::U8),
        ),
    };
    super::scalar_representation::mutable_scalar_representation_facts(program, region.element)
        .is_some_and(|facts| {
            super::scalar_representation::scalar_representation_facts_imply(program, &raw, &facts)
        })
}

/// Authored coordinates of a slice representation view. This identifies the
/// source occurrence; it grants no bounds, representation or borrow authority.
/// The ordinary recast judgment and the emitted region obligation own those.
#[derive(Debug, Clone, Copy)]
pub struct SliceRecastByteRegion {
    pub collection: ExpressionHandle,
    pub source: ExpressionHandle,
    pub start: ExpressionHandle,
    pub extent: u64,
    pub element: TypeReferenceHandle,
    pub access: language_core::ReferenceAccess,
}

pub fn slice_recast_byte_region(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<SliceRecastByteRegion> {
    let authored = super::slice_recast_source(program, expression)?;
    let source = authored.source;
    let indexed = program.expression_table.indexed_node(source)?;
    let collection_type = crate::value_custody::places::declared_place_type(
        program,
        machine,
        Some(state),
        indexed.collection,
    )?;
    let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Literal(length),
        ..
    } = program.type_reference_table.type_reference(collection_type)
    else {
        return None;
    };
    if super::exact_scalar_representation_type(program, *element_type) != Some(PrimitiveType::U8) {
        return None;
    }
    Some(SliceRecastByteRegion {
        collection: indexed.collection,
        source,
        start: indexed.index,
        extent: u64::try_from(*length).ok()?,
        element: authored.element,
        access: authored.access,
    })
}

/// Exact value of an interior byte offset when its syntax or declared range
/// pins one value. A mere upper bound is sufficient for fixed-footprint views,
/// but an unsized slice with multi-byte elements additionally owes divisibility
/// of the complete remaining region; an interval cannot prove that congruence.
pub(super) fn exact_interior_byte_region_offset(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
) -> Option<i64> {
    let indexed = program.expression_table.indexed_node(source)?;
    if let Some(literal) = program.expression_table.integer_node(indexed.index) {
        return literal.value_i64().filter(|offset| *offset >= 0);
    }
    let type_reference = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        indexed.index,
    )?;
    let interval = crate::proof_contracts::arithmetic_domains::range_constraint_interval(
        program,
        type_reference,
    )?;
    let (Some(low), Some(high)) = (interval.low(), interval.high()) else {
        return None;
    };
    (low == high && low >= 0).then_some(low)
}

/// The interior byte-region judgment's three-way answer (owner-measured
/// diagnostic split 2026-07-11: a recognized shape whose OFFSET cannot be
/// bounded must say so -- it used to fall through to the form errors
/// ("not a scalar primitive or an eligible fixed record" / "source must be a
/// borrowed scalar place"), which misled: the real failure was the unproven
/// bound).
pub(super) enum InteriorByteRegion {
    /// Not `<[u8; N] place>[k]` at all -- fall through to the other source
    /// classes and their form messages.
    NotInteriorShape,
    /// The shape is right, but no route bounds the runtime offset.
    OffsetUnproven {
        offset_display: String,
        region_length: i64,
    },
    /// `k` (or its proven upper bound) and `N`.
    Bounded { offset: i64, region_length: i64 },
}

/// Fixed-footprint admission is deliberately separate from an exact suffix
/// extent. A descriptor's lower length bound can authorize a scalar or record
/// read, but must never become the length of a newly formed unsized view.
pub(super) enum ByteRegionFootprint {
    NotByteRegion,
    Contained,
    Rejected(String),
}

pub(super) fn byte_region_footprint(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    source: ExpressionHandle,
    footprint: i64,
) -> ByteRegionFootprint {
    let fixed =
        interior_byte_region_source(program, machine, state, statement_index, source, footprint);
    match fixed {
        InteriorByteRegion::Bounded {
            offset,
            region_length,
        } => {
            return if offset
                .checked_add(footprint)
                .is_some_and(|end| end <= region_length)
            {
                ByteRegionFootprint::Contained
            } else {
                ByteRegionFootprint::Rejected(format!(
                    "the recast target needs {footprint} bytes at offset {offset}, but the region holds {region_length} -- the view would read past the buffer"
                ))
            };
        }
        InteriorByteRegion::OffsetUnproven {
            offset_display,
            region_length,
        } => {
            return ByteRegionFootprint::Rejected(format!(
                "cannot bound the recast offset `{offset_display}` -- the region holds {region_length} bytes, but no declared range, dominating incoming guard, or boundary-ensures witness bounds the offset below the footprint"
            ));
        }
        InteriorByteRegion::NotInteriorShape => {}
    }
    if !indexed_byte_region(program, machine, state, source)
        .is_some_and(|region| matches!(region.extent, ByteRegionExtent::Descriptor))
    {
        return ByteRegionFootprint::NotByteRegion;
    }
    if crate::proof_contracts::arithmetic_domains::occurrence_collection_footprint_fits(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        source,
        footprint,
    ) {
        ByteRegionFootprint::Contained
    } else {
        ByteRegionFootprint::Rejected(format!(
            "cannot prove the recast's {footprint}-byte footprint fits the live byte slice: the offset must be nonnegative and offset + {footprint} must not exceed its length"
        ))
    }
}

/// Rungs B/C1's interior source: `<[u8; N] place>[k]`. Shape is recognized
/// FIRST (byte-element fixed array, literal length); the offset bound then
/// comes from a literal, the declared range, the dominating incoming
/// guard, or the boundary-ensures witness. `footprint` is the number of bytes
/// the view occupies at the offset (zero for a slice over the remainder).
pub(super) fn interior_byte_region_source(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    source: ExpressionHandle,
    footprint: i64,
) -> InteriorByteRegion {
    let Some(IndexedByteRegion {
        start,
        extent: ByteRegionExtent::Fixed(region_length),
        ..
    }) = indexed_byte_region(program, machine, state, source)
    else {
        return InteriorByteRegion::NotInteriorShape;
    };

    // Standing declaration bounds hold at every read. Arrival requirements,
    // predecessor guards and call postconditions instead pass through the
    // shared effect walk up to this exact selector occurrence, so a write
    // before formation retires them while a later write cannot retarget it.
    let offset = match program.expression_table.expression(start) {
        ExpressionNode::Integer(literal) => literal.value_i64().filter(|offset| *offset >= 0),
        _ => proved_offset_upper_bound(
            program,
            machine,
            state,
            statement_index,
            start,
            region_length - footprint,
        ),
    };
    match offset {
        Some(offset) => InteriorByteRegion::Bounded {
            offset,
            region_length,
        },
        None => InteriorByteRegion::OffsetUnproven {
            offset_display: program.expression_table.display_name(start),
            region_length,
        },
    }
}

/// The tightest upper bound the program states on a runtime byte offset.
/// Three routes answer and the tightest wins: the offset place's standing
/// bounds (its declared range, an immutable parameter's `requires`, or its
/// data's `where` facts), the structural walk over a composite offset's own
/// operands (`k * 2`), and the facts standing at this exact occurrence.
///
/// Those routes read interval endpoints, which lose relations between
/// places: `size == cap && cap <= 8` bounds `size` only through `cap`. When
/// no endpoint reaches `needed_bound` (the largest offset the view fits at),
/// the live relational judgment at the same occurrence is asked for exactly
/// that bound. It sees the relations the effect walk retained, so a write
/// that retires the equality also retires the bound.
///
/// Each answer is an UPPER bound, so only one narrower than the offset
/// place's own integer carrier states anything about the program: the
/// carrier maximum is what the type already says, and because the tightest
/// bound wins, an unnarrowed `u32` offset otherwise reached the author as the
/// literal offset 4294967295 -- a number no program contains. Rejected here,
/// the unproven-offset judgment names the three discharge routes instead.
fn proved_offset_upper_bound(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    index: ExpressionHandle,
    needed_bound: i64,
) -> Option<i64> {
    let declared_high = crate::proof_contracts::arithmetic_domains::standing_integer_interval(
        program, machine, state, index,
    )
    .and_then(|interval| {
        let high = interval.high()?;
        (interval.low().is_none_or(|low| low >= 0) && high >= 0).then_some(high)
    });
    let composite_high =
        super::offset_bounds::composite_offset_upper_bound(program, machine, state, index);
    let occurrence_high =
        crate::proof_contracts::arithmetic_domains::occurrence_integer_expression_bounds(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            index,
        )
        .and_then(|(low, high)| (low >= 0).then_some(high));
    let interval_high = [declared_high, composite_high, occurrence_high]
        .into_iter()
        .flatten()
        .filter_map(|high| {
            crate::proof_contracts::arithmetic_domains::upper_bound_beyond_place_carrier(
                program, machine, state, index, high,
            )
        })
        .min();
    if interval_high.is_some_and(|high| high <= needed_bound) || needed_bound < 0 {
        return interval_high;
    }
    let relational_high =
        crate::proof_contracts::arithmetic_domains::occurrence_integer_expression_fits_range(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            index,
            &numerics::bignum::BigInt::from_i64(0),
            &numerics::bignum::BigInt::from_i64(needed_bound),
        )
        .then_some(needed_bound);
    relational_high.or(interval_high)
}

pub(super) fn push_offset_unproven(
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
    offset_display: &str,
    region_length: i64,
    source_span: source::SourceSpan,
) {
    diagnostics.push(
        Diagnostic::error(format!(
            "{context}: cannot bound the recast offset `{offset_display}` -- the region holds \
             {region_length} bytes, but no declared range, dominating incoming guard, or \
             boundary-ensures witness bounds the offset below the footprint. Bound it: declare \
             a range on the offset param, guard the transition arm (`transition \
             {offset_display} <= K {{ true -> ... }}`), or `ensures`-bound the boundary \
             out-param that feeds it",
        ))
        .with_source_span(source_span),
    );
}

/// Mutable byte views must preserve every target fact after arbitrary writes.
/// Until the general bidirectional entailment solver lands, the complete
/// decidable subset is a raw named record whose transitive fields are raw
/// fact-free scalar primitives or records with no default-domain facts.
fn record_view_is_fact_free(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
    visiting: &mut HashSet<SymbolIdentity>,
) -> bool {
    if !symbol.is_valid() {
        return false;
    }
    let symbol_identity = (symbol.arena_index(), symbol.generation());
    if !visiting.insert(symbol_identity) {
        return false;
    }
    let Some(data) = crate::declarations::data_queries::data_definition_by_symbol(program, symbol)
    else {
        visiting.remove(&symbol_identity);
        return false;
    };
    if !data.where_facts.is_empty() || data.zero_gated {
        visiting.remove(&symbol_identity);
        return false;
    }
    for member in program.data_members(data) {
        let Some(field) = member.as_field() else {
            visiting.remove(&symbol_identity);
            return false;
        };
        if !record_view_type_is_fact_free(program, field.type_reference, visiting) {
            visiting.remove(&symbol_identity);
            return false;
        }
    }
    visiting.remove(&symbol_identity);
    true
}

pub(super) fn record_view_type_is_fact_free(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<SymbolIdentity>,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            PrimitiveType::from_name(name.as_str())
                .is_some_and(|primitive| primitive != PrimitiveType::Bool)
                || record_view_is_fact_free(program, *symbol, visiting)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => record_view_type_is_fact_free(program, *element_type, visiting),
        _ => false,
    }
}
