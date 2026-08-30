//! §5b recast judgment, rung A (programmable-layouts brief): `&x as &T`
//! re-views a place's bytes under a second stated shape. Legality is a
//! STATIC judgment -- a bad relation is a compile error, never unsafety.
//!
//! The scalar rung serves the core end-to-end and fences the rest loudly:
//!
//! - **Served:** a recast between fixed-width scalar primitives of EQUAL byte
//!   size (`&i32 as &f32`, `&mut u32 as &mut i32`), or a scalar view into a
//!   proven in-bounds `[u8; N]` region, bound as the direct
//!   initializer of a reference-typed let whose stated type restates the
//!   target. Shared views may weaken source facts. Mutable scalar views admit
//!   fact-free types, normalized domain conjunctions that imply one another
//!   in BOTH directions, or integer ranges that denote the same normalized
//!   bit-pattern set. Same-carrier float ranges compose by numeric interval
//!   inclusion. A shared view may forget a float interval into an unconstrained
//!   equal-width bit carrier, but it never justifies cross-carrier mutable
//!   equivalence. Merely equal-looking cross-carrier predicates remain fenced.
//!   Byte-region aggregate views require recursively
//!   fact-free target shapes, including top-level and nested literal-length
//!   fixed arrays. Mutable typed aggregate aliases may retain facts when source
//!   and target have identical geometry and representation-equivalent leaves;
//!   shared aliases may weaken facts. The same repeated-leaf judgment serves
//!   unsized slices of aggregate elements over a complete typed fixed array;
//!   element stride includes layout padding rather than repacking the leaves.
//!   Lowering is address identity:
//!   native reads/writes the place through the stated type; the interpreter
//!   bit-reinterprets both sides of the alias or assembles/writes the complete
//!   little-endian byte-region footprint.
//! - **Fenced (deeper byte-view rung, L4/L5):** remaining dynamically-sized
//!   shapes beyond complete-source and proven interior unsized slices
//!   (byte-granular tiling over plan-laid layouts), and recasts in non-let
//!   positions. A runtime interior byte offset cannot establish multi-byte
//!   element tiling until its congruence is proved; an exact offset can.
//! - **Refused absolutely:** targets that would ESTABLISH a fact the bytes
//!   don't prove (`bool`'s 0/1, text encodings) -- establishing facts is a
//!   MINT's job (fallible, case-returning), never a recast's.
//!
//! The companion rule closes the accidental-pun hole this judgment would
//! otherwise be bypassed by: a reference-typed let whose initializer is a
//! BARE borrow of a differently-typed scalar place (`let v: &f32 =
//! &self.x` over an i64) used to compile unjudged and DIVERGE (native
//! bit-punned, the interpreter delivered the semantic value; pinned by
//! tests/omega/fail/recast/reference_let_pun_requires_recast). Re-viewing is
//! spelled `as`; the bare mismatch refuses.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use std::collections::HashSet;

mod offset_bounds;
mod qualification;
mod raw_byte_region;
mod record_representation;
mod scalar_representation;

use qualification::{judge_qualification_cast, judge_statement_qualification_casts};

use raw_byte_region::{
    InteriorByteRegion, exact_interior_byte_region_offset, interior_byte_region_source,
    push_offset_unproven, record_view_type_is_fact_free,
};

use record_representation::{
    mutable_record_representations_equivalent, mutable_type_representation,
    record_representation_implies, repeat_representation, representation_is_exactly_tiled,
    shared_projection_type_representation,
};

use scalar_representation::{
    MutableScalarRepresentationFacts, ScalarRepresentationSet, full_scalar_bit_patterns,
    mutable_scalar_representation_facts, mutable_scalar_representation_facts_equivalent,
    scalar_representation_facts_imply,
};

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
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    local: &psi_typed_trees::statement::TableLocalData,
) -> Option<ValidatedLiteralIndexedRecastFootprint> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
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
        *referee,
        access.is_exclusive(),
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

