//! Successor-edge construction for one composed state.
//!
//! `successor` (ordinary edges) and `successor_bindings` (the shared operand
//! planner) build each `CheckedStructuralControlSuccessorPlan`: the exact
//! parameter/local result transfers, projected loans and case payloads the
//! edge carries, under the same argument custody an in-sequence call takes.
//! The terminator and closed-sum modules call these through `super::`.
use super::super::calls;
use super::Signature;
use super::fused::{FusedStates, fused_target_index};
use super::result_custody::{operand_owned_construction, retained_structural_result};
use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralScalarArgumentPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, ExpressionNode,
    Multiplicity, StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::control;
use crate::execution::terminal_unit::types::ShapeCollector;

/// The terminator family an edge is built for. The trace names the guard
/// that declined the edge beneath that family's phase; the edge's statement
/// ordinal separates the true and false successors of one conditional.
#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) enum SuccessorEdge {
    Jump,
    Conditional,
    GuardedJump,
    ClosedCase,
}

#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) enum SuccessorGuard {
    TransitionForm,
    TargetState,
    ArgumentCount,
    StructuralArgument,
    SubsliceTransfer,
    LiteralTransfer,
    ResultTransfer,
    CasePayloadTransfer,
    EstablishedArgument,
    ProjectedValue,
    LoanTransfer,
    ParameterTransfer,
    ScalarArguments,
    ErasedArguments,
    ErasedProofArguments,
    EdgeCleanup,
}

impl SuccessorEdge {
    /// The trace phase naming `guard` on this successor edge. Every phase is
    /// `state graph: terminator: <edge>: <guard>`, spelled at compile time.
    pub(in crate::execution::terminal_unit) fn phase(self, guard: SuccessorGuard) -> &'static str {
        macro_rules! guard_phase {
            ($edge:literal, $guard:literal) => {
                concat!("state graph: terminator: ", $edge, ": ", $guard)
            };
            ($edge:literal) => {
                match guard {
                    SuccessorGuard::TransitionForm => guard_phase!($edge, "transition form"),
                    SuccessorGuard::TargetState => guard_phase!($edge, "target state"),
                    SuccessorGuard::ArgumentCount => guard_phase!($edge, "argument count"),
                    SuccessorGuard::StructuralArgument => {
                        guard_phase!($edge, "structural argument")
                    }
                    SuccessorGuard::SubsliceTransfer => {
                        guard_phase!($edge, "byte-subslice transfer")
                    }
                    SuccessorGuard::LiteralTransfer => guard_phase!($edge, "literal transfer"),
                    SuccessorGuard::ResultTransfer => guard_phase!($edge, "result-local transfer"),
                    SuccessorGuard::CasePayloadTransfer => {
                        guard_phase!($edge, "case-payload transfer")
                    }
                    SuccessorGuard::EstablishedArgument => {
                        guard_phase!($edge, "established argument")
                    }
                    SuccessorGuard::ProjectedValue => guard_phase!($edge, "projected value"),
                    SuccessorGuard::LoanTransfer => guard_phase!($edge, "loan transfer"),
                    SuccessorGuard::ParameterTransfer => guard_phase!($edge, "parameter transfer"),
                    SuccessorGuard::ScalarArguments => guard_phase!($edge, "scalar arguments"),
                    SuccessorGuard::ErasedArguments => guard_phase!($edge, "erased arguments"),
                    SuccessorGuard::ErasedProofArguments => {
                        guard_phase!($edge, "erased proof arguments")
                    }
                    SuccessorGuard::EdgeCleanup => guard_phase!($edge, "edge cleanup"),
                }
            };
        }
        match self {
            Self::Jump => guard_phase!("jump successor"),
            Self::Conditional => guard_phase!("conditional successors"),
            Self::GuardedJump => guard_phase!("guarded jump successors"),
            Self::ClosedCase => guard_phase!("closed-sum case successor"),
        }
    }
}

/// The terminal verifier refuses a continuation argument bound to a
/// place whose declared type carries a reference, so a loan can only name
/// storage whose declared shape has no `Reference` anywhere inside it.
fn declared_contains_reference(
    shapes: &crate::execution::terminal_unit::types::ShapeCollector<'_>,
    identity: &str,
) -> bool {
    let mut pending = vec![identity.to_owned()];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }
        let Some(plan) = shapes.types.get(&current) else {
            return true;
        };
        let mut fields = |items: &[checked_trees::CheckedUnitStructuralFieldPlan]| {
            for field in items {
                if let checked_trees::CheckedUnitStructuralFieldType::Structural { type_identity } =
                    &field.field_type
                {
                    pending.push(type_identity.clone());
                }
            }
        };
        match &plan.shape {
            checked_trees::CheckedUnitStructuralTypeShape::Reference { .. } => return true,
            checked_trees::CheckedUnitStructuralTypeShape::FusedServiceBinding
            | checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
            | checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(_) => {}
            checked_trees::CheckedUnitStructuralTypeShape::Record { fields: items } => {
                fields(items);
            }
            checked_trees::CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                ..
            }
            | checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView {
                element_type_identity,
            } => pending.push(element_type_identity.clone()),
            checked_trees::CheckedUnitStructuralTypeShape::Sum { cases } => {
                for case in cases {
                    fields(&case.fields);
                }
            }
            checked_trees::CheckedUnitStructuralTypeShape::Mixed {
                fields: items,
                cases,
            } => {
                fields(items);
                for case in cases {
                    fields(&case.fields);
                }
            }
        }
    }
    false
}

