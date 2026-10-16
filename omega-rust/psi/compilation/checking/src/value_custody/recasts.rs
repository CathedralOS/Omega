//! §5b recast judgment, rung A (programmable-layouts brief): `&x as &T`
//! re-views a place's bytes under a second stated shape. Legality is a
//! STATIC judgment -- a bad relation is a compile error, never unsafety.
//!
//! The scalar rung serves the core end-to-end and fences the rest loudly:
//!
//! - **Served:** a recast between fixed-width scalar primitives of EQUAL byte
//!   size (`&i32 as &f32`, `&mut u32 as &mut i32`), or a scalar view into a
//!   proven in-bounds fixed array or borrowed byte-slice region, bound as the direct
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
//!   shared aliases may weaken facts. It also serves
//!   stored-width integers by comparing their encoded bit sets. A compatible
//!   canonical record exposes the original foreign bytes, not a copy of the
//!   wider semantic value. Semantic stores through encoded views still prove fit.
//!   The same repeated-leaf judgment serves unsized slices of aggregate
//!   elements over a complete typed fixed array;
//!   element stride includes layout padding rather than repacking the leaves.
//!   Lowering is address identity:
//!   native reads/writes the place through the stated type; the interpreter
//!   bit-reinterprets both sides of the alias or assembles/writes the complete
//!   little-endian byte-region footprint. The judgment is position-free:
//!   statement-reachable casts in guard operands, call arguments, and nested
//!   expressions receive the same relation in place.
//! - **Fenced (deeper byte-view rung, L4/L5):** remaining dynamically-sized
//!   shapes beyond complete-source and proven interior unsized slices
//!   (byte-granular tiling over plan-laid layouts), and recasts in proof-only
//!   ground (`requires`/`ensures` facts and contract expressions produce no
//!   runtime borrow). A runtime interior byte offset cannot establish
//!   multi-byte element tiling until its congruence is proved; an exact
//!   offset can.
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
mod offset_bounds;
mod raw_byte_region;
mod recast_judgments;
mod record_eligibility;
mod record_representation;
mod slice_views;
pub(crate) use record_representation::recast_representations_equivalent;
pub use record_representation::{
    RecastLeafSegment, RecastRecordSourceLeaf, RecastRuntimeIndexGeometry, RecastScalarGeometry,
    RecastScalarLayout, RecastScalarProjection, recast_record_source_leaf, recast_scalar_layout,
    recast_scalar_path_geometry, recast_scalar_path_projection, recast_scalar_projection,
    recast_scalar_source_leaf,
};
mod representation_types;
mod scalar_representation;
mod storage_interpretation;
pub(crate) use scalar_representation::declaration_scalar_view_is_total;
pub use storage_interpretation::reference_view_storage;

pub use literal_indexed_footprints::{
    ValidatedLiteralIndexedRecastFootprint, validate_literal_indexed_recast_footprint,
    validate_runtime_indexed_recast_source,
};
pub(crate) use representation_types::{exact_primitive_type, exact_scalar_representation_type};

use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Authored formation coordinates; zero view_type means an ordinary borrow.
/// The same reference recast may have an explicit outer borrow or borrow an
/// already-reference-typed source. These forms do not grant different custody.
pub struct ReferenceFormationSource {
    pub source: ExpressionHandle,
    pub access: language_core::ReferenceAccess,
    pub view_type: TypeReferenceHandle,
    pub interpretation: ReferenceFormationInterpretation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceFormationInterpretation {
    Declared,
    RepresentationView,
    Placed,
}

pub fn reference_formation_source(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ReferenceFormationSource> {
    if let Some(call) = program.expression_table.call_node(expression) {
        let selected = program.machine_by_symbol(call.target_symbol).or_else(|| {
            crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)
                .map(|(machine, _)| machine)
        })?;
        let request = program.placed_borrow_factory(selected.symbol)?;
        let result = crate::machine_calls::calls::call_result_type(program, expression)?;
        let shell = crate::reference_shell(program, result)?;
        if shell.access != request.access {
            return None;
        }
        let referee = program
            .type_reference_table
            .peeled_type_reference(shell.referee);
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(referee)
        else {
            return None;
        };
        let mut plans = program
            .placed_view_plans
            .iter()
            .filter(|plan| plan.data_symbol == *symbol);
        let (Some(_), None) = (plans.next(), plans.next()) else {
            return None;
        };
        let [argument] = program.expression_table.expression_handles(call.arguments) else {
            return None;
        };
        // The ordinary source capture evaluates selectors once and retains
        // any stored reference crossings. This query adds no loan or access.
        let source = match program.expression_table.expression(*argument) {
            ExpressionNode::Borrow(borrow) => {
                if !matches!(
                    (borrow.access, request.access),
                    (language_core::ReferenceAccess::Mutable, _)
                        | (
                            language_core::ReferenceAccess::Shared,
                            language_core::ReferenceAccess::Shared
                        )
                ) {
                    return None;
                }
                borrow.target
            }
            _ => *argument,
        };
        return Some(ReferenceFormationSource {
            source,
            access: request.access,
            view_type: shell.referee,
            interpretation: ReferenceFormationInterpretation::Placed,
        });
    }
    let (operand, borrowed_access) = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => (borrow.target, Some(borrow.access)),
        _ => (expression, None),
    };
    if let Some(cast) = program.expression_table.cast_node(operand) {
        let access = match cast.form {
            language_core::cast_form::CastForm::RecastShared => {
                language_core::ReferenceAccess::Shared
            }
            language_core::cast_form::CastForm::RecastMutable => {
                language_core::ReferenceAccess::Mutable
            }
            language_core::cast_form::CastForm::Value => return None,
        };
        if borrowed_access.is_some_and(|borrowed| borrowed != access) {
            return None;
        }
        return Some(ReferenceFormationSource {
            source: strip_mutable(program, cast.value),
            access,
            view_type: cast.target_type,
            interpretation: ReferenceFormationInterpretation::RepresentationView,
        });
    }
    Some(ReferenceFormationSource {
        source: operand,
        access: borrowed_access?,
        view_type: TypeReferenceHandle::default(),
        interpretation: ReferenceFormationInterpretation::Declared,
    })
}