fn literal_indexed_recast_target_size(
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

    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(target_type)
    else {
        return None;
    };
    if !closed_fact_free_record_symbol_is_eligible(program, *symbol, &mut Vec::new()) {
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
        length: psi_typed_trees::types::FixedArrayLength::Literal(length),
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
            closed_fact_free_record_symbol_is_eligible(program, *symbol, &mut Vec::new())
                .then_some(LiteralFixedArrayTerminal::ClosedRecord)
        }
        TypeReferenceNode::FixedArray { .. } => {
            literal_fixed_array_target_terminal(program, *element_type)
        }
        _ => None,
    }
}

fn exact_primitive_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    PrimitiveType::from_name(name.as_str()).filter(|primitive| name.as_str() == primitive.name())
}

fn closed_fact_free_record_symbol_is_eligible(
    program: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> bool {
    if !symbol.is_valid() || visiting.contains(&symbol) {
        return false;
    }
    visiting.push(symbol);
    let eligible = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)
        .is_some_and(|data| {
            let members = program.data_members(data);
            data.supply_mode == psi_language_semantics::DataSupplyMode::CheckedShape
                && data.lifetime_parameters.is_empty()
                && program.data_type_parameters(data).is_empty()
                && closed_record_generic_origin_is_eligible(program, data)
                && data.quotient.is_none()
                && data.where_facts.is_empty()
                && !data.zero_gated
                && psi_typed_trees::data::DataDefinition::shape_kind_from_members(members)
                    == psi_typed_trees::data::DataShapeKind::Record
                && members.iter().all(|member| {
                    let psi_typed_trees::data::DataMember::Field(field) = member else {
                        return false;
                    };
                    !field.relevance.is_erased()
                        && closed_fact_free_record_field_is_eligible(
                            program,
                            field.type_reference,
                            visiting,
                        )
                })
        });
    visiting.pop();
    eligible
}

fn closed_record_generic_origin_is_eligible(
    program: &TypedTrees,
    instance: &psi_typed_trees::data::DataDefinition,
) -> bool {
    let Some(origin) = instance.generic_instance else {
        return true;
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(origin)
    else {
        return false;
    };
    if !base_symbol.is_valid() || *base_symbol == instance.symbol || !lifetime_arguments.is_empty()
    {
        return false;
    }
    let Some(base) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *base_symbol)
    else {
        return false;
    };
    let parameters = program.data_type_parameters(base);
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    base.supply_mode == psi_language_semantics::DataSupplyMode::CheckedShape
        && base.generic_instance.is_none()
        && base.quotient.is_none()
        && psi_typed_trees::data::DataDefinition::shape_kind_from_members(
            program.data_members(base),
        ) == psi_typed_trees::data::DataShapeKind::Record
        && !parameters.is_empty()
        && parameters.len() == arguments.len()
        && parameters
            .iter()
            .zip(arguments)
            .all(|(parameter, argument)| match parameter.kind {
                psi_typed_trees::data::TypeParameterKind::Type => {
                    closed_raw_generic_type_argument_is_eligible(program, *argument)
                }
                psi_typed_trees::data::TypeParameterKind::Const { type_reference } => {
                    closed_raw_generic_const_argument_is_eligible(
                        program,
                        type_reference,
                        *argument,
                    )
                }
                psi_typed_trees::data::TypeParameterKind::Machine { .. }
                | psi_typed_trees::data::TypeParameterKind::Proposition { .. } => false,
            })
}

fn closed_raw_generic_const_argument_is_eligible(
    program: &TypedTrees,
    parameter_type: TypeReferenceHandle,
    argument: TypeReferenceHandle,
) -> bool {
    let Some(primitive) = exact_primitive_type(program, parameter_type)
        .filter(|primitive| primitive.accepts_integer_literal())
    else {
        return false;
    };
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        return false;
    };
    if symbol.is_valid() {
        return false;
    }
    let Ok(value) = name.as_str().parse::<i128>() else {
        return false;
    };
    name.as_str() == value.to_string()
        && crate::type_references::const_integer_value_fits_primitive(primitive, value)
}

fn closed_raw_generic_type_argument_is_eligible(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { symbol, .. } => {
            exact_primitive_type(program, argument).is_some()
                || (symbol.is_valid()
                    && program.data_definitions().iter().any(|data| {
                        data.symbol == *symbol
                            && data.lifetime_parameters.is_empty()
                            && program.data_type_parameters(data).is_empty()
                    }))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } => *length > 0 && closed_raw_generic_type_argument_is_eligible(program, *element_type),
        _ => false,
    }
}

