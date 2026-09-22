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
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
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
    // Inline positions judged under the same relation: the cast's own
    // stated type is the source of truth (there is no let to restate it).
    let mut judged_inline: Vec<ExpressionHandle> = Vec::new();
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
                if let StatementNode::LocalData(local) = statement
                    && local.type_reference.is_valid()
                    && local.initial_value.is_valid()
                    && let TypeReferenceNode::Reference {
                        referee, access, ..
                    } = program
                        .type_reference_table
                        .type_reference(local.type_reference)
                {
                    // The `&mut x as &mut T` spelling parses as
                    // Mutable(Cast(..)): the unary `&mut` wraps the postfix
                    // chain. Look through it so the blessed root is the CAST
                    // node the sweep checks.
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

                // The recast contract judges representation compatibility,
                // not source position (§5b): a recast nested in an ordinary
                // statement expression -- a guard operand, a call argument,
                // a binary operand -- is the SAME judgment, with the cast's
                // own stated type as the referee restatement and the `as &`
                // / `as &mut` spelling as the access polarity. Judge each
                // statement-reachable recast in place so a stray cast cannot
                // route around the relation. Proof-only assembly facts are
                // excluded: they do not produce runtime borrows.
                let mut roots: Vec<ExpressionHandle> = Vec::new();
                statement_expression_roots(program, statement, &mut roots);
                let mut pending = roots;
                while let Some(handle) = pending.pop() {
                    let node = program.expression_table.expression(handle);
                    if let ExpressionNode::Cast(cast) = node
                        && cast.form.is_recast()
                        && !blessed.contains(&handle)
                        && !judged_inline.contains(&handle)
                    {
                        judged_inline.push(handle);
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
                                handle,
                                cast.target_type,
                                cast.form
                                    == language_core::cast_form::CastForm::RecastMutable,
                                diagnostics,
                            );
                        }
                    }
                    crate::value_custody::literals::expression_children::children(
                        program,
                        node,
                        |child| pending.push(child),
                    );
                }
            }
        }
    }

    // Position sweep: a recast that no statement walk reached refuses. The
    // parser builds recast nodes only from the `as &` spelling; the per-
    // statement walk above covers ordinary execution positions, so a stray
    // here sits in proof-only ground (assembly facts, contract expressions)
    // or is unreachable.
    for (handle, node) in program.expression_table.expression_entries() {
        if let ExpressionNode::Cast(cast) = node
            && cast.form.is_recast()
            && !blessed.contains(&handle)
            && !judged_inline.contains(&handle)
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

/// The expression roots of one executable statement -- every position an
/// authored expression can occupy in ordinary sequencing (assignment halves,
/// bare expressions, call and transition-target arguments, guards, let
/// initializers, binding operands). `AssemblyFact` is proof-only ground and
/// stays outside the walk: a recast there produces no runtime borrow, so the
/// positional sweep below keeps it fenced.
fn statement_expression_roots(
    program: &TypedTrees,
    statement: &StatementNode,
    roots: &mut Vec<ExpressionHandle>,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            roots.push(binding.receiver);
            roots.push(binding.implementation_operand);
        }
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            roots.push(assignment.target);
            roots.push(assignment.value);
        }
        StatementNode::Call(call) => {
            roots.extend(
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        StatementNode::Expression(expression) => roots.push(*expression),
        StatementNode::LocalData(local) => {
            if local.initial_value.is_valid() {
                roots.push(local.initial_value);
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target in [transition.target, transition.continuation] {
                if !program.statement_table.transition_target_is_valid(target) {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        roots.extend(
                            program
                                .statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .copied(),
                        );
                    }
                    TransitionTargetNode::Value(value) => roots.push(*value),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}