/// The callee catalog, source-state entry claims and source-state local
/// results an edge plans its own operand calls against. A composite
/// argument's call runs only on the selected edge, under the same argument
/// custody an in-sequence call takes: an operand naming one of the state's
/// `let` locals (or an anonymous result it established) resolves to that
/// result binding exactly as a later statement's call would.
#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) struct EdgeCalls<'a> {
    pub(super) scalar_callees: ScalarCalleePlans<'a>,
    pub(super) entry_claims: &'a [checked_trees::CheckedUnitEntryClaimPlan],
    pub(super) local_results: &'a [(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
}

pub(in crate::execution::terminal_unit) fn successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    fused: &FusedStates<'_>,
    source_index: usize,
    signatures: &[Signature],
    operations: &[CheckedUnitEffectOperationPlan],
    transition: &typed_trees::statement::TableTransition,
    ordinal: u32,
    edge: SuccessorEdge,
    edge_calls: EdgeCalls<'_>,
    shapes: &mut ShapeCollector<'_>,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let successor = successor_bindings(
        program,
        facts,
        fused,
        source_index,
        signatures,
        operations,
        transition,
        ordinal,
        &[],
        edge,
        edge_calls,
        shapes,
        trace,
    )?;
    trace.phase(edge.phase(SuccessorGuard::EdgeCleanup));
    trace.statement(Some(ordinal));
    let &(machine, source) = fused.get(source_index)?;
    let cleanups = &facts.flow.terminal_structural_control_cleanups;
    // Exact normal-exit cleanup is retained after this graph's value schedule
    // is complete. The legacy whole-parameter roster cannot admit or reject
    // projected transfers: it cannot express the remaining subtree. Keep its
    // optional rows for old case-dispatch correspondence only; execution uses
    // the ordered normal-exit actions, including residuals and nominal hooks.
    let mut successor = successor;
    if let Some(cleanup) = cleanups.for_edge(machine.symbol, source.symbol, ordinal) {
        successor.trivial_affine_discard_parameter_positions =
            cleanup.trivial_affine_discard_parameter_positions.clone();
    }
    Some(successor)
}

