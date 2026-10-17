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
    Multiplicity, TransitionExit, TransitionGuardNode, TransitionTargetNode, TypeReferenceNode,
};
use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::control;
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::expression::ExpressionHandle;

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

/// The callee catalog, source-state entry claims and source-state local
/// results an edge plans its own operand calls against. A composite
/// argument's call runs only on the selected edge, under the same argument
/// custody an in-sequence call takes: an operand naming one of the state's
/// `let` locals (or an anonymous result it established) resolves to that
/// result binding exactly as a later statement's call would.
#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) struct EdgeCalls<'a> {
    pub(super) scalar_callees: ScalarCalleeClosure,
    pub(super) entry_claims: &'a [checked_trees::CheckedUnitEntryClaimPlan],
    pub(super) local_results: &'a [(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
}

pub(in crate::execution::terminal_unit) fn successor(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
                validation::canonical_place_from_expression_in_state(
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
        let place = validation::canonical_place_from_expression_in_state(
            program,
            source.symbol,
            ordinal as usize,
            argument,
        )?;
        let symbol = place.root.as_symbol()?;
        if !place.segments.is_empty() {
            return None;
        }
        typed_trees::signature::state_parameter_position(program.state_parameters(source), symbol)
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
                // A whole owned move carries the source's complete contract.
                // An argument that adds or erases a membership is a fresh
                // value with the target's contract, established below.
                && (target.access != CheckedStructuralAccess::Owned
                    || (source_parameter.qualifications == target.qualifications
                        && source_parameter.projected_qualifications
                            == target.projected_qualifications))
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
                && let Some(place) = validation::canonical_place_from_expression_in_state(program, source.symbol, ordinal as usize, expression)
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
                // Direct result forwarding is valid only while that result
                // remains the named binding's current occurrence. A replaced
                // home is evaluated below through the ordinary edge operand
                // route; forwarding its initializer would revive an old value.
                if place.segments.is_empty()
                    && let Some(symbol) = place.root.as_symbol()
                    && let Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }) =
                        super::super::local_storage::source(program, source, operations, symbol)
                    // As for a forwarded parameter: an owned move carries the
                    // local's declared contract, and an argument adding or
                    // erasing a membership is established below instead.
                    && (target.access != CheckedStructuralAccess::Owned
                        || program.state_local_declaration(source, symbol).is_some_and(|(_, local)| {
                            super::super::types::parameter_qualifications(
                                program, shapes, local.type_reference,
                                &super::super::types::machine_binders(program, machine),
                            )
                            .is_some_and(|qualifications| qualifications == target.qualifications)
                        }))
                {
                    let mut matches = operations.iter()
                        .filter(|operation| !operand_owned_construction(operation))
                        .filter_map(retained_structural_result)
                        .filter(|result| result.binding_ordinal == binding_ordinal && result.statement_index < ordinal);
                    if let Some(result) = matches.next() {
                        if matches.next().is_some() { return None; }
                        let presents_target = result.type_identity == target.type_identity
                            || (target.access != CheckedStructuralAccess::Owned
                                && program.state_local_declaration(source, symbol).is_some_and(|(_, local)| {
                                    super::super::types::stored_type_presents_borrowed(
                                        program, local.type_reference, result.type_identity, target.type_identity,
                                    )
                                }));
                        if !presents_target
                            || (!mutable_view && result.multiplicity != target.multiplicity) { return None; }
                        if mutable_view {
                            // The local's own declaration must hold the
                            // exclusive view: its binding is then the affine
                            // loan custody `let` planning gave a `&mut` view.
                            // A shared view local never gains mutation here.
                            let local = program.statement_table.statements(source.statement_nodes).get(result.statement_index as usize)?.as_local_data()?;
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
                && let Some(place) = validation::canonical_place_from_expression_in_state(
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
            // Reuse established storage before planning a fresh value. A
            // saved dispatch result is one occurrence, not a second call.
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
            mark(SuccessorGuard::EstablishedArgument);
            // A fresh composite argument — a record, case, or fixed-array
            // literal, or a call's structural result — has no custody root to
            // name; the edge establishes its checked home and transfers or
            // lends that home to the successor. Its calls are planned as a `let`
            // initializer's are, against this state's parameters and its own
            // earlier local results, and run on the selected edge only. A
            // whole owned operand naming an affine local moves it on this
            // edge alone: the sibling edges still dispose of it, so the
            // state's operations keep their discard flags and the edge's
            // own move roster subtracts it from this edge's cleanup. Nested
            // structural operands remain in the edge's operation sequence;
            // they run after selection through the ordinary call emitter.
            if let Some(root) = control::structural_operands::evaluated_argument_root(
                program,
                facts,
                machine.symbol,
                source.symbol,
                ordinal as usize,
                expression,
                target_parameters.get(target.position as usize)?,
            )
            .filter(|root| {
                // Established edge values currently transport a whole home,
                // not an owner plus a selected loan path. Keep projected
                // borrows out of that route: copying the child would lose
                // the complete owner's cleanup obligation.
                target.access == CheckedStructuralAccess::Owned
                    || validation::slice_view_shell(program, root.type_reference, &[]).is_some()
                    || !matches!(
                        facts.values.structural_values.nodes.get(root.root).kind,
                        checked_trees::CheckedStructuralValueKind::Projection { .. }
                    )
            }) {
                let stored_reference = target.access != CheckedStructuralAccess::Owned
                    && validation::reference_result_custody::stored_loan_parts(program, root.type_reference).is_some();
                let multiplicity = if stored_reference {
                    validation::reference_result_custody::result_multiplicity(program, root.type_reference)
                } else if target.access == CheckedStructuralAccess::Owned {
                    // The edge transfers storage, so storage identities must
                    // agree. A qualified operand (`x as Counter in Ready`)
                    // keeps its carrier's storage; checking owns whether its
                    // memberships meet the successor's declared domains.
                    if super::super::types::base_type_identity(program, root.type_reference, &[], &[])?
                        != target.type_identity
                    {
                        return None;
                    }
                    target.multiplicity
                } else if validation::slice_view_shell(program, root.type_reference, &[])
                    .is_some()
                {
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
                        || super::super::types::base_type_identity(program, root.type_reference, &[],
        &[])?
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
                } else {
                    // The shared operand owner authenticates the referent,
                    // not a copied reference descriptor. Preserve the actual
                    // producer's custody: affine homes travel through the
                    // successor loan-chain owner and die at that chain's end.
                    // No nominal hook or linear debt may become a plain death.
                    let multiplicity = validation::reference_result_custody::result_multiplicity(
                        program,
                        root.type_reference,
                    );
                    if target.access != CheckedStructuralAccess::SharedBorrow
                        || target.multiplicity != Multiplicity::Unrestricted
                        || !matches!(multiplicity, Multiplicity::Unrestricted | Multiplicity::Affine)
                        || (multiplicity == Multiplicity::Affine
                            && (!validation::has_contents(
                                program,
                                root.type_reference, &[],
                                validation::ContentsRequirement::OwnedOrSharedView,
                            ) || super::super::types::type_graph_requires_nominal_drop(
                                program,
                                root.type_reference,
                                &[],
                                &mut std::collections::BTreeSet::new(),
                            )))
                        || super::super::types::base_type_identity(
                            program,
                            root.type_reference,
                            &[],
                            &[],
                        )? != target.type_identity
                    {
                        return None;
                    }
                    multiplicity
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
                    &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
                )?;
                mark(SuccessorGuard::EstablishedArgument);
                let result = checked_trees::CheckedUnitStructuralResultBindingPlan {
                    statement_index: ordinal,
                    binding_ordinal: u32::try_from(result_count).ok()?,
                    type_identity: if stored_reference {
                        shapes.add_reference_type(root.type_reference, &super::super::types::machine_binders(program, machine))?
                    } else { target.type_identity.clone() },
                    multiplicity,
                    local: Default::default(),
                };
                let presentation = if stored_reference {
                    let owner = facts::PlaceRoot::Expression(expression);
                    calls::retained_reference_argument(
                        program, machine.symbol, source, ordinal as usize,
                        &validation::CanonicalPlace { root: owner, segments: Vec::new() },
                        owner, root.type_reference, &result,
                        target_parameters.get(target.position as usize)?, target.type_identity,
                        trace,
                    )?
                } else {
                    CheckedUnitStructuralArgumentPlan {
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path: Vec::new(),
                        type_identity: target.type_identity,
                        access: target.access,
                    }
                };
                let establishment = control::structural_operands::establish_value(
                    facts, result, root.root, calls, None, false, &mut operand_calls,
                )?;
                let operand_calls = operand_calls.split_off(operations.len());
                return Some(CheckedStructuralControlTransferPlan {
                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                        operands: checked_trees::CheckedOperandSequence::new(operand_calls),
                        establishment: Box::new(establishment),
                        presentation,
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
            // A loan transports access, not ownership of its referent. The
            // source checker has admitted this exact edge and its lifetime;
            // requiring copyable contents would discard valid affine backing.
            if matches!(target.access,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                        | CheckedStructuralAccess::WriteOnlyBorrow)
                && validation::checked_argument_matches_type_reference(
                    program,
                    expression,
                    target_parameters.get(target.position as usize)?.type_reference,
                )
            {
                let expected_access = target.access;
                if let Some(place) = validation::canonical_place_from_expression_in_state(
                        program,
                        source.symbol,
                        ordinal as usize,
                        expression,
                    )
                    && let Some(root_symbol) = place.root.as_symbol()
                {
                    // The root names either a source parameter — the authored
                    // `self.field` spelling roots at `self`'s own symbol while
                    // the implicit receiver roots at the machine — or the one
                    // earlier result binding a local published.
                    let parameter_subject = program.state_by_symbol(source.symbol)
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
                        // its constructor. Detached call-owned operands cannot
                        // back an edge; normal-return cleanup remains scheduled.
                        let mut matches = operations.iter().filter(|operation| {
                            // Automatic normal-return discard is future cleanup,
                            // not retirement of this currently live local home.
                            !operand_owned_construction(operation)
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
                        // The ordered home query rejects detached results and
                        // retains rebinding identity across this state edge.
                        super::super::local_storage::source(program, source, operations, root_symbol)?;
                        Some((local, result_subject(index)?))
                    };
                    let loan_permitted = |root_access| match expected_access {
                        CheckedStructuralAccess::SharedBorrow => matches!(
                            root_access,
                            CheckedStructuralAccess::Owned
                                | checked_trees::checked_borrowed_access_pair_pattern!()),
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
                                facts,
                                shapes,
                                machine.symbol,
                                source.symbol,
                                ordinal,
                                expression,
                                &place,
                                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index: u32::try_from(source_index).ok()?,
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
                        if calls::is_stored_reference(program, local.type_reference) {
                            // A stored result owns a descriptor, not the value
                            // its borrowed successor presents. Calls and state
                            // edges retain the same authenticated crossing;
                            // failed reference admission cannot lend descriptor
                            // storage through the ordinary owner-loan route.
                            let mut subject = calls::retained_reference_argument(
                                program,
                                machine.symbol,
                                source,
                                ordinal as usize,
                                &place,
                                facts::PlaceRoot::Symbol(local.symbol),
                                local.type_reference,
                                result,
                                target_parameters.get(target.position as usize)?,
                                target.type_identity,
                                trace,
                            )?;
                            if matches!(subject.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }) {
                                subject.source = super::super::local_storage::source(
                                    program, source, operations, root_symbol,
                                )?;
                            }
                            if subject.access != expected_access
                                || !subject.path.iter().all(|segment| {
                                    matches!(
                                        segment,
                                        CheckedUnitStructuralPathSegment::Field(_)
                                            | CheckedUnitStructuralPathSegment::Case(_)
                                            | CheckedUnitStructuralPathSegment::FixedIndex(_)
                                            | CheckedUnitStructuralPathSegment::Referent
                                    )
                                })
                            {
                                return None;
                            }
                            return Some(CheckedStructuralControlTransferPlan {
                                source: checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                                    subject,
                                    formation_expression: expression,
                                    carrier_type_identity: Default::default(),
                                },
                                target_parameter_index: u32::try_from(target_index).ok()?,
                            });
                        }
                        // A mutable loan keeps the local's exact constrained
                        // type: a bounded byte owner's represented predicates
                        // (`[u8; N] in Utf8`) travel with it, and the
                        // successor's declared qualifications must imply them
                        // (argument presentation below). Checking proves each
                        // store through the loan preserves the domain.
                        // Lending an enrolled live home does not copy/recast
                        // its contents. Binding-backed fields retain their own
                        // receipts; endpoint preparation checks the selected
                        // referent and its exact descriptor route.
                        // A stored reference leaf supplies its own loan
                        // authority. Its immutable container does not make
                        // that referent shared, nor grant mutation of the
                        // container. Source admission checked the exact leaf;
                        // endpoint preparation retains its pointer crossing.
                        let stored_leaf_access = validation::canonical_place_type_reference(
                            program, source.symbol, ordinal as usize, &place,
                        ).and_then(|reference| {
                            validation::reference_result_custody::stored_loan_parts(program, reference)
                        }).map(|(_, access)| access);
                        // The established owned home remains in place under
                        // this loan, including its affine/linear and cleanup
                        // obligations. Those obligations do not turn a borrow
                        // into an owned transfer. A stored leaf instead keeps
                        // its own access ceiling; it cannot acquire authority
                        // from ownership of its descriptor's container.
                        let mutable_local_loan = stored_leaf_access
                            .map_or(true, loan_permitted);
                        if mutable_local_loan
                            && let Some(transfer) = projected_loan_transfer(
                                program,
                                facts,
                                shapes,
                                machine.symbol,
                                source.symbol,
                                ordinal,
                                expression,
                                &place,
                                super::super::local_storage::source(
                                    program, source, operations, root_symbol,
                                )?,
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
                    || !computations.is_authored_root(
                        root,
                        machine.symbol,
                        argument,
                        target.primitive_type,
                    )
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
    // The composed route mints full rosters through the one successor-plan
    // constructor; ordinary edges never select a continuation.
    Some(CheckedStructuralControlSuccessorPlan::new(
        ordinal,
        target.symbol,
        false,
        transfers,
        scalar_arguments,
        erased_arguments,
        erased_proof_arguments,
        Vec::new(),
    ))
}

/// Resolve a finite field/index path below an established source root.
/// Unrestricted observations may read a borrowed root; transferring affine or
/// linear custody requires an owned root. Parameter and local establishments
/// share that rule and the target's exact subtree type.
fn projected_value_subject(
    program: &dyn ::validation::CheckingInput,
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
    // Value transfer observes or moves the selected stored value. A borrowed
    // formal instead needs that value's referent and exact loan access; use
    // the common projected-loan endpoint below, never copy an exclusive
    // descriptor or substitute its referent's unrestricted multiplicity.
    if target.access != CheckedStructuralAccess::Owned {
        return None;
    }
    let place = validation::canonical_place_from_expression_in_state(
        program,
        source.symbol,
        ordinal as usize,
        expression,
    )?;
    if place.segments.is_empty() {
        return None;
    }
    let parameter = program
        .state_parameters(source)
        .iter()
        .enumerate()
        .find(|(_, parameter)| {
            place
                .root
                .as_symbol()
                .is_some_and(|symbol| super::super::types::parameter_is_root(parameter, symbol))
        });
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
        let mut candidates = local_results
            .iter()
            .filter(|(result, root)| *root == place.root && result.statement_index <= ordinal);
        let (result, _) = candidates.next()?;
        let root_type = validation::canonical_place_type_reference(
            program,
            source.symbol,
            ordinal as usize,
            &validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        if candidates.next().is_some()
            || super::super::types::base_type_identity(program, root_type, &[], &[])?
                != result.type_identity
        {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            super::super::types::structural_access_for_type_reference(program, root_type)?,
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
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_) | CheckedUnitStructuralPathSegment::FixedIndex(_) | CheckedUnitStructuralPathSegment::Case(_)))
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
        || super::super::types::base_type_identity(program, projected_type, &[],
        &[])
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
/// `subject_source` is the parameter or earlier result the loan rides on.
///
/// A projected loan keeps its original storage root and exact path across
/// the edge. Case steps use this same route, including deeper payload
/// fields; receiving verification must prove each selected case active
/// before rebinding the loan. Stored reference leaves cross their exact
/// descriptor under the same endpoint judgment as call arguments. Carrier
/// custody remains with the checked source; the selected referent, rather
/// than an unrelated enclosing record, determines the incoming storage type.
/// Reference-bearing referents still need their separate carrier custody.
/// A state argument has the same
/// presentation as a call argument; access, live-home and reference-content
/// checks remain independent.
#[allow(clippy::too_many_arguments)]
fn projected_loan_transfer(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: symbols::SymbolHandle,
    source: symbols::SymbolHandle,
    ordinal: u32,
    formation_expression: ExpressionHandle,
    place: &validation::CanonicalPlace,
    subject_source: CheckedUnitStructuralArgumentSourcePlan,
    expected_access: CheckedStructuralAccess,
    target: &CheckedUnitStructuralParameterPlan,
    target_parameters: &[typed_trees::signature::StateParameter],
    target_index: usize,
) -> Option<CheckedStructuralControlTransferPlan> {
    let computations = &facts.values.scalar_computations;
    let selector = |expression| {
        let root = computations.root_at(
            source,
            ordinal,
            CheckedScalarExpressionRole::TransitionSelector {
                argument_ordinal: target.position,
                expression,
            },
        )?;
        if !computations.nodes.is_valid(root.root) {
            return None;
        }
        let primitive = computations.nodes.get(root.root).primitive_type;
        (primitive.is_fixed_integer()
            && computations.is_authored_root(root, machine, expression, primitive))
        .then_some(CheckedUnitStructuralPathSegment::ComputedIndex(root.root))
    };
    let (projected_type, path) = calls::projected_evaluated_path(
        program,
        machine,
        source,
        ordinal as usize,
        place,
        Some(&selector),
    )?;
    let target_type_reference = target_parameters
        .get(target.position as usize)?
        .type_reference;
    let (projected_type, path) =
        calls::borrowed_argument_endpoint(program, projected_type, path, target_type_reference)?;
    if !path.iter().all(|segment| {
        matches!(
            segment,
            CheckedUnitStructuralPathSegment::Field(_)
                | CheckedUnitStructuralPathSegment::Case(_)
                | CheckedUnitStructuralPathSegment::FixedIndex(_)
                | CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::Parameter { .. }
                        | checked_trees::CheckedRuntimeIndex::Local { .. }
                )
                | CheckedUnitStructuralPathSegment::ComputedIndex(_)
                | CheckedUnitStructuralPathSegment::Referent
        )
    }) || !calls::argument_presentation::classify(
        program,
        projected_type,
        target_type_reference,
        target.type_identity,
        &[],
    )
    .is_some_and(|presentation| {
        // A raw-array-to-slice edge still needs its descriptor formation;
        // compatibility alone does not establish that incoming carrier.
        presentation != calls::argument_presentation::ArgumentPresentation::FixedArraySlice
    }) {
        return None;
    }
    // This loans the aggregate's existing home; it does not copy its embedded
    // references. Ordinary reference formation and continuation replay retain
    // the complete supplier/parent closure, including reference-bearing fields.
    let has_dynamic_selector = path.iter().any(|segment| {
        matches!(
            segment,
            CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                | CheckedUnitStructuralPathSegment::ComputedIndex(_)
        )
    });
    // A bounded byte view already carries its pointer under the full declared
    // reference identity. Static rooted storage lends that backing directly;
    // enrolling an additional Reference shape would add an indirection and
    // conflict with the existing carrier. Dynamic selectors still require an
    // explicit retained descriptor, and refuse when its carrier is unavailable.
    let carrier_type_identity = if matches!(
        super::super::types::byte_sequence_carrier(program, target_type_reference, &[]),
        Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. })
    ) && !has_dynamic_selector
    {
        Default::default()
    } else {
        match shapes.add_reference_type(target_type_reference, &[]) {
            Some(identity) => identity,
            None if !has_dynamic_selector => Default::default(),
            None => return None,
        }
    };
    Some(CheckedStructuralControlTransferPlan {
        source: checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
            formation_expression,
            carrier_type_identity,
            subject: CheckedUnitStructuralArgumentPlan {
                source: subject_source,
                path,
                type_identity: target.type_identity.clone(),
                access: expected_access,
            },
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
    program: &dyn ::validation::CheckingInput,
    place: validation::CanonicalPlace,
    subject_place: &validation::CanonicalPlace,
    selected_case: symbols::SymbolHandle,
    source: &typed_trees::state::State,
    source_structural: &[CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
    ordinal: u32,
    target: &CheckedUnitStructuralParameterPlan,
) -> Option<checked_trees::CheckedStructuralControlTransferSourcePlan> {
    // Affine contents move through the ordinary projected-value route below.
    // This observation route must not turn their plain bytes into copyability.
    if target.access == CheckedStructuralAccess::Owned
        && target.multiplicity != Multiplicity::Unrestricted
    {
        return None;
    }
    // The argument must name exactly one payload field beneath the edge's
    // proven case: `subject-place ++ [Case{selected}, Field{payload}]`.
    // Deeper owned projections use the ordinary rooted transfer route;
    // this observation route establishes only the declared payload itself.
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
    let field = program
        .variant_owner(selected_case)
        .and_then(|(_, variant)| program.data_payload_field_by_symbol(variant, *field_symbol))?;
    // Copying the payload out of the borrowed subject must not disturb
    // the subject's custody. Two admitted shapes, each exact on declared
    // type and multiplicity: an unrestricted `Owned` target copies a plain-contents
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
        super::super::types::base_type_identity(program, reference, &[], &[])
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
            &[],
            validation::ContentsRequirement::NumericOwned,
        ) || validation::has_contents(
            program,
            field.type_reference,
            &[],
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
    let root = subject_place.root.as_symbol()?;
    let (subject_source, mut subject_type, root_identity) = if let Some(position) =
        typed_trees::signature::state_parameter_position(program.state_parameters(source), root)
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
            let local = program
                .statement_table
                .statements(source.statement_nodes)
                .get(result.statement_index as usize)?
                .as_local_data()?;
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
            subject_type = program
                .field_owner(*symbol)
                .map(|(_, field)| field.type_reference)?;
            super::super::types::terminal_field_identity(program, *symbol)
                .map(CheckedUnitStructuralPathSegment::Field)
        })
        .collect::<Option<Vec<_>>>()?;
    let variant = program
        .variant_owner(selected_case)
        .map(|(_, variant)| variant)?;
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
            case_identity: checked_trees::CheckedMemberIdentity::of_member(
                variant.identity,
                variant.name.clone(),
            ),
            field_identity: checked_trees::CheckedMemberIdentity::of_member(
                field.identity,
                field.name.clone(),
            ),
            path: Vec::new(),
        },
    )
}
