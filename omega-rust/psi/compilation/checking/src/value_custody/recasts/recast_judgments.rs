//! Representation compatibility is independent from reference access.
//!
//! Formation retains the exact access mode; exclusivity is used only for the
//! reverse representation implication owed by both writable modes. In
//! particular, a write-only view neither observes the source contents nor
//! becomes readable merely because its validity judgment is bidirectional.

use crate::value_custody::recasts::raw_byte_region::{
    ByteRegionFootprint, InteriorByteRegion, byte_region_allows_arbitrary_writes,
    byte_region_footprint, exact_interior_byte_region_offset, interior_byte_region_source,
    push_offset_unproven, record_view_type_is_fact_free,
};
use crate::value_custody::recasts::record_eligibility::direct_record_view_type_is_fact_free;
use crate::value_custody::recasts::record_representation::{
    mutable_record_representations_equivalent, record_representation_implies,
    repeat_representation, representation_is_exactly_tiled, type_representation,
};
use crate::value_custody::recasts::representation_types::exact_scalar_representation_type;
use crate::value_custody::recasts::scalar_representation::{
    MutableScalarRepresentationFacts, ScalarRepresentationSet, full_scalar_bit_patterns,
    mutable_scalar_representation_facts, mutable_scalar_representation_facts_equivalent,
    narrow_to_integer_interval, scalar_representation_facts_imply,
};
use diagnostics::Diagnostic;
use std::collections::HashSet;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The rung-A judgment for one blessed `&x as &T` (§5b rules 1-4 over the
/// scalar fragment).
pub(crate) fn judge_scalar_recast(
    program: &dyn crate::CheckingInput,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    cast: &TableCastExpression,
    cast_handle: ExpressionHandle,
    let_referee: TypeReferenceHandle,
    receiving_access: language_core::ReferenceAccess,
    source_borrow_access: Option<language_core::ReferenceAccess>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let layout_context = crate::LayoutCheckingContext::for_owner(program, machine.symbol);
    let program: &dyn crate::CheckingInput = &layout_context;
    let source = strip_mutable(program, cast.value);
    let source_type = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        source,
    );
    let roots = source_type
        .into_iter()
        .chain([cast.target_type, let_referee]);
    let Ok(targets) = crate::layout_checking_targets(program, roots) else {
        diagnostics.push(Diagnostic::error(
            "recast has no complete exact layout alternative roster",
        ));
        return;
    };
    for target in targets {
        let alternative = crate::LayoutCheckingContext::new(program, target);
        judge_scalar_recast_alternative(
            &alternative,
            call_frames,
            machine,
            state,
            statement_index,
            cast,
            cast_handle,
            let_referee,
            receiving_access,
            source_borrow_access,
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn judge_scalar_recast_alternative(
    program: &dyn crate::CheckingInput,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    cast: &TableCastExpression,
    cast_handle: ExpressionHandle,
    let_referee: TypeReferenceHandle,
    receiving_access: language_core::ReferenceAccess,
    source_borrow_access: Option<language_core::ReferenceAccess>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let context = format!(
        "machine `{}` state `{}`",
        machine.name.as_str(),
        state.name.as_str()
    );
    let source_span = program.expression_table.source_span(cast_handle);

    let Some(view_access) = cast.form.recast_access() else {
        diagnostics.push(Diagnostic::error(
            "reference-view judgment requires a recast form",
        ));
        return;
    };
    // Exclusivity controls writable representation validity, never readable
    // permission. Keep the exact source/view/receiving axes at admission.
    let writable_view = view_access.is_exclusive();
    if view_access != receiving_access {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: recast borrow polarity must agree -- use `&x as &T` for a shared \
                 view or `&mut x as &mut T` for a writable view; `&write x as &write T` retains write-only access"
            ))
            .with_source_span(source_span),
        );
        return;
    }

    // The authored source borrow must spell the same access the view claims.
    // `&x as &mut T` mints an exclusive view from a shared borrow -- access
    // escalation; `&mut x as &T` mismatches in the other direction. A bare
    // `x as &T` has no source borrow to disagree with, and the target-side
    // `as &mut` suffix still marks the view exclusive for the loan walk.
    let source_borrow_access =
        source_borrow_access.or_else(|| match program.expression_table.expression(cast.value) {
            ExpressionNode::Borrow(borrow) => Some(borrow.access),
            _ => None,
        });
    if source_borrow_access.is_some_and(|source| source != view_access) {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: recast borrow polarity must agree -- use `&x as &T` for a shared \
                 view or `&mut x as &mut T` for a writable view; `&write x as &write T` retains write-only access"
            ))
            .with_source_span(source_span),
        );
        return;
    }

    let source = strip_mutable(program, cast.value);
    let source_type = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        source,
    );
    // The judgment concerns backing storage, not the permission descriptor.
    // Use the same root-reference crossing as aggregate representation; keep
    // referent constraints intact for scalar implication in both directions.
    let source_type = source_type
        .map(|source| crate::value_custody::places::assignment_value_type(program, source));
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
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: placed-view recast from `{source_name}` to `{target_name}` is unavailable; retain the underlying qualified extent borrow and explicitly admit the intended placement"
            ))
            .with_source_span(source_span),
        );
        return;
    }

    let shared_mutation = type_representation(
        program,
        program
            .type_reference_table
            .unconstrained_type_reference(cast.target_type),
    )
    .is_some_and(|representation| representation.permits_shared_mutation());
    let qualification_compatible = source_type.and_then(|source| {
        super::qualification_views::qualification_view_compatible(program, source, let_referee)
    });
    if qualification_compatible == Some(false) {
        diagnostics.push(Diagnostic::error(format!(
                "{context}: a qualification reference view must retain existing source qualifications required by its target; `as` cannot mint routed provenance",
            )).with_source_span(source_span));
        return;
    }
    if qualification_compatible == Some(true)
        && (writable_view || shared_mutation)
        && source_type.is_some_and(|source| {
            super::qualification_views::qualification_view_compatible(program, let_referee, source)
                != Some(true)
        })
    {
        let statement = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index);
        let declarations = statement
            .ok_or("the recast statement is unavailable")
            .and_then(|statement| {
                call_frames
                    .ok_or("the exact lender resolver is unavailable")?
                    .reference_lender_declarations_before_statement(machine, statement, cast.value)
            });
        let refusal = match declarations {
            Err(reason) => Some(reason),
            Ok(declarations) => declarations
                .iter()
                .any(|declaration| {
                    super::qualification_views::lender_qualifications_implied(
                        program,
                        let_referee,
                        *declaration,
                    ) != Some(true)
                })
                .then_some(
                    "the target does not preserve declared lender field/element qualifications",
                ),
        };
        if let Some(reason) = refusal {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: writable qualification erasure cannot preserve lender validity: {reason}"
            )).with_source_span(source_span));
            return;
        }
    }
    // Membership and representation are independent obligations. Even an
    // identical carrier must continue through place-specific field bounds,
    // access and geometry checks; a qualified view is not an early success.
    let representation_carrier = |reference| {
        if qualification_compatible == Some(true) {
            super::qualification_views::qualification_carrier(program, reference)
        } else {
            reference
        }
    };

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
            statement_index,
            cast,
            source_span,
            *element_type,
            let_referee,
            view_access,
            diagnostics,
            &context,
        );
        return;
    }
    if exact_scalar_representation_type(program, cast.target_type).is_none() {
        // RUNG C2/C3: a recursively fixed RECORD or literal-length ARRAY
        // target. The same normalized representation supplies size/alignment
        // and scalar-leaf facts; this is the top-level-array continuation of
        // the array fields records already admit.
        // Compare encoded physical bit sets, not the wider logical carriers.
        // Writes through a stored-width view still prove fit; a compatible
        // canonical view writes those same bytes without a new encoding.
        let target_representation =
            type_representation(program, representation_carrier(cast.target_type));
        if let Some(target_representation) = target_representation {
            if program.normalized_type_identity(let_referee)
                != program.normalized_type_identity(cast.target_type)
            {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "{context}: the let's declared type must restate the recast target \
                         `&{}{target_name}`",
                        view_access.borrow_modifier(),
                    ))
                    .with_source_span(source_span),
                );
                return;
            }
            let interior = byte_region_footprint(
                program,
                machine,
                state,
                statement_index,
                source,
                target_representation.size as i64,
            );
            if footprint_rejected(&interior, diagnostics, &context, source_span) {
                return;
            }
            if let ByteRegionFootprint::Contained = interior {
                if writable_view
                    && !byte_region_allows_arbitrary_writes(program, machine, state, source)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: a mutable byte-region recast must preserve the source element's representation facts in BOTH directions"
                    )).with_source_span(source_span));
                    return;
                }
                if !direct_record_view_type_is_fact_free(program, cast.target_type) {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "{context}: byte-region recast target `{target_name}` must be recursively \
                             fact-free; unchecked bytes cannot establish constrained fields, bool, \
                             or record invariants{}",
                            if writable_view {
                                "; mutable views require fact implication in BOTH directions"
                            } else {
                                ""
                            },
                        ))
                        .with_source_span(source_span),
                    );
                    return;
                }
                return;
            }
            if let Some(source_type) = source_type
                && let Some(source_representation) =
                    type_representation(program, representation_carrier(source_type))
            {
                let compatible = if writable_view {
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
                diagnostics.push(
                    Diagnostic::error(format!(
                        "{context}: {} aggregate aliases require identical layout geometry and {}; \
                         the source and target `{target_name}` are not representation-compatible",
                        if writable_view { "mutable" } else { "shared" },
                        if writable_view {
                            "leaf fact implication in BOTH directions"
                        } else {
                            "source leaf facts implying every target leaf fact"
                        },
                    ))
                    .with_source_span(source_span),
                );
                return;
            }
        }
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: recast target `{target_name}` is not a scalar primitive or an \
                 eligible fixed aggregate over a byte region or typed aggregate place; deeper shapes \
                 land with the byte-view rung"
            ))
            .with_source_span(source_span),
        );
        return;
    }
    let Some(target) = exact_scalar_representation_type(program, cast.target_type) else {
        return;
    };
    let let_primitive =
        crate::value_custody::places::unwrapped_type_reference(program, let_referee)
            .and_then(|unwrapped| exact_scalar_representation_type(program, unwrapped));
    if let_primitive != Some(target) {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: the let's declared type must restate the recast target `&{target_name}` \
                 (the stated shape is the single source of truth for reads through the view)"
            ))
            .with_source_span(source_span),
        );
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
    let footprint = target.scalar_byte_size().map_or(0, |size| size as i64);
    let interior =
        byte_region_footprint(program, machine, state, statement_index, source, footprint);
    if footprint_rejected(&interior, diagnostics, &context, source_span) {
        return;
    }
    if let ByteRegionFootprint::Contained = interior {
        if writable_view && !byte_region_allows_arbitrary_writes(program, machine, state, source) {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: a mutable byte-region recast must preserve the source element's representation facts in BOTH directions"
            )).with_source_span(source_span));
            return;
        }
        let target_facts = mutable_scalar_representation_facts(program, let_referee);
        let raw_facts = MutableScalarRepresentationFacts {
            atomic: false,
            domains: Vec::new(),
            values: ScalarRepresentationSet::ExactBitPatterns(full_scalar_bit_patterns(target)),
        };
        let compatible = target_facts.as_ref().is_some_and(|target_facts| {
            if writable_view {
                mutable_scalar_representation_facts_equivalent(program, &raw_facts, target_facts)
            } else {
                scalar_representation_facts_imply(program, &raw_facts, target_facts)
            }
        });
        if !compatible {
            diagnostics.push(
                Diagnostic::error(format!(
                    "{context}: a {} recast {}; a raw byte region cannot establish the target's \
                     representation facts",
                    if writable_view { "mutable" } else { "shared" },
                    if writable_view {
                        "must prove fact implication in BOTH directions"
                    } else {
                        "may weaken established facts but cannot strengthen them"
                    },
                ))
                .with_source_span(source_span),
            );
            return;
        }
        return;
    }
    let source_primitive = source_type
        .and_then(|type_reference| exact_scalar_representation_type(program, type_reference));
    let Some(source_primitive) = source_primitive else {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: a recast re-views a PLACE's bytes -- the source must be a borrowed \
                 scalar place (`&x as &{target_name}`); record sources and temporaries land \
                 with the byte-view rung"
            ))
            .with_source_span(source_span),
        );
        return;
    };
    let (Some(source_size), Some(target_size)) = (
        source_primitive.scalar_byte_size(),
        target.scalar_byte_size(),
    ) else {
        return;
    };
    if source_size != target_size {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: a recast re-views the SAME bytes, so the shapes must agree on \
                 size (§5b rule 1) -- source `{}` is {source_size} bytes, target \
                 `{target_name}` is {target_size} bytes",
                source_primitive.name()
            ))
            .with_source_span(source_span),
        );
        return;
    }

    let source_facts = source_type
        .and_then(|type_reference| {
            mutable_scalar_representation_facts(program, representation_carrier(type_reference))
        })
        .map(|mut facts| {
            // A field's `where` clauses narrow what the place stores just as a
            // type-attached range would; fold them in so equivalence does not
            // depend on which spelling declared the fact.
            if let Some(interval) = crate::proof_contracts::default_domains::where_fact_interval(
                program,
                machine,
                Some(state),
                source,
            ) {
                narrow_to_integer_interval(
                    &mut facts,
                    source_primitive,
                    interval.low,
                    interval.high,
                );
            }
            facts
        });
    let target_facts =
        mutable_scalar_representation_facts(program, representation_carrier(let_referee));
    let target_is_fact_free = target_facts.as_ref().is_some_and(|target_facts| {
        target_facts.domains.is_empty()
            && target_facts.values
                == ScalarRepresentationSet::ExactBitPatterns(full_scalar_bit_patterns(target))
    });
    let source_atomic = source_type
        .and_then(|reference| super::representation_types::scalar_is_atomic(program, reference));
    let target_atomic = super::representation_types::scalar_is_atomic(program, let_referee);
    if !writable_view
        && source_atomic
            .zip(target_atomic)
            .is_some_and(|(source, target)| source != target)
    {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: a shared recast must preserve atomic observation and mutation authority"
            ))
            .with_source_span(source_span),
        );
        return;
    }
    let shared_observation_compatible =
        source_atomic == Some(false) && target_atomic == Some(false);
    let compatible = if !writable_view && target_is_fact_free && shared_observation_compatible {
        // A shared view may always forget source facts. This remains safe even
        // when the source uses a fact family (such as a float interval) whose
        // precise representation set is not yet modeled.
        true
    } else {
        source_facts
            .as_ref()
            .zip(target_facts.as_ref())
            .is_some_and(|(source_facts, target_facts)| {
                if writable_view {
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
        diagnostics.push(
            Diagnostic::error(if writable_view {
                format!(
                    "{context}: a mutable recast must prove fact implication in BOTH directions; \
                     source and target constraints are not proven representation-equivalent"
                )
            } else {
                format!(
                    "{context}: a shared recast may weaken established facts but cannot strengthen \
                     them; source facts do not establish the target representation"
                )
            })
            .with_source_span(source_span),
        );
    }
}

#[allow(clippy::too_many_arguments)]
/// Reject with the shared unproven-offset diagnostic when `interior`
/// carries the `OffsetUnproven` region kind.
fn interior_offset_unproven_rejected(
    interior: &InteriorByteRegion,
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
    source_span: source::SourceSpan,
) -> bool {
    if let InteriorByteRegion::OffsetUnproven {
        offset_display,
        region_length,
    } = interior
    {
        push_offset_unproven(
            diagnostics,
            context,
            offset_display,
            *region_length,
            source_span,
        );
        return true;
    }
    false
}

fn footprint_rejected(
    footprint: &ByteRegionFootprint,
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
    source_span: source::SourceSpan,
) -> bool {
    if let ByteRegionFootprint::Rejected(reason) = footprint {
        diagnostics
            .push(Diagnostic::error(format!("{context}: {reason}")).with_source_span(source_span));
        return true;
    }
    false
}

fn judge_slice_recast(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    cast: &TableCastExpression,
    source_span: source::SourceSpan,
    element_type: TypeReferenceHandle,
    let_referee: TypeReferenceHandle,
    view_access: language_core::ReferenceAccess,
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
) {
    let writable_view = view_access.is_exclusive();
    let target_label = program.display_type_reference(cast.target_type);
    if program.normalized_type_identity(let_referee)
        != program.normalized_type_identity(cast.target_type)
    {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: the let's declared type must restate the recast target \
                 `&{}{target_label}`",
                view_access.borrow_modifier(),
            ))
            .with_source_span(source_span),
        );
        return;
    }

    let source = strip_mutable(program, cast.value);
    let Some(element_representation) = type_representation(program, element_type) else {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: slice target `{target_label}` needs a fixed-layout element type"
            ))
            .with_source_span(source_span),
        );
        return;
    };

    // An interior slice starts at one byte of a proven `[u8; N]` region and
    // consumes every remaining byte. This is the dynamically-sized companion
    // to the fixed aggregate/scalar interior rungs: raw bytes may establish
    // only recursively fact-free, exactly tiled element representations.
    let interior = interior_byte_region_source(program, machine, state, statement_index, source, 0);
    if interior_offset_unproven_rejected(&interior, diagnostics, context, source_span) {
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
            diagnostics.push(
                Diagnostic::error(format!(
                    "{context}: interior slice recast `{target_label}` requires a recursively \
                     fact-free element whose scalar leaves exactly tile its byte stride; raw \
                     storage cannot establish element facts or implicit padding"
                ))
                .with_source_span(source_span),
            );
            return;
        }
        if offset > region_length {
            diagnostics.push(
                Diagnostic::error(format!(
                    "{context}: interior slice recast starts at byte {offset}, past the \
                     {region_length}-byte source region"
                ))
                .with_source_span(source_span),
            );
            return;
        }
        let exact_offset = exact_interior_byte_region_offset(program, machine, state, source);
        if let Some(exact_offset) = exact_offset {
            let remaining = region_length - exact_offset;
            if remaining < 0
                || element_representation.size == 0
                || !(remaining as usize).is_multiple_of(element_representation.size)
            {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "{context}: interior slice target `{target_label}` does not exactly tile \
                         the {remaining} bytes remaining at offset {exact_offset} with {}-byte \
                         elements",
                        element_representation.size,
                    ))
                    .with_source_span(source_span),
                );
            }
        } else if element_representation.size != 1 {
            // Congruent runtime offsets admit without an exact value: when
            // the region length and every reachable offset residue are both
            // `0 (mod size)`, the remaining bytes always tile whole
            // elements. The bound already proved the composed offset small
            // enough to stay inside the region, so the residue arithmetic
            // cannot have wrapped.
            let congruent = (region_length as usize).is_multiple_of(element_representation.size)
                && matches!(
                    program.expression_table.expression(source),
                    ExpressionNode::Indexed(indexed)
                        if super::offset_bounds::offset_expression_is_multiple_of(
                            program,
                            machine,
                            state,
                            indexed.index,
                            element_representation.size,
                        )
                );
            if !congruent {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "{context}: cannot prove exact tiling for interior slice `{target_label}`: \
                         the runtime byte offset may leave a remainder for {}-byte elements; use a \
                         statically exact offset or validate the dynamic region before establishing \
                         the typed slice",
                        element_representation.size,
                    ))
                    .with_source_span(source_span),
                );
            }
        }
        return;
    }

    let Some(source_type) = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        source,
    ) else {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: a slice recast re-views a fixed-layout PLACE's complete bytes"
            ))
            .with_source_span(source_span),
        );
        return;
    };
    let Some(source_representation) = type_representation(program, source_type) else {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: slice recast source `{}` has no fixed representation",
                program.display_type_reference(source_type)
            ))
            .with_source_span(source_span),
        );
        return;
    };
    if element_representation.size == 0
        || source_representation.size % element_representation.size != 0
    {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: slice target `{target_label}` does not exactly tile the source's {} \
                 bytes with {}-byte elements",
                source_representation.size, element_representation.size,
            ))
            .with_source_span(source_span),
        );
        return;
    }

    let element_count = source_representation.size / element_representation.size;
    let Some(target_representation) = repeat_representation(
        &element_representation,
        element_count,
        element_representation.size,
    ) else {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: slice target `{target_label}` representation overflows"
            ))
            .with_source_span(source_span),
        );
        return;
    };

    let source_tiled = representation_is_exactly_tiled(&source_representation);
    let target_tiled = representation_is_exactly_tiled(&target_representation);
    let source_fact_free = record_view_type_is_fact_free(program, source_type, &mut HashSet::new());
    let target_fact_free =
        record_view_type_is_fact_free(program, element_type, &mut HashSet::new());
    // Plain target bits do not erase a source observation protocol. Atomic
    // storage still uses the leaf-wise operation/representation judgment,
    // including for a shared slice that performs no ordinary writes.
    let compatible = if source_tiled
        && target_tiled
        && target_fact_free
        && !source_representation.permits_shared_mutation()
    {
        !writable_view || source_fact_free
    } else if writable_view {
        mutable_record_representations_equivalent(
            program,
            &source_representation,
            &target_representation,
        )
    } else {
        record_representation_implies(program, &source_representation, &target_representation)
    };
    if !compatible {
        diagnostics.push(
            Diagnostic::error(format!(
                "{context}: slice recast `{target_label}` does not preserve exact byte tiling and {}; \
                 raw storage cannot establish element facts",
                if writable_view {
                    "fact implication in BOTH directions"
                } else {
                    "source-to-target fact implication"
                },
            ))
            .with_source_span(source_span),
        );
    }
}