// Operand identity is shared by ordinary and closed-case edges. The caller
// separately admits either whole-parameter cleanup or exact result-local
// cleanup; constructing bindings establishes neither cleanup contract.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn successor_bindings(
    program: &TypedTrees,
    facts: &CheckFacts,
    fused: &FusedStates<'_>,
    source_index: usize,
    signatures: &[Signature],
    operations: &[CheckedUnitEffectOperationPlan],
    transition: &typed_trees::statement::TableTransition,
    ordinal: u32,
    payload_parameters: &[u32],
    edge: SuccessorEdge,
    edge_calls: EdgeCalls<'_>,
    shapes: &mut ShapeCollector<'_>,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let mark = |guard| {
        trace.phase(edge.phase(guard));
        trace.statement(Some(ordinal));
    };
    mark(SuccessorGuard::TransitionForm);
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    mark(SuccessorGuard::TargetState);
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let target_index = fused_target_index(program, fused, path.symbol)?;
    let &(machine, source) = fused.get(source_index)?;
    let &(_, target) = fused.get(target_index)?;
    let arguments = program.statement_table.expression_handles(*arguments);
    let target_parameters = program.state_parameters(target);
    mark(SuccessorGuard::ArgumentCount);
    if arguments.len() != target_parameters.len() {
        return None;
    }
    let (source_structural, source_scalar) = &signatures[source_index];
    let (target_structural, target_scalar) = &signatures[target_index];
    // A guard that is an exact case test selects one sum case for this edge,
    // so the destructure-bound payload subtree `subject.Case::field` carries
    // the edge's proven membership into the target's custody. Record the
    // tested subject's canonical place and the selected case once; the
    // transfer walk reuses them for every structural argument.
    let case_test = match transition.guard {
        TransitionGuardNode::When(guard) => crate::proof::exact_outcome_case_test(program, guard)
            .and_then(|(subject, case)| {
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    source.symbol,
                    ordinal as usize,
                    subject,
                )
                .map(|place| (place, case))
            }),
        _ => None,
    };
    let argument_at = |position: u32| arguments.get(position as usize).copied();
    let source_position = |position: u32| {
        let mut argument = argument_at(position)?;
        // `value as T` on a structural type ascribes the same value, so the
        // transferred place is still the source parameter root. Scalar
        // (primitive-target) casts stay opaque: they compute a new value.
        while let ExpressionNode::Cast(cast) = program.expression_table.expression(argument)
            && program.primitive_type_reference(cast.target_type).is_none()
        {
            argument = cast.value;
        }
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            source.symbol,
            ordinal as usize,
            argument,
        )?;
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        if !place.segments.is_empty() {
            return None;
        }
        program
            .state_parameters(source)
            .iter()
            .position(|parameter| parameter.symbol == symbol)
    };
    let transfers = target_structural
        .iter()
        .enumerate()
        .map(|(target_index, target)| {
            mark(SuccessorGuard::StructuralArgument);
            let expression = argument_at(target.position)?;
            // Forward a whole parameter before planning any new loan. This
            // preserves its existing custody/entry-place transport; a projected
            // argument instead names a subtree and establishes a loan below it.
            // Resolve the source declaration, never the target's ordinal.
            if let Some((source_index, source_parameter)) = source_position(target.position)
                .and_then(|position| source_structural.iter().enumerate()
                    .find(|(_, parameter)| parameter.position as usize == position))
                && source_parameter.type_identity == target.type_identity
                && (source_parameter.access == target.access
                    || (source_parameter.access == CheckedStructuralAccess::MutableBorrow
                        && target.access == CheckedStructuralAccess::SharedBorrow))
            {
                mark(SuccessorGuard::ParameterTransfer);
                return Some(CheckedStructuralControlTransferPlan {
                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                        index: u32::try_from(source_index).ok()?,
                    },
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            if let Some(range_expression) = validation::subslice_expression(program, expression)
                && let ExpressionNode::Indexed(indexed) = program.expression_table.expression(range_expression)
                && matches!(program.expression_table.expression(indexed.index), ExpressionNode::Range(_))
            {
                mark(SuccessorGuard::SubsliceTransfer);
                let target_parameter = target_parameters.get(target.position as usize)?;
                let subslice = calls::view_subslice::admit(
                    program, facts, machine, source, source_structural,
                    target_parameter.type_reference, expression, ordinal as usize,
                    checked_trees::CheckedSubsliceSite::TransitionArgument {
                        argument_ordinal: target.position,
                    },
                )?;
                if subslice.range.type_identity != target.type_identity {
                    return None;
                }
                return Some(CheckedStructuralControlTransferPlan {
                    source: subslice.transfer(),
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            // An authored string literal supplied directly to a whole
            // unrestricted shared byte view (`relay("yes")` into
            // `text: &[u8] in Utf8`) has no custody root. The edge establishes
            // its octets into immutable literal storage, exactly as a
            // byte-literal call argument does, and the target's declared
            // domains are the establishment's membership evidence.
            if matches!(
                program.expression_table.expression(expression),
                ExpressionNode::String(_)
            ) {
                mark(SuccessorGuard::LiteralTransfer);
                let target_parameter = target_parameters.get(target.position as usize)?;
                let literal = calls::byte_sequence_literal_argument(
                    program,
                    target_parameter.type_reference,
                    expression,
                )?;
                let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } =
                    literal.source
                else {
                    return None;
                };
                if target.access != CheckedStructuralAccess::SharedBorrow
                    || target.multiplicity != Multiplicity::Unrestricted
                    || literal.access != CheckedStructuralAccess::SharedBorrow
                    || literal.type_identity != target.type_identity
                {
                    return None;
                }
                let source =
                    checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral {
                        bytes,
                    };
                return Some(CheckedStructuralControlTransferPlan {
                    source,
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            // A whole view local forwards the view its `let` published: the
            // target re-borrows that same place exactly as a forwarded view
            // parameter does, so the edge names the local's result binding
            // just as an owned local's move does. The binding, not the lent
            // root, stays the loan subject: substituting `room.exits` for
            // `exits` would mint a second loan instead of continuing the one
            // the `let` captured. A shared target takes a shared view local;
            // a `&mut [T]` target takes only a local that itself holds the
            // exclusive view loan (checked against the result below).
            let shared_view = target.access == CheckedStructuralAccess::SharedBorrow
                && target_parameters.get(target.position as usize).is_some_and(|parameter| {
                    calls::view_subslice::view_kind(program, parameter.type_reference).is_some()
                });
            let mutable_view = target.access == CheckedStructuralAccess::MutableBorrow
                && target_parameters.get(target.position as usize).is_some_and(|parameter| {
                    validation::slice_view_shell(program, parameter.type_reference, &[])
                        .is_some_and(|view| view.access == language_core::ReferenceAccess::Mutable)
                });
            // Fresh inline values have no retained source place. Only local
            // result forwarding needs one; its absence leaves construction
            // to the ordinary selected-edge establishment below.
            if (target.access == CheckedStructuralAccess::Owned || shared_view || mutable_view)
                && let Some(place) = crate::flow::canonical_place_from_expression_in_state(program, source.symbol, ordinal as usize, expression)
                // Forward an existing descriptor here. Borrowing an owned
                // local establishes a loan, even when its referent has the
                // same representation as the target. The ordinary loan path
                // below owns access, backing and array-to-view presentation;
                // an entry-only mutable-result shortcut loses that relation.
                && (target.access == CheckedStructuralAccess::Owned
                    || matches!(place.root, facts::PlaceRoot::Symbol(symbol)
                        if program.state_local_declaration(source, symbol)
                            .is_some_and(|(_, local)| validation::slice_view_shell(
                                program, local.type_reference, &[],
                            ).is_some())))
            {
                mark(SuccessorGuard::ResultTransfer);
                if place.segments.is_empty() {
                    let mut matches = operations.iter()
                        .filter(|operation| !operand_owned_construction(operation))
                        .filter_map(retained_structural_result)
                        .filter(|result| result.statement_index < ordinal && matches!(program.statement_table.statements(source.statement_nodes).get(result.statement_index as usize), Some(StatementNode::LocalData(local)) if place.root == facts::PlaceRoot::Symbol(local.symbol)));
                    if let Some(result) = matches.next() {
                        if matches.next().is_some() { return None; }
                        if result.type_identity != target.type_identity
                            || (!mutable_view && result.multiplicity != target.multiplicity) { return None; }
                        if mutable_view {
                            // The local's own declaration must hold the
                            // exclusive view: its binding is then the affine
                            // loan custody `let` planning gave a `&mut` view.
                            // A shared view local never gains mutation here.
                            let StatementNode::LocalData(local) = program.statement_table.statements(source.statement_nodes).get(result.statement_index as usize)? else { return None; };
                            if local.is_mutable
                                || result.multiplicity != Multiplicity::Affine
                                || !validation::slice_view_shell(program, local.type_reference, &[])
                                    .is_some_and(|view| view.access == language_core::ReferenceAccess::Mutable)
                            { return None; }
                        }
                        return Some(CheckedStructuralControlTransferPlan {
                            source: checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal: result.binding_ordinal },
                            target_parameter_index: u32::try_from(target_index).ok()?,
                        });
                    }
                }
            }
            if let Some((subject_place, selected_case)) = &case_test
                && let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                    program, source.symbol, ordinal as usize, expression,
                )
            {
                mark(SuccessorGuard::CasePayloadTransfer);
                if let Some(source) = case_payload_transfer(
                    program,
                    place,
                    subject_place,
                    *selected_case,
                    source,
                    source_structural,
                    operations,
                    ordinal,
                    target,
                ) {
                    return Some(CheckedStructuralControlTransferPlan {
                        source,
                        target_parameter_index: u32::try_from(target_index).ok()?,
                    });
                }
            }
            mark(SuccessorGuard::EstablishedArgument);
            // A fresh composite argument — a record, case, or fixed-array
            // literal, or a call's structural result — has no custody root to
            // name; the edge establishes the registered value into the
            // parameter's storage. Its calls are planned exactly as a `let`
            // initializer's are, against this state's parameters and its own
            // earlier local results, and run on the selected edge only. A
            // whole owned operand naming an affine local moves it on this
            // edge alone: the sibling edges still dispose of it, so the
            // state's operations keep their discard flags and the edge's
            // own move roster subtracts it from this edge's cleanup. Nested
            // structural operands remain in the edge's operation sequence;
            // they run after selection through the ordinary call emitter.
            if let Some(root) = facts.values.structural_values.root_for_expression(
                    source.symbol,
                    ordinal,
                    expression,
                )
                && root.machine == machine.symbol
                && (target.access == CheckedStructuralAccess::Owned
                    || validation::slice_view_shell(program, root.type_reference, &[]).is_some())
            {
                let multiplicity = if target.access == CheckedStructuralAccess::Owned {
                    if program.normalized_type_identity(root.type_reference).as_str()
                        != target.type_identity
                    {
                        return None;
                    }
                    target.multiplicity
                } else {
                    // A descriptor is constructed here just as in a local
                    // initializer. Its mutable producer owes affine cleanup;
                    // the successor borrows it and does not acquire that debt.
                    // Type correspondence does not grant the backing loan:
                    // the registered value already rejoins its exact call
                    // occurrence and ordered checked borrow accesses.
                    let view = validation::slice_view_shell(program, root.type_reference, &[])?;
                    let access: CheckedStructuralAccess = view.access.into();
                    let parameter = target_parameters.get(target.position as usize)?;
                    if access != target.access
                        || target.multiplicity != Multiplicity::Unrestricted
                        || program.normalized_type_identity(root.type_reference)
                            != program.normalized_type_identity(parameter.type_reference)
                        || super::super::types::base_type_identity(program, root.type_reference, &[])?
                            != target.type_identity
                    {
                        return None;
                    }
                    // Custody follows the declared access. Common structural
                    // establishment authenticates the registered producer,
                    // including descriptor copies from a record field; the
                    // edge must not maintain another producer-kind catalog.
                    match access {
                        CheckedStructuralAccess::SharedBorrow => Multiplicity::Unrestricted,
                        CheckedStructuralAccess::MutableBorrow => Multiplicity::Affine,
                        _ => return None,
                    }
                };
                // Edge-local bindings number after the state's own results,
                // so no source result ordinal is reused on this edge.
                // Scratch includes the predecessor's result producers so the
                // common planner can resolve and consume local operands. Its
                // changed discard flags never escape into sibling edges;
                // selected-edge cleanup owns those predecessor moves.
                let mut operand_calls = operations.to_vec();
                let mut result_count = super::returns::next_result_ordinal(operations)?;
                let calls = control::structural_operands::value_calls(
                    program,
                    facts,
                    edge_calls.scalar_callees,
                    shapes,
                    machine,
                    source,
                    source_structural,
                    &[],
                    edge_calls.entry_claims,
                    edge_calls.local_results,
                    &operand_calls,
                    &mut result_count,
                    root.root,
                    trace,
                )?;
                mark(SuccessorGuard::EstablishedArgument);
                let result = checked_trees::CheckedUnitStructuralResultBindingPlan {
                    statement_index: ordinal,
                    binding_ordinal: u32::try_from(result_count).ok()?,
                    type_identity: target.type_identity.clone(),
                    multiplicity,
                };
                let establishment = control::structural_operands::establish_value(
                    facts, result, root.root, calls, None, false, &mut operand_calls,
                )?;
                let operand_calls = operand_calls.split_off(operations.len());
                return Some(CheckedStructuralControlTransferPlan {
                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                        operands: checked_trees::CheckedOperandSequence::new(operand_calls),
                        establishment: Box::new(establishment),
                    },
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            mark(SuccessorGuard::ProjectedValue);
            // A static subtree keeps its root and complete path. Multiplicity
            // decides whether emission copies the leaf or transfers its
            // custody; selected normal cleanup owns any remaining complement.
            if let Some(subject) = projected_value_subject(
                    program,
                    machine,
                    source,
                    source_structural,
                    edge_calls.local_results,
                    ordinal,
                    expression,
                    target,
                )
                && subject.access == target.access
            {
                return Some(CheckedStructuralControlTransferPlan {
                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue {
                        subject,
                    },
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            mark(SuccessorGuard::LoanTransfer);
            // A reference argument loans its resolved place into the borrowed
            // target parameter. Declared correspondence covers explicit borrows,
            // forwarded references and implicit shared field loans alike; an
            // `&` token is not the authority for the transfer. The successor
            // argument keeps the source's rooted place and walks the
            // projection path below it, so the loan names the same storage
            // the borrow does rather than a fresh establishment. A projected
            // argument never forwards a same-typed unrelated parameter.
            if target.multiplicity == Multiplicity::Unrestricted
                && matches!(target.access,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                        | CheckedStructuralAccess::WriteOnlyBorrow)
                && validation::checked_argument_matches_type_reference(
                    program,
                    expression,
                    target_parameters.get(target.position as usize)?.type_reference,
                )
            {
                let expected_access = target.access;
                if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                        program,
                        source.symbol,
                        ordinal as usize,
                        expression,
                    )
                    && let facts::PlaceRoot::Symbol(root_symbol) = place.root
                {
                    // The root names either a source parameter — the authored
                    // `self.field` spelling roots at `self`'s own symbol while
                    // the implicit receiver roots at the machine — or the one
                    // earlier result binding a local published.
                    let parameter_subject = crate::semantic::calls::find_state(
                        program,
                        source.symbol,
                    )
                    .and_then(|typed_state| {
                        program
                            .state_parameters(typed_state)
                            .iter()
                            .enumerate()
                            .find(|(_, parameter)| {
                                super::super::types::parameter_is_root(parameter, root_symbol)
                            })
                            .and_then(|(position, _)| {
                                source_structural
                                    .iter()
                                    .enumerate()
                                    .find(|(_, candidate)| {
                                        candidate.position as usize == position
                                    })
                                    .map(|(index, structural)| (index, structural))
                            })
                    });
                    let result_subject = |local_index: usize| {
                        // Loan the local's established home, independently of
                        // its constructor. Call-owned operand temporaries and
                        // results retired at return still cannot back an edge.
                        let mut matches = operations.iter().filter(|operation| {
                            !operand_owned_construction(operation)
                                && !operation.discardable_structural_result()
                                    .is_some_and(|(_, discard)| discard)
                        }).filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                        .filter(|result| {
                            result.statement_index == local_index as u32
                        });
                        let result = matches.next()?;
                        if matches.next().is_some() {
                            return None;
                        }
                        Some(result)
                    };
                    let local_subject = || {
                        let (index, local) =
                            program.state_local_declaration(source, root_symbol)?;
                        Some((local, result_subject(index)?))
                    };
                    let loan_permitted = |root_access| match expected_access {
                        CheckedStructuralAccess::SharedBorrow => matches!(
                            root_access,
                            CheckedStructuralAccess::Owned
                                | CheckedStructuralAccess::SharedBorrow
                                | CheckedStructuralAccess::MutableBorrow
                        ),
                        CheckedStructuralAccess::MutableBorrow => matches!(
                            root_access,
                            CheckedStructuralAccess::Owned | CheckedStructuralAccess::MutableBorrow
                        ),
                        CheckedStructuralAccess::WriteOnlyBorrow => matches!(
                            root_access,
                            CheckedStructuralAccess::Owned
                                | CheckedStructuralAccess::MutableBorrow
                                | CheckedStructuralAccess::WriteOnlyBorrow
                        ),
                        CheckedStructuralAccess::Owned => false,
                    };
                    if let Some((source_index, source_parameter)) = parameter_subject
                    {
                        if loan_permitted(source_parameter.access)
                            && let Some(transfer) = projected_loan_transfer(
                                program,
                                shapes,
                                machine.symbol,
                                source.symbol,
                                ordinal,
                                &place,
                                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index: u32::try_from(source_index).ok()?,
                                },
                                // Borrowed roots retain their existing referent
                                // custody: inspect that storage, not the outer
                                // reference shell representing the parameter's
                                // loan. Owned roots lend only the peeled
                                // projected subtree.
                                match source_parameter.access {
                                    CheckedStructuralAccess::SharedBorrow
                                    | CheckedStructuralAccess::MutableBorrow
                                    | CheckedStructuralAccess::WriteOnlyBorrow => {
                                        Some(source_parameter.type_identity.clone())
                                    }
                                    _ => None,
                                },
                                expected_access,
                                target,
                                target_parameters,
                                target_index,
                            )
                        {
                            return Some(transfer);
                        }
                    } else if let Some((local, result)) = local_subject() {
                        // A mutable loan keeps the local's exact constrained
                        // type: a bounded byte owner's represented predicates
                        // (`[u8; N] in Utf8`) travel with it, and the
                        // successor's declared qualifications must imply them
                        // (argument presentation below). Checking proves each
                        // store through the loan preserves the domain.
                        let mutable_local_loan = expected_access
                            == CheckedStructuralAccess::SharedBorrow
                            || ((local.is_mutable || !local.initial_value.is_valid())
                                && matches!(
                                    result.multiplicity,
                                    Multiplicity::Affine | Multiplicity::Unrestricted
                                )
                                && validation::has_contents(
                                    program,
                                    local.type_reference, validation::ContentsRequirement::RepresentedOwned)
                                && !super::super::types::type_graph_requires_nominal_drop(
                                    program,
                                    local.type_reference,
                                ));
                        if mutable_local_loan
                            && let Some(transfer) = projected_loan_transfer(
                                program,
                                shapes,
                                machine.symbol,
                                source.symbol,
                                ordinal,
                                &place,
                                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                    binding_ordinal: result.binding_ordinal,
                                },
                                None,
                                expected_access,
                                target,
                                target_parameters,
                                target_index,
                            )
                        {
                            return Some(transfer);
                        }
                    }
                }
            }
            None
        })
        .collect::<Option<Vec<_>>>()?;
    mark(SuccessorGuard::ScalarArguments);
    let scalar_arguments = target_scalar
        .iter()
        .enumerate()
        .filter(|(index, _)| !payload_parameters.contains(&(*index as u32)))
        .map(|(target_index, target)| {
            let argument = argument_at(target.source_position)?;
            let role = CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: target.source_position,
            };
            let pure = &facts.values.scalar_expressions;
            let computations = &facts.values.scalar_computations;
            let mut roots = computations
                .roots
                .iter()
                .map(|(_, root)| root)
                .filter(|root| {
                    root.state == source.symbol
                        && root.statement_ordinal == ordinal
                        && root.role == role
                });
            let root = roots.next();
            if roots.next().is_some() {
                return None;
            }
            if let Some(root) = root {
                let has_pure = pure.source_bindings.iter().any(|(_, binding)| {
                    binding.state == source.symbol
                        && binding.statement_ordinal == ordinal
                        && binding.role == role
                }) || pure.expressions.iter().any(|expression| {
                    expression.state == source.symbol
                        && expression.statement_ordinal == ordinal
                        && expression.role == role
                });
                if has_pure
                    || root.machine != machine.symbol
                    || !computations.nodes.is_valid(root.root)
                    || computations.nodes.get(root.root).authored_root != argument
                    || computations.nodes.get(root.root).primitive_type != target.primitive_type
                {
                    return None;
                }
            } else {
                let (custody, expression) =
                    pure.bound_expression_at(source.symbol, ordinal, role)?;
                if custody.expression != argument
                    || custody.destination
                        != target_parameters
                            .get(target.source_position as usize)?
                            .symbol
                    || crate::values::scalar_expression_type(expression)
                        != Some(target.primitive_type)
                {
                    return None;
                }
            }
            let immutable_source_position = source_position(target.source_position)
                .filter(|position| !program.state_parameters(source)[*position].is_mutable);
            let source = if let Some(source_position) = immutable_source_position {
                let source_index = source_scalar
                    .iter()
                    .position(|parameter| parameter.source_position as usize == source_position)?;
                if source_scalar[source_index].primitive_type != target.primitive_type {
                    return None;
                }
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter {
                    index: u32::try_from(source_index).ok()?,
                }
            } else {
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
            };
            Some(CheckedStructuralScalarArgumentPlan {
                argument_ordinal: target.source_position,
                source,
                target_scalar_parameter_index: u32::try_from(target_index).ok()?,
                primitive_type: target.primitive_type,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    mark(SuccessorGuard::ErasedArguments);
    // Erased formals read the same retained transition expressions but index
    // the proof-only roster densely; their lowered terms are emitted against
    // the source state's erased namespace.
    let erased_arguments =
        crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, target)?
            .iter()
            .enumerate()
            .map(|(erased_index, target)| {
                let argument = argument_at(target.source_position)?;
                let (custody, expression) = facts.values.scalar_expressions.bound_expression_at(
                    source.symbol,
                    ordinal,
                    CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: target.source_position,
                    },
                )?;
                if custody.expression != argument
                    || custody.destination
                        != target_parameters
                            .get(target.source_position as usize)?
                            .symbol
                    || crate::values::scalar_expression_type(expression)
                        != Some(target.primitive_type)
                {
                    return None;
                }
                Some(CheckedStructuralScalarArgumentPlan {
                    argument_ordinal: target.source_position,
                    source: checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression,
                    target_scalar_parameter_index: u32::try_from(erased_index).ok()?,
                    primitive_type: target.primitive_type,
                })
            })
            .collect::<Option<Vec<_>>>()?;
    mark(SuccessorGuard::ErasedProofArguments);
    // Erased contract-term formals carry proof terms, not scalar
    // expressions: each actual is the term recorded under this edge's exact
    // coordinate by the scalar expression pass.
    let erased_proof_arguments =
        crate::execution::terminal_unit::types::erased_proof_parameter_plans(program, target)?
            .iter()
            .map(|target| {
                facts
                    .values
                    .proof_terms
                    .term_at(
                        source.symbol,
                        ordinal,
                        checked_trees::CheckedProofTermRole::TransitionArgument {
                            argument_ordinal: target.source_position,
                        },
                    )
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: target.symbol,
        transfers,
        scalar_arguments,
        erased_arguments,
        erased_proof_arguments,
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}

/// Resolve a finite field/index path below an established source root.
/// Unrestricted observations may read a borrowed root; transferring affine or
/// linear custody requires an owned root. Parameter and local establishments
/// share that rule and the target's exact subtree type.
fn projected_value_subject(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    source_structural: &[CheckedUnitStructuralParameterPlan],
    local_results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
    target: &CheckedUnitStructuralParameterPlan,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        source.symbol,
        ordinal as usize,
        expression,
    )?;
    let facts::PlaceRoot::Symbol(root_symbol) = place.root else {
        return None;
    };
    if place.segments.is_empty() {
        return None;
    }
    let parameter = program
        .state_parameters(source)
        .iter()
        .enumerate()
        .find(|(_, parameter)| super::super::types::parameter_is_root(parameter, root_symbol));
    let (argument_source, root_access) = if let Some((position, _)) = parameter {
        let (parameter_index, parameter) = source_structural
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.position as usize == position)?;
        (
            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            parameter.access,
        )
    } else {
        let (declaration, local) = program.state_local_declaration(source, root_symbol)?;
        let mut candidates = local_results.iter().filter(|(result, root)| {
            *root == place.root
                && result.statement_index == declaration as u32
                && result.statement_index < ordinal
        });
        let (result, _) = candidates.next()?;
        if candidates.next().is_some()
            || super::super::types::base_type_identity(program, local.type_reference, &[])?
                != result.type_identity
        {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            super::super::types::structural_access_for_type_reference(
                program,
                local.type_reference,
            )?,
        )
    };
    if root_access == CheckedStructuralAccess::WriteOnlyBorrow {
        return None;
    }
    let (projected_type, path) = calls::projected_borrowed_path(
        program,
        machine.symbol,
        source.symbol,
        ordinal as usize,
        &place,
    )?;
    if path.is_empty()
        || !path
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_) | CheckedUnitStructuralPathSegment::FixedIndex(_)))
        || program.type_multiplicity(projected_type) != target.multiplicity
        || (target.multiplicity != Multiplicity::Unrestricted
            && root_access != CheckedStructuralAccess::Owned)
        // This is an existing value of the target's exact declared type,
        // not a fresh initializer or permission to erase cleanup/loans.
        // Its contents need no second "plain storage" classification.
        // The signature's structural identities are storage identities:
        // domain constraints and reference shells peel, so a borrowed leaf
        // matches its referent and a `T in D` leaf matches `T`'s base —
        // the same base-identity convention argument presentation uses.
        || super::super::types::base_type_identity(program, projected_type, &[])
            .is_none_or(|identity| identity != target.type_identity)
    {
        return None;
    }
    let access =
        super::super::types::structural_access_for_type_reference(program, projected_type)?;
    Some(CheckedUnitStructuralArgumentPlan {
        source: argument_source,
        path,
        type_identity: target.type_identity.clone(),
        access,
    })
}

/// A projected-loan edge transfer against an already-admitted place:
/// `subject_source` is the parameter or earlier result the loan rides on
/// and `borrowed_identity` is the borrowed storage's referent identity —
/// `None` computes the peeled base identity of the projected type.
///
/// A projected loan keeps its original storage root and exact path across
/// the edge. Case steps use this same route, including deeper payload
/// fields; receiving verification must prove each selected case active
/// before rebinding the loan. Reference-bearing storage still needs its own
/// carrier custody and is not admitted here: the shape table keys storage
/// identities, so an owned projection names its peeled base while a
/// borrowed root inspects the referent. A state argument has the same
/// presentation as a call argument; access, live-home and reference-content
/// checks remain independent.
#[allow(clippy::too_many_arguments)]
fn projected_loan_transfer(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: symbols::SymbolHandle,
    source: symbols::SymbolHandle,
    ordinal: u32,
    place: &crate::flow::CanonicalPlace,
    subject_source: CheckedUnitStructuralArgumentSourcePlan,
    borrowed_identity: Option<String>,
    expected_access: CheckedStructuralAccess,
    target: &CheckedUnitStructuralParameterPlan,
    target_parameters: &[typed_trees::signature::StateParameter],
    target_index: usize,
) -> Option<CheckedStructuralControlTransferPlan> {
    let (projected_type, path) =
        calls::projected_borrowed_path(program, machine, source, ordinal as usize, place)?;
    let target_type_reference = target_parameters
        .get(target.position as usize)?
        .type_reference;
    if !path.iter().all(|segment| {
        matches!(
            segment,
            CheckedUnitStructuralPathSegment::Field(_)
                | CheckedUnitStructuralPathSegment::Case(_)
                | CheckedUnitStructuralPathSegment::FixedIndex(_)
        )
    }) || !calls::argument_presentation::is_admitted(
        program,
        projected_type,
        target_type_reference,
        &target.type_identity,
        &[],
    ) {
        return None;
    }
    let identity = borrowed_identity
        .or_else(|| super::super::types::base_type_identity(program, projected_type, &[]));
    if identity.is_some_and(|identity| declared_contains_reference(shapes, &identity)) {
        return None;
    }
    let carrier_identity = if path.is_empty() {
        String::new()
    } else {
        shapes.add_reference_type(target_type_reference, &[])?
    };
    Some(CheckedStructuralControlTransferPlan {
        source: checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
            subject: CheckedUnitStructuralArgumentPlan {
                source: subject_source,
                path,
                type_identity: target.type_identity.clone(),
                access: expected_access,
            },
            carrier_identity,
        },
        target_parameter_index: u32::try_from(target_index).ok()?,
    })
}

/// Admit a `[copy]` case-payload subtree transfer on a case-tested edge.
///
/// The edge's guard selected `selected_case` on `subject_place`, so an
/// argument whose place extends that subject through exactly that case
/// into one declared payload field carries proven membership into the
/// target's custody — the same authorization an authored case arm gives
/// its destructure bindings. The checked source keeps the resolved
/// subject argument plus the selected case and field identities, so a
/// downstream channel can rejoin the projected subtree without
/// re-deriving the destructure local.
fn case_payload_transfer(
    program: &TypedTrees,
    place: crate::flow::CanonicalPlace,
    subject_place: &crate::flow::CanonicalPlace,
    selected_case: symbols::SymbolHandle,
    source: &typed_trees::state::State,
    source_structural: &[CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
    ordinal: u32,
    target: &CheckedUnitStructuralParameterPlan,
) -> Option<checked_trees::CheckedStructuralControlTransferSourcePlan> {
    // The argument must name exactly one payload field beneath the edge's
    // proven case: `subject-place ++ [Case{selected}, Field{payload}]`.
    // Longer tails reach inside the payload field's own subtree and stay
    // unadmitted.
    if place.root != subject_place.root
        || place.segments.len() != subject_place.segments.len() + 2
        || place.segments[..subject_place.segments.len()] != subject_place.segments[..]
    {
        return None;
    }
    let [
        facts::PlaceSegment::Case { variant },
        facts::PlaceSegment::Field {
            symbol: field_symbol,
        },
    ] = &place.segments[subject_place.segments.len()..]
    else {
        return None;
    };
    if *variant != selected_case {
        return None;
    }
    // The selected case's declared payload field owns the transferred
    // subtree's identity and type.
    let field = program.data_definitions().iter().find_map(|data| {
        program.data_members(data).iter().find_map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.symbol == selected_case)
                .then(|| {
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == *field_symbol)
                })
                .flatten()
        })
    })?;
    // Copying the payload out of the borrowed subject must not disturb
    // the subject's custody. Two admitted shapes, each exact on declared
    // type and multiplicity: an `Owned` target copies a plain-contents
    // subtree and compares the member's whole identity; a `SharedBorrow`
    // target re-seats a member that is itself a shared borrow — reading
    // the view out of borrowed storage copies only its descriptor, so the
    // same loan transfers (loan->loan). A view parameter's declared
    // identity is its referent's, so the referee's normalized identity is
    // what the target compares against.
    let normalized = |reference| {
        // The signature's structural identities are storage identities:
        // domain constraints and reference shells peel, so the comparison
        // uses the same base-identity convention argument presentation does.
        super::super::types::base_type_identity(program, reference, &[])
    };
    let shared_referee = {
        let mut cursor = field.type_reference;
        loop {
            match program.type_reference_table.type_reference(cursor) {
                TypeReferenceNode::Constrained { base_type, .. } => cursor = *base_type,
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared,
                    referee,
                    ..
                } => break Some(*referee),
                _ => break None,
            }
        }
    };
    let admitted_shape = if target.access == CheckedStructuralAccess::Owned {
        // The payload subtree copies under the same contents judgment the
        // dispatch itself admits for the subject: a view descriptor inside
        // the carrier travels as a descriptor copy whose loan stays with
        // the view's owner, not with the dispatched place.
        (validation::has_contents(
            program,
            field.type_reference,
            validation::ContentsRequirement::NumericOwned,
        ) || validation::has_contents(
            program,
            field.type_reference,
            validation::ContentsRequirement::OwnedOrSharedView,
        )) && normalized(field.type_reference)
            .is_some_and(|identity| identity == target.type_identity)
    } else {
        target.access == CheckedStructuralAccess::SharedBorrow
            && super::super::types::structural_access_for_type_reference(
                program,
                field.type_reference,
            ) == Some(CheckedStructuralAccess::SharedBorrow)
            && shared_referee.is_some_and(|referee| {
                normalized(referee).is_some_and(|identity| identity == target.type_identity)
            })
    };
    if !admitted_shape || program.type_multiplicity(field.type_reference) != target.multiplicity {
        return None;
    }
    // A case test can observe either an arrival or an earlier local result.
    // Both use the same payload projection; a local must rejoin its unique
    // retained producer, not just a same-typed value in this state.
    let facts::PlaceRoot::Symbol(root) = subject_place.root else {
        return None;
    };
    let (subject_source, mut subject_type, root_identity) = if let Some(position) = program
        .state_parameters(source)
        .iter()
        .position(|parameter| parameter.symbol == root)
    {
        let (parameter_index, parameter) = source_structural
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.position as usize == position)?;
        (
            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            program.state_parameters(source)[position].type_reference,
            parameter.type_identity.clone(),
        )
    } else {
        let mut results = operations.iter().filter_map(|operation| {
            if operand_owned_construction(operation) {
                return None;
            }
            let result = retained_structural_result(operation)
                .filter(|result| result.statement_index < ordinal)?;
            let StatementNode::LocalData(local) = program
                .statement_table
                .statements(source.statement_nodes)
                .get(result.statement_index as usize)?
            else {
                return None;
            };
            (local.symbol == root && !local.is_mutable).then_some((result, local))
        });
        let (result, local) = results.next()?;
        if results.next().is_some() {
            return None;
        };
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            local.type_reference,
            result.type_identity.clone(),
        )
    };
    // The projection reaching the tested sum keeps the shared
    // structural-path vocabulary; the subject's own type is the field
    // type at the end of that projection.
    let path = subject_place
        .segments
        .iter()
        .map(|segment| {
            let facts::PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            subject_type = program.data_definitions().iter().find_map(|data| {
                program.data_members(data).iter().find_map(|member| {
                    let typed_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    (field.symbol == *symbol).then_some(field.type_reference)
                })
            })?;
            super::super::types::terminal_field_identity(program, *symbol)
                .map(CheckedUnitStructuralPathSegment::Field)
        })
        .collect::<Option<Vec<_>>>()?;
    let variant = program.data_definitions().iter().find_map(|data| {
        program.data_members(data).iter().find_map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.symbol == selected_case).then_some(variant)
        })
    })?;
    // The recorded subject identity must spell the resolved structural
    // type the emitted declaration catalog uses. A bare subject's checked
    // parameter already carries that identity — attached `self` names the
    // machine's nominal type in the typed table, so normalizing its
    // declared reference would spell the wrong owner. A projected subject
    // ends on a field's declared type, whose canonical identity peels any
    // borrow/constrained wrapper the same way the field declaration does.
    let subject_identity = if path.is_empty() {
        root_identity
    } else {
        normalized(subject_type)?
    };
    Some(
        checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
            subject: CheckedUnitStructuralArgumentPlan {
                source: subject_source,
                path,
                type_identity: subject_identity,
                access: CheckedStructuralAccess::SharedBorrow,
            },
            case_identity: variant.path_identity(),
            field_identity: field.path_identity(),
            path: Vec::new(),
        },
    )
}