fn closed_fact_free_record_field_is_eligible(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> bool {
    if let Some(primitive) = exact_primitive_type(program, type_reference) {
        return primitive != PrimitiveType::Bool && primitive.scalar_byte_size().is_some();
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            closed_fact_free_record_symbol_is_eligible(program, *symbol, visiting)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(_),
        } => closed_fact_free_record_field_is_eligible(program, *element_type, visiting),
        _ => false,
    }
}

pub(crate) fn validate_recasts(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    // The blessed positions: direct initializers of reference-typed lets
    // (mirrors the D14 literal gate's shape -- collect the legal roots,
    // then sweep the whole expression table for strays).
    let mut blessed: Vec<ExpressionHandle> = Vec::new();
    // Qualification casts judged WITH machine/state context (the declared-
    // range discharge needs the value's declared type); the positional
    // sweep below only judges strays (literal-only).
    let mut judged_qualifications: Vec<ExpressionHandle> = Vec::new();
    let dynamic_rebindings =
        crate::traits::collect_dynamic_conformance_selections(program).unwrap_or_default();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                judge_statement_qualification_casts(
                    program,
                    machine,
                    state,
                    statement,
                    &mut judged_qualifications,
                    diagnostics,
                );
                if let StatementNode::Assignment(assignment) = statement {
                    let value = strip_mutable(program, assignment.value);
                    if dynamic_rebindings.iter().any(|selection| {
                        selection.machine == machine.symbol
                            && selection.state == state.symbol
                            && selection.statement_index == statement_index
                            && selection.occurrence == value
                            && matches!(
                                program.expression_table.expression(value),
                                ExpressionNode::Cast(cast)
                                    if cast.form.is_recast()
                                        && crate::traits::dynamic_trait_symbol(
                                            program,
                                            cast.target_type,
                                        )
                                        .is_some()
                            )
                    }) {
                        blessed.push(value);
                    }
                }
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if !local.type_reference.is_valid() || !local.initial_value.is_valid() {
                    continue;
                }
                let TypeReferenceNode::Reference {
                    referee, access, ..
                } = program
                    .type_reference_table
                    .type_reference(local.type_reference)
                else {
                    continue;
                };
                // The `&mut x as &mut T` spelling parses as Mutable(Cast(..)):
                // the unary `&mut` wraps the postfix chain. Look through it
                // so the blessed root is the CAST node the sweep checks.
                let initializer = strip_mutable(program, local.initial_value);
                match program.expression_table.expression(initializer) {
                    ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                        blessed.push(initializer);
                        if crate::traits::dynamic_trait_symbol(program, cast.target_type).is_none()
                        {
                            judge_scalar_recast(
                                program,
                                machine,
                                state,
                                cast,
                                *referee,
                                access.is_exclusive(),
                                diagnostics,
                            );
                        }
                    }
                    _ => {
                        report_unspelled_reference_pun(
                            program,
                            machine,
                            state,
                            local.initial_value,
                            *referee,
                            diagnostics,
                        );
                    }
                }
            }
        }
    }

    // Position sweep: a recast anywhere but a blessed root refuses. (The
    // parser builds recast nodes only from the `as &` spelling, and every
    // expression is reachable from some statement, so this catches guard /
    // argument / nested positions uniformly.)
    for (handle, node) in program.expression_table.expression_entries() {
        if let ExpressionNode::Cast(cast) = node
            && cast.form.is_recast()
            && !blessed.contains(&handle)
        {
            diagnostics.push(Diagnostic::error(
                "a recast binds to a reference-typed let (`let v: &T = &x as &T;`) in this \
                 rung; inline re-views land with the byte-view rung"
                    .to_string(),
            ));
        }
        // STR4 checked plans, slice 3 (decision 19): a NON-policy `in <Name>`
        // cast suffix is the semantic-domain QUALIFICATION spelling. It is
        // recognized here but its MINT rung (introduction authority +
        // predicate discharge) has not landed -- the staged fence names the
        // declared domain; an unmatched name gets the honest unknown error
        // the parser used to give (now with the declaration check the parser
        // could not perform).
        if let ExpressionNode::Cast(cast) = node
            && cast.semantic_domain.count() > 0
            && !judged_qualifications.contains(&handle)
        {
            judge_qualification_cast(program, None, cast, diagnostics);
        }
    }
}