/// The companion hole-closer: a reference-typed let over a BARE borrow of a
/// scalar place whose type disagrees with the stated referee. Without this,
/// `let v: &f32 = &self.x;` (x: i64) is an accidental, unjudged recast --
/// native bit-puns while the interpreter delivers the semantic value.
pub(crate) fn report_unspelled_reference_pun(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    initializer: ExpressionHandle,
    let_referee: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = strip_mutable(program, initializer);
    let Some(source_primitive) =
        crate::value_custody::places::declared_place_type(program, machine, Some(state), source)
            .and_then(|type_reference| exact_scalar_representation_type(program, type_reference))
    else {
        return;
    };
    let Some(referee_primitive) =
        crate::value_custody::places::unwrapped_type_reference(program, let_referee)
            .and_then(|unwrapped| exact_scalar_representation_type(program, unwrapped))
    else {
        return;
    };
    if source_primitive != referee_primitive {
        diagnostics.push(
            Diagnostic::error(format!(
                "machine `{}` state `{}`: reference initializer type `{}` must match the \
                 stated `&{}`; re-viewing a place's bytes under another shape is spelled \
                 `&x as &{}` (§5b recast)",
                machine.name.as_str(),
                state.name.as_str(),
                source_primitive.name(),
                referee_primitive.name(),
                referee_primitive.name(),
            ))
            .with_source_span(program.expression_table.source_span(initializer)),
        );
    }
}

pub(crate) fn strip_mutable(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => strip_mutable(program, inner.target),
        _ => expression,
    }
}