use crate::value_custody::recasts::recast_judgments::{
    judge_scalar_recast, report_unspelled_reference_pun, strip_mutable,
};

pub(crate) fn validate_recasts(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The blessed positions: direct initializers of reference-typed lets
    // (mirrors the D14 literal gate's shape -- collect the legal roots,
    // then sweep the whole expression table for strays).
    let mut blessed: Vec<ExpressionHandle> = Vec::new();
    // Inline positions judged under the same relation: the cast's own
    // stated type is the source of truth (there is no let to restate it).
    let mut judged_inline: Vec<ExpressionHandle> = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                if let Some(local) = statement.as_local_data()
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
                    // node the sweep checks. The outermost Borrow still
                    // spells the authored source access: `&x as &mut T`
                    // would mint an exclusive view from a shared borrow.
                    let spelled_borrow_exclusive =
                        match program.expression_table.expression(local.initial_value) {
                            ExpressionNode::Borrow(borrow) => Some(borrow.access.is_exclusive()),
                            _ => None,
                        };
                    let initializer = strip_mutable(program, local.initial_value);
                    match program.expression_table.expression(initializer) {
                        ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                            blessed.push(initializer);
                            judge_scalar_recast(
                                program,
                                machine,
                                state,
                                statement_index,
                                cast,
                                initializer,
                                *referee,
                                access.is_exclusive(),
                                spelled_borrow_exclusive,
                                diagnostics,
                            );
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
                // Each entry carries the access of its nearest enclosing
                // Borrow: the authored `&`/`&mut` source spelling of a recast
                // operand must agree with the view polarity the `as` form
                // claims (`&x as &mut T` is escalation, not a shared view).
                let mut pending: Vec<(ExpressionHandle, Option<bool>)> =
                    roots.iter().map(|root| (*root, None)).collect();
                while let Some((handle, enclosing_borrow)) = pending.pop() {
                    let node = program.expression_table.expression(handle);
                    let enclosing_borrow = match node {
                        ExpressionNode::Borrow(borrow) => Some(borrow.access.is_exclusive()),
                        _ => enclosing_borrow,
                    };
                    if let ExpressionNode::Cast(cast) = node
                        && cast.form.is_recast()
                        && !blessed.contains(&handle)
                        && !judged_inline.contains(&handle)
                    {
                        judged_inline.push(handle);
                        judge_scalar_recast(
                            program,
                            machine,
                            state,
                            statement_index,
                            cast,
                            handle,
                            cast.target_type,
                            cast.form == language_core::cast_form::CastForm::RecastMutable,
                            enclosing_borrow,
                            diagnostics,
                        );
                    }
                    crate::expression_traversal::children(program, node, |child| {
                        pending.push((child, enclosing_borrow))
                    });
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
                    "a recast re-views storage at an executable position (`let v: &T = &x as &T`, \
                     a guard operand, a call argument, a nested expression); proof-only fact \
                     positions (`requires`/`ensures`) produce no runtime borrow and stay fenced"
                        .to_string(),
                )
                .with_source_span(program.expression_table.source_span(handle)),
            );
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
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
    roots: &mut Vec<ExpressionHandle>,
) {
    use typed_trees::statement::StatementRootRole as Role;
    roots.extend(
        program
            .tables
            .expression_roots(statement)
            .into_iter()
            .filter(|(role, root)| match role {
                Role::AssemblyFact => false,
                Role::LocalInitializer => root.is_valid(),
                _ => true,
            })
            .map(|(_, root)| root),
    );
}

pub use raw_byte_region::{SliceRecastByteRegion, slice_recast_byte_region};
pub use slice_views::{SliceRecastSource, slice_recast_source};