/// The rung-A judgment for one blessed `&x as &T` (§5b rules 1-4 over the
/// scalar fragment).
fn judge_scalar_recast(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    cast: &TableCastExpression,
    let_referee: TypeReferenceHandle,
    let_is_mutable: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let context = format!(
        "machine `{}` state `{}`",
        machine.name.as_str(),
        state.name.as_str()
    );

    let mutable_recast = cast.form == psi_language_core::cast_form::CastForm::RecastMutable;
    if mutable_recast != let_is_mutable {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: recast borrow polarity must agree -- use `&x as &T` for a shared \
             view or `&mut x as &mut T` for a writable view"
        )));
        return;
    }

    let source = strip_mutable(program, cast.value);
    let source_type = crate::places::declared_place_type_raw(program, machine, Some(state), source);
    let source_placed = source_type
        .and_then(|type_reference| program.placed_view_plan_for_type_reference(type_reference));
    let target_placed = program.placed_view_plan_for_type_reference(cast.target_type);
    if source_placed.is_some() || target_placed.is_some() {
        let source_name = source_placed
            .map(|view| view.data_name.as_str())
            .unwrap_or("non-placed storage");
        let target_name = target_placed
            .map(|view| view.data_name.as_str())
            .unwrap_or("non-placed storage");
        diagnostics.push(Diagnostic::error(format!(
            "{context}: placed-view recast from `{source_name}` to `{target_name}` is unavailable; retain the underlying qualified extent borrow and explicitly admit the intended placement"
        )));
        return;
    }

    // Target: a fixed-width scalar or recursively fixed aggregate, restated
    // exactly by the let. Structural targets are semantic type references;
    // their cached display spelling never participates in the judgment.
    let target_name = program
        .named_type_reference(cast.target_type)
        .map(|name| name.as_str().to_string())
        .unwrap_or_else(|| program.display_type_reference(cast.target_type));
    if let TypeReferenceNode::Slice { element_type } = program
        .type_reference_table
        .type_reference(cast.target_type)
    {
        judge_slice_recast(
            program,
            machine,
            state,
            cast,
            *element_type,
            let_referee,
            mutable_recast,
            diagnostics,
            &context,
        );
        return;
    }
    if program.primitive_type_reference(cast.target_type).is_none() {
        // RUNG C2/C3: a recursively fixed RECORD or literal-length ARRAY
        // target. The same normalized representation supplies size/alignment
        // and scalar-leaf facts; this is the top-level-array continuation of
        // the array fields records already admit.
        // Stored-width integer leaves are admissible in a mutable BYTE-REGION
        // view because every concrete assignment remains a proved-fit lowering
        // obligation. Typed aggregate aliases still reject them below: those
        // require one representation valid for arbitrary writes in both
        // directions, not per-write encoding evidence.
        let target_representation =
            shared_projection_type_representation(program, cast.target_type);
        if let Some(target_representation) = target_representation {
            if program.normalized_type_identity(let_referee)
                != program.normalized_type_identity(cast.target_type)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{context}: the let's declared type must restate the recast target \
                     `&{}{target_name}`",
                    if mutable_recast { "mut " } else { "" },
                )));
                return;
            }
            let interior = interior_byte_region_source(program, machine, state, source);
            if let InteriorByteRegion::OffsetUnproven {
                offset_display,
                region_length,
            } = &interior
            {
                push_offset_unproven(diagnostics, &context, offset_display, *region_length);
                return;
            }
            if let InteriorByteRegion::Bounded {
                offset,
                region_length,
            } = interior
            {
                if !record_view_type_is_fact_free(program, cast.target_type, &mut HashSet::new()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: byte-region recast target `{target_name}` must be recursively \
                         fact-free; unchecked bytes cannot establish constrained fields, bool, \
                         or record invariants{}",
                        if mutable_recast {
                            "; mutable views require fact implication in BOTH directions"
                        } else {
                            ""
                        },
                    )));
                    return;
                }
                let Some(end) = offset.checked_add(target_representation.size as i64) else {
                    return;
                };
                if end > region_length {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: the recast target `{target_name}` needs {} bytes at offset \
                         {offset}, but the region holds {region_length} -- the view would read \
                         past the buffer (§5b rule 1 is byte-granular)",
                        target_representation.size,
                    )));
                }
                return;
            }
            let source_type =
                crate::places::declared_place_type_raw(program, machine, Some(state), source);
            if let Some(source_type) = source_type
                && let Some(source_representation) =
                    mutable_type_representation(program, source_type)
            {
                if target_representation.has_stored_integer_projection
                    || source_representation.has_stored_integer_projection
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: stored-width integer decoding is admitted only for a shared \
                         view over a proven byte region; typed aggregate aliases require identical \
                         storage representations"
                    )));
                    return;
                }
                let compatible = if mutable_recast {
                    mutable_record_representations_equivalent(
                        program,
                        &source_representation,
                        &target_representation,
                    )
                } else {
                    record_representation_implies(
                        program,
                        &source_representation,
                        &target_representation,
                    )
                };
                if compatible {
                    return;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "{context}: {} aggregate aliases require identical layout geometry and {}; \
                     the source and target `{target_name}` are not representation-compatible",
                    if mutable_recast { "mutable" } else { "shared" },
                    if mutable_recast {
                        "leaf fact implication in BOTH directions"
                    } else {
                        "source leaf facts implying every target leaf fact"
                    },
                )));
                return;
            }
        }
        diagnostics.push(Diagnostic::error(format!(
            "{context}: recast target `{target_name}` is not a scalar primitive or an \
             eligible fixed aggregate over a byte region or typed aggregate place; deeper shapes \
             land with the byte-view rung"
        )));
        return;
    }
    let Some(target) = program.primitive_type_reference(cast.target_type) else {
        return;
    };
    let let_primitive = crate::places::unwrapped_type_reference(program, let_referee)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped));
    if let_primitive != Some(target) {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: the let's declared type must restate the recast target `&{target_name}` \
             (the stated shape is the single source of truth for reads through the view)"
        )));
        return;
    }

    // Source: a scalar place of the SAME byte width (§5b rule 1: same total
    // size; scalar alignment follows from size). Facts on the source are
    // fine under a shared view (weakening).
    // RUNG B: an INTERIOR recast into a `[u8; N]` region at a STATIC offset
    // (`&self.buf[4] as &u32`): the target's footprint must fit the
    // remaining bytes (`k + size(T) <= N`). Byte buffers carry no facts and
    // align to 1; both ISAs' scalar loads tolerate the resulting unaligned
    // addresses on normal memory. The stated-type-restated check above
    // already ran; the same-width rule below does NOT apply (the source
    // region is byte-granular).
    let interior = interior_byte_region_source(program, machine, state, source);
    if let InteriorByteRegion::OffsetUnproven {
        offset_display,
        region_length,
    } = &interior
    {
        push_offset_unproven(diagnostics, &context, offset_display, *region_length);
        return;
    }
    if let InteriorByteRegion::Bounded {
        offset,
        region_length,
    } = interior
    {
        let target_facts = mutable_scalar_representation_facts(program, let_referee);
        let raw_facts = MutableScalarRepresentationFacts {
            domains: Vec::new(),
            values: ScalarRepresentationSet::ExactBitPatterns(full_scalar_bit_patterns(target)),
        };
        let compatible = target_facts.as_ref().is_some_and(|target_facts| {
            if mutable_recast {
                mutable_scalar_representation_facts_equivalent(program, &raw_facts, target_facts)
            } else {
                scalar_representation_facts_imply(program, &raw_facts, target_facts)
            }
        });
        if !compatible {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: a {} recast {}; a raw byte region cannot establish the target's \
                 representation facts",
                if mutable_recast { "mutable" } else { "shared" },
                if mutable_recast {
                    "must prove fact implication in BOTH directions"
                } else {
                    "may weaken established facts but cannot strengthen them"
                },
            )));
            return;
        }
        let Some(target_size) = target.scalar_byte_size() else {
            return;
        };
        let Some(end) = offset.checked_add(target_size as i64) else {
            return;
        };
        if end > region_length {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: the recast target `{target_name}` needs {target_size} bytes at offset {offset}, but the region holds {region_length} -- the view would read past the buffer (§5b rule 1 is byte-granular)",
            )));
        }
        return;
    }
    let source_type = crate::places::declared_place_type_raw(program, machine, Some(state), source);
    let source_primitive =
        source_type.and_then(|type_reference| program.primitive_type_reference(type_reference));
    let Some(source_primitive) = source_primitive else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a recast re-views a PLACE's bytes -- the source must be a borrowed \
             scalar place (`&x as &{target_name}`); record sources and temporaries land \
             with the byte-view rung"
        )));
        return;
    };
    let (Some(source_size), Some(target_size)) = (
        source_primitive.scalar_byte_size(),
        target.scalar_byte_size(),
    ) else {
        return;
    };
    if source_size != target_size {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a recast re-views the SAME bytes, so the shapes must agree on \
             size (§5b rule 1) -- source `{}` is {source_size} bytes, target \
             `{target_name}` is {target_size} bytes",
            source_primitive.name()
        )));
        return;
    }

    let source_facts = source_type
        .and_then(|type_reference| mutable_scalar_representation_facts(program, type_reference));
    let target_facts = mutable_scalar_representation_facts(program, let_referee);
    let target_is_fact_free = target_facts.as_ref().is_some_and(|target_facts| {
        target_facts.domains.is_empty()
            && target_facts.values
                == ScalarRepresentationSet::ExactBitPatterns(full_scalar_bit_patterns(target))
    });
    let compatible = if !mutable_recast && target_is_fact_free {
        // A shared view may always forget source facts. This remains safe even
        // when the source uses a fact family (such as a float interval) whose
        // precise representation set is not yet modeled.
        true
    } else {
        source_facts
            .as_ref()
            .zip(target_facts.as_ref())
            .is_some_and(|(source_facts, target_facts)| {
                if mutable_recast {
                    mutable_scalar_representation_facts_equivalent(
                        program,
                        source_facts,
                        target_facts,
                    )
                } else {
                    scalar_representation_facts_imply(program, source_facts, target_facts)
                }
            })
    };
    if !compatible {
        diagnostics.push(Diagnostic::error(if mutable_recast {
            format!(
                "{context}: a mutable recast must prove fact implication in BOTH directions; \
                 source and target constraints are not proven representation-equivalent"
            )
        } else {
            format!(
                "{context}: a shared recast may weaken established facts but cannot strengthen \
                 them; source facts do not establish the target representation"
            )
        }));
    }
}

