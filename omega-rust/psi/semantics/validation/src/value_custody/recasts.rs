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
//!
//! This file validates the recasts of one machine.
//! `literal_indexed_footprints.rs` validates literal-indexed recast
//! footprints, `representation_types.rs` names exact primitive and scalar
//! representation types, `record_eligibility.rs` decides fact-free record
//! eligibility and `recast_judgments.rs` judges scalar and slice recasts.

mod literal_indexed_footprints;
#[cfg(test)]
mod offset_bound_tests;
mod offset_bounds;
mod qualification;
mod raw_byte_region;
mod recast_judgments;
mod record_eligibility;
mod record_representation;
mod representation_types;
mod scalar_representation;

pub use literal_indexed_footprints::{
    ValidatedLiteralIndexedRecastFootprint, validate_literal_indexed_recast_footprint,
};
pub(crate) use representation_types::{exact_primitive_type, exact_scalar_representation_type};

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceNode;

use qualification::{judge_qualification_cast, judge_statement_qualification_casts};

use crate::value_custody::recasts::recast_judgments::{
    judge_scalar_recast, report_unspelled_reference_pun, strip_mutable,
};

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
        crate::declarations::traits::collect_dynamic_conformance_selections(program)
            .unwrap_or_default();

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
                                        && crate::declarations::traits::dynamic_trait_symbol(
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
                        if crate::declarations::traits::dynamic_trait_symbol(
                            program,
                            cast.target_type,
                        )
                        .is_none()
                        {
                            judge_scalar_recast(
                                program,
                                machine,
                                state,
                                cast,
                                initializer,
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
            diagnostics.push(
                Diagnostic::error(
                    "a recast binds to a reference-typed let (`let v: &T = &x as &T;`) in this \
                     rung; inline re-views land with the byte-view rung"
                        .to_string(),
                )
                .with_source_span(program.expression_table.source_span(handle)),
            );
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