#[allow(clippy::too_many_arguments)]
fn judge_slice_recast(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    cast: &TableCastExpression,
    element_type: TypeReferenceHandle,
    let_referee: TypeReferenceHandle,
    mutable_recast: bool,
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
) {
    let target_label = program.display_type_reference(cast.target_type);
    if program.normalized_type_identity(let_referee)
        != program.normalized_type_identity(cast.target_type)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: the let's declared type must restate the recast target \
             `&{}{target_label}`",
            if mutable_recast { "mut " } else { "" },
        )));
        return;
    }

    let source = strip_mutable(program, cast.value);
    let Some(element_representation) = mutable_type_representation(program, element_type) else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice target `{target_label}` needs a fixed-layout element type"
        )));
        return;
    };

    // An interior slice starts at one byte of a proven `[u8; N]` region and
    // consumes every remaining byte. This is the dynamically-sized companion
    // to the fixed aggregate/scalar interior rungs: raw bytes may establish
    // only recursively fact-free, exactly tiled element representations.
    let interior = interior_byte_region_source(program, machine, state, source);
    if let InteriorByteRegion::OffsetUnproven {
        offset_display,
        region_length,
    } = &interior
    {
        push_offset_unproven(diagnostics, context, offset_display, *region_length);
        return;
    }
    if let InteriorByteRegion::Bounded {
        offset,
        region_length,
    } = interior
    {
        let target_tiled = representation_is_exactly_tiled(&element_representation);
        let target_fact_free =
            record_view_type_is_fact_free(program, element_type, &mut HashSet::new());
        if !target_tiled || !target_fact_free {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: interior slice recast `{target_label}` requires a recursively \
                 fact-free element whose scalar leaves exactly tile its byte stride; raw \
                 storage cannot establish element facts or implicit padding"
            )));
            return;
        }
        if offset > region_length {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: interior slice recast starts at byte {offset}, past the \
                 {region_length}-byte source region"
            )));
            return;
        }
        let exact_offset = exact_interior_byte_region_offset(program, machine, state, source);
        if let Some(exact_offset) = exact_offset {
            let remaining = region_length - exact_offset;
            if remaining < 0
                || element_representation.size == 0
                || remaining as usize % element_representation.size != 0
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{context}: interior slice target `{target_label}` does not exactly tile \
                     the {remaining} bytes remaining at offset {exact_offset} with {}-byte \
                     elements",
                    element_representation.size,
                )));
            }
        } else if element_representation.size != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: cannot prove exact tiling for interior slice `{target_label}`: \
                 the runtime byte offset may leave a remainder for {}-byte elements; use a \
                 statically exact offset or validate the dynamic region before establishing \
                 the typed slice",
                element_representation.size,
            )));
        }
        return;
    }

    let Some(source_type) =
        crate::places::declared_place_type_raw(program, machine, Some(state), source)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a slice recast re-views a fixed-layout PLACE's complete bytes"
        )));
        return;
    };
    let Some(source_representation) = mutable_type_representation(program, source_type) else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice recast source `{}` has no fixed representation",
            program.display_type_reference(source_type)
        )));
        return;
    };
    if element_representation.size == 0
        || source_representation.size % element_representation.size != 0
    {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice target `{target_label}` does not exactly tile the source's {} \
             bytes with {}-byte elements",
            source_representation.size, element_representation.size,
        )));
        return;
    }

    let element_count = source_representation.size / element_representation.size;
    let Some(target_representation) = repeat_representation(&element_representation, element_count)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice target `{target_label}` representation overflows"
        )));
        return;
    };

    let source_tiled = representation_is_exactly_tiled(&source_representation);
    let target_tiled = representation_is_exactly_tiled(&target_representation);
    let source_fact_free = record_view_type_is_fact_free(program, source_type, &mut HashSet::new());
    let target_fact_free =
        record_view_type_is_fact_free(program, element_type, &mut HashSet::new());
    let compatible = if source_tiled && target_tiled && target_fact_free {
        !mutable_recast || source_fact_free
    } else if mutable_recast {
        mutable_record_representations_equivalent(
            program,
            &source_representation,
            &target_representation,
        )
    } else {
        record_representation_implies(program, &source_representation, &target_representation)
    };
    if !compatible {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice recast `{target_label}` does not preserve exact byte tiling and {}; \
             raw storage cannot establish element facts",
            if mutable_recast {
                "fact implication in BOTH directions"
            } else {
                "source-to-target fact implication"
            },
        )));
    }
}

/// The companion hole-closer: a reference-typed let over a BARE borrow of a
/// scalar place whose type disagrees with the stated referee. Without this,
/// `let v: &f32 = &self.x;` (x: i64) is an accidental, unjudged recast --
/// native bit-puns while the interpreter delivers the semantic value.
fn report_unspelled_reference_pun(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    initializer: ExpressionHandle,
    let_referee: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = strip_mutable(program, initializer);
    let Some(source_primitive) =
        crate::places::declared_place_type(program, machine, Some(state), source)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
    else {
        return;
    };
    let Some(referee_primitive) = crate::places::unwrapped_type_reference(program, let_referee)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
    else {
        return;
    };
    if source_primitive != referee_primitive {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}`: reference initializer type `{}` must match the \
             stated `&{}`; re-viewing a place's bytes under another shape is spelled \
             `&x as &{}` (§5b recast)",
            machine.name.as_str(),
            state.name.as_str(),
            source_primitive.name(),
            referee_primitive.name(),
            referee_primitive.name(),
        )));
    }
}

fn strip_mutable(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => strip_mutable(program, inner.target),
        _ => expression,
    }
}
