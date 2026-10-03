//! State-local calls and explicit successor bindings, independent of graph shape.
//!
//! `build_traced` plans a multi-state machine as a composed Unit control
//! graph. It fixes the machine's result signature and natural ranks, then
//! the signature and entry claims of every state. Each state then plans its
//! prefix initializers and statement sequence (`control::statement_sequence`),
//! admits the sequence's operations (`operation_custody`), plans its
//! terminator (`terminator`, with `closed_sum` and `returns`), and accounts
//! for every structural result it produces (`result_custody`, with
//! `local_results`). Finally the parameter custody of live states and the
//! provider attachment requirements are checked.
use super::{
    CheckFacts, CheckedByteSequenceCarrier, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedGuardedJumpPlan, CheckedScalarBinding, CheckedScalarBindingValue,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, ExpressionNode, Multiplicity, PermissionEventKind,
    PermissionEventSource, PrimitiveType, StatementNode, TransitionExit, TransitionGuardNode,
    TransitionTargetNode, TypeReferenceNode, TypedTrees, calls,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::calls::{
    entry_claims, free_structural_scalar_signature_traced, structural_scalar_signature_traced,
};
use crate::execution::terminal_unit::providers::checked_composed_provider_attachment_requirements;
use crate::execution::terminal_unit::returns::checked_boolean_contains_short_circuit;
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_reference, machine_binders, return_unit_affine_discards, state_flow,
};
use crate::execution::terminal_unit::types::{borrowed_slice_view_element, byte_sequence_carrier};
use crate::execution::terminal_unit::{composed_control, control};

mod closed_sum;
mod local_results;
mod operation_custody;
mod result_custody;
pub(super) mod returns;
mod terminal_calls;
mod terminator;

#[cfg(test)]
mod tests;

/// A composed graph's state roster: the dispatch machine's states followed by
/// every foreign machine whose entry an arm can reach. Each entry pairs the
/// state with its owning machine — signatures, entry claims, and every
/// machine-scoped fact answer through the owner, never the dispatch machine.
type FusedStates<'program> = Vec<(
    &'program typed_trees::machine::Machine,
    &'program typed_trees::state::State,
)>;

/// The dispatch machine's state list fused with every machine its transition
/// arms can reach: `-> self.view(...)` names another machine's entry state,
/// and that machine's whole state list joins this graph so the arm stays an
/// in-graph edge — the same worklist rule the scalar machine graph applies
/// transitively. Only the edge list is fused; states keep their own owning
/// machine's parameters and contracts.
fn fused_states<'program>(
    program: &'program TypedTrees,
    machine: &'program typed_trees::machine::Machine,
) -> FusedStates<'program> {
    let mut fused: FusedStates<'program> = program
        .machine_states(machine)
        .iter()
        .map(|state| (machine, state))
        .collect();
    let mut cursor = 0;
    while let Some(&(owner, state)) = fused.get(cursor) {
        cursor += 1;
        let _ = owner;
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for target in [transition.target, transition.continuation] {
                if !program.statement_table.transition_target_is_valid(target) {
                    continue;
                }
                let TransitionTargetNode::Named { path, .. } =
                    program.statement_table.transition_target(target)
                else {
                    continue;
                };
                if fused_target_index(program, &fused, path.symbol).is_some() {
                    continue;
                }
                let Some((target_owner, _)) =
                    crate::semantic::calls::find_machine_by_entry_state(program, path.symbol)
                else {
                    continue;
                };
                for owner_state in program.machine_states(target_owner) {
                    if !fused
                        .iter()
                        .any(|(_, candidate)| candidate.symbol == owner_state.symbol)
                    {
                        fused.push((target_owner, owner_state));
                    }
                }
            }
        }
    }
    fused
}

/// The fused position of a named transition target: a state symbol resolves
/// directly, while a machine head resolves to that owner's entry state —
/// the same normalization `named_transition_target_state_index` applies to
/// one machine's own roster.
fn fused_target_index(
    program: &TypedTrees,
    fused: &FusedStates<'_>,
    target: symbols::SymbolHandle,
) -> Option<usize> {
    if let Some(index) = fused.iter().position(|(_, state)| state.symbol == target) {
        return Some(index);
    }
    let (_, entry) = crate::semantic::calls::find_machine_head(program, target)?;
    fused
        .iter()
        .position(|(_, state)| state.symbol == entry.symbol)
}

/// Test convenience: the traced builder without a trace.
#[cfg(test)]
pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    build_traced(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        call_frames,
        &control::LocalConstructionTrace::default(),
    )
}

/// `build` with a trace of the phase, state, and statement where the general
/// state-graph route stopped when it declines a body.
///
/// A borrowed `self` is a structural operand only while the graph names its
/// place: receiver reads, stores, projections, or lent fields
/// (`receiver_calls::uses_receiver_storage`), receiver-rooted borrows,
/// whole-`self` operands, or receiver-rooted results
/// (`receiver_rooted_operand`), a receiver-rooted call whose callee may keep
/// its `self` (`receiver_calls::calls_self_receiver`), or an edge feeding a
/// `self`-keeping successor. A state that never names it runs on the
/// ambient attachment the ordinary plans already erase, so no caller owes a
/// whole-receiver loan beside a field loan it also passes. The retention
/// set is fixed before signatures exist; if a build then declines because a
/// use the predicate missed still needed the place, the retry restores the
/// signature shape earlier passes produced unconditionally.
pub(super) fn build_traced(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let fused = fused_states(program, machine);
    // State-local `self` retention exists for fused rosters: a foreign
    // owner's receiver is a different `self` identity than the dispatch
    // machine's, so a fused state that never names its receiver runs on the
    // ambient attachment. A single-owner graph keeps the signature shape
    // the custody gate and its callers were proved against — every authored
    // `&mut self` state retains its receiver there.
    let retained_self = if fused
        .iter()
        .any(|(owner, _)| owner.symbol != machine.symbol)
    {
        retained_receivers(program, facts, &fused)
    } else {
        vec![true; fused.len()]
    };
    if retained_self.iter().all(|retained| *retained) {
        return build_traced_with(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            &fused,
            call_frames,
            trace,
            None,
        );
    }
    build_traced_with(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        &fused,
        call_frames,
        trace,
        Some(&retained_self),
    )
    .or_else(|| {
        build_traced_with(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            &fused,
            call_frames,
            trace,
            None,
        )
    })
}

/// `build_traced` under an explicit per-state `self` retention set. `None`
/// keeps every borrowed receiver, the signature shape this route produced
/// before retention became state-local.
#[allow(clippy::too_many_arguments)]
fn build_traced_with(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    fused: &FusedStates<'_>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &control::LocalConstructionTrace,
    retained_self: Option<&[bool]>,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    trace.phase("state graph: shape");
    trace.state(None);
    let states = program.machine_states(machine);
    if states.is_empty() || !machine_binders(program, machine).is_empty() {
        return None;
    }
    trace.phase("state graph: result signature");
    let result = returns::signature(program, shapes, states[0].return_type)?;
    if matches!(&result, checked_trees::CheckedControlResultPlan::Structural(result)
        if result.multiplicity == Multiplicity::Linear)
        && !program.machine_contracts(machine).is_empty()
    {
        // Parameter qualifications are transported exactly below. Additional
        // authored contracts still need their own retained proof obligations.
        return None;
    }
    // A plain single-state Unit body belongs to the ordinary sequence. A
    // transition tail does not: the ordinary plan has no terminator, so the
    // graph route takes every state reachable through transition arms —
    // including foreign machine entries fused into `fused`.
    if fused.len() < 2
        && result == checked_trees::CheckedControlResultPlan::Unit
        && !fused.iter().any(|(_, state)| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| matches!(statement, StatementNode::Transition(_)))
        })
        && !facts
            .values
            .structural_values
            .roots
            .iter()
            .any(|(_, root)| root.machine == machine.symbol)
        && !terminal_calls::has_guarded_machine_arm(program, machine, &states[0])
    {
        return None;
    }
    if let checked_trees::CheckedControlResultPlan::Scalar { .. } = result {
        // Producer ownership, not state count, chooses the body. One authored
        // state can still have a ranked backedge or selected return arms that
        // require the graph's terminator and proof machinery. Keep any existing
        // scalar producer; otherwise sequence and complete the body here.
        trace.phase("state graph: result signature: scalar owner precedence");
        if facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .is_some()
            || scalar_callees
                .structural_returns
                .for_machine(machine.symbol)
                .is_some()
            || scalar_callees
                .boundary_returns
                .machines
                .iter()
                .any(|plan| plan.machine == machine.symbol)
        {
            return None;
        }
        // A result guarantee (`ensures`, a closed result range) is the
        // machine's normal-return obligation, published on its contract and
        // proved at its exits, exactly as for an ordinary single-state body.
        // Only the machine signature may promise one; a later state's
        // contract admits entry facts only. A bound guarantee
        // (`ensures ... as name`) has no publication here.
        trace.phase("state graph: result signature: scalar result guarantee");
        if program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(&states[0]))
            .any(|contract| {
                !matches!(
                    contract.kind,
                    super::SignatureContractKind::Requires
                        | super::SignatureContractKind::Crashes { .. }
                        | super::SignatureContractKind::Ensures
                ) || contract.binding.is_some()
            })
            || states[1..]
                .iter()
                .flat_map(|state| program.state_contracts(state))
                .any(|contract| {
                    !matches!(
                        contract.kind,
                        super::SignatureContractKind::Requires
                            | super::SignatureContractKind::Crashes { .. }
                    )
                })
        {
            return None;
        }
    }
    trace.phase("state graph: natural ranks");
    let natural_ranks = if machine.termination_plan.implementation_witness.is_some() {
        // A fused graph's cyclic edges may cross machine boundaries; the
        // rank checker owns only the dispatch machine's own state list, so
        // a witnessed machine that reaches foreign entries stays unadmitted
        // until that custody extends to the whole fused roster.
        if fused.len() != states.len() {
            return None;
        }
        // Other retained witnesses belong to their existing producer until this
        // path can preserve them. Never publish an unranked replacement.
        let ranks = crate::checks::termination::proven_state_natural_ranks_with_call_frames(
            program,
            machine,
            call_frames,
        )?;
        if ranks.is_empty() {
            return None;
        }
        ranks
    } else {
        Vec::new()
    };
    let mut attachment = None;
    let mut signatures = Vec::new();
    let mut state_entry_claims = Vec::new();
    let mut state_requires = Vec::new();
    for (state_index, &(owner, state)) in fused.iter().enumerate() {
        trace.phase("state graph: state signature: result and contracts");
        trace.state(u32::try_from(state_index).ok());
        if returns::signature(program, shapes, state.return_type)? != result {
            trace.phase("state graph: state signature: result differs");
            return None;
        }
        // Membership contracts still restate the exact declared parameter
        // qualifications. Authored scalar `requires` expressions lower into
        // the state's proof-only contract roster; any other shape stays
        // outside the state graph's admitted custody.
        let Some(contract_predicates) =
            validation::structural_state_contract_scalar_predicates(program, state)
        else {
            trace.phase("state graph: state signature: contract shape unadmitted");
            return None;
        };
        let mut requires = Vec::with_capacity(contract_predicates.len());
        for row in contract_predicates {
            let predicate = match row {
                validation::StateScalarContractRow::Expression(expression) => {
                    crate::values::lower_state_scalar_contract_predicate(
                        program,
                        &facts.operators,
                        owner,
                        state,
                        expression,
                        &mut 4096,
                    )
                }
                validation::StateScalarContractRow::Interval {
                    parameter,
                    minimum,
                    maximum,
                } => crate::values::lower_state_interval_predicate(
                    program, state, parameter, minimum, maximum,
                ),
                validation::StateScalarContractRow::Finite { parameter } => {
                    validation::state_finite_predicate(program, state, parameter)
                }
            };
            let Some(predicate) = predicate else {
                trace.phase("state graph: state signature: predicate lowering failed");
                return None;
            };
            requires.push(Some(checked_trees::ClosedScalarContractValue::Predicate(
                predicate,
            )));
        }
        // An authored state transition closes the declared-domain window of
        // each readable argument. These are destination arrival obligations,
        // not facts inherited from the machine's first invocation or a loop's
        // previous iteration. Synthetic expression continuations are not
        // authored state boundaries and receive no such rows here.
        requires.extend(
            validation::state_declaration_predicates(program, &facts.operators, owner, state)
                .into_iter()
                .map(|row| {
                    Some(checked_trees::ClosedScalarContractValue::Predicate(
                        row.predicate,
                    ))
                }),
        );
        state_requires.push(requires);
        trace.phase("state graph: state signature: parameter signature");
        let (structural, scalar) = if owner.attached_data.is_some() {
            let (identity, structural, scalar) = structural_scalar_signature_traced(
                program,
                shapes,
                owner,
                state,
                &[],
                retained_self.is_none_or(|retained| retained[state_index]),
                trace,
            )?;
            // Fused states reached through `self.` transitions share the
            // receiver's attachment; any other owner would publish a second
            // attachment identity this plan cannot name.
            if attachment.is_some() && attachment != identity {
                trace.phase("state graph: state signature: fused attachment differs");
                return None;
            }
            attachment = attachment.or(identity);
            (structural, scalar)
        } else {
            free_structural_scalar_signature_traced(program, shapes, state, &[], trace)?
        };
        trace.phase("state graph: state signature: entry claims");
        let claims = entry_claims(
            program,
            facts,
            owner.symbol,
            state.symbol,
            &structural,
            program.state_parameters(state),
        )?;
        state_entry_claims.push(claims);
        signatures.push((structural, scalar));
    }
    let mut planned = Vec::new();
    for (state_index, &(owner, state)) in fused.iter().enumerate() {
        trace.phase("state graph: prefix initializers");
        trace.state(u32::try_from(state_index).ok());
        let (structural, scalar) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        // A body containing structural bindings is not a scalar-only prefix.
        // The shared sequence must account for every statement in that case.
        let mut bindings = crate::execution::terminal_scalar::checked_binding_prefix(
            program,
            state,
            &facts.values.scalar_computations,
        )
        .unwrap_or_default()
        .into_iter()
        // Entry scalar parameters already own current-value slots. Their
        // assignments use the same ordered SSA binding owner as immutable
        // locals. Mutable local declarations instead establish addressable
        // homes in ordinary sequencing; never split those initializers away
        // from the later reads, assignments, and loans that need the home.
        .take_while(|binding| {
            binding.value == CheckedScalarBindingValue::Expression
                && match binding.destination {
                    checked_trees::CheckedScalarBindingDestination::Immutable => true,
                    checked_trees::CheckedScalarBindingDestination::StorageAssign { symbol } => {
                        program.state_parameters(state).iter().any(|parameter| {
                            parameter.symbol == symbol
                                && crate::values::mutable_scalar_parameter_type(program, parameter)
                                    == Some(binding.primitive_type)
                        })
                    }
                    checked_trees::CheckedScalarBindingDestination::StorageInitialize {
                        ..
                    } => false,
                }
        })
        // An atomic load or result placeholder is an event, not a pure prefix.
        .take_while(|binding| {
            !super::atomic_operations::owns_local(
                program,
                state,
                binding.statement_ordinal as usize,
            )
        })
        .collect::<Vec<_>>();
        let binding_initializers = prefix_initializers(program, facts, state, &bindings, trace)?;
        bindings.truncate(binding_initializers.len());
        let binding_count = bindings.len();
        // Only a final expression completes a value-returning state. Earlier
        // Unit calls are ordinary operations, even when their source nodes
        // are expressions. Keep them in the shared statement sequence so
        // their call occurrences and following operations remain in order.
        let terminator_index = statements
            .iter()
            .enumerate()
            .position(|(statement_index, statement)| {
                matches!(statement, StatementNode::Transition(_))
                    || (result != checked_trees::CheckedControlResultPlan::Unit
                        && statement_index + 1 == statements.len()
                        && matches!(statement, StatementNode::Expression(_)))
            })
            .unwrap_or(statements.len());
        trace.phase("state graph: state flow");
        let flow = state_flow(facts, owner.symbol, state.symbol)?;
        let source_calls = facts.flow.control.calls.span_or_empty(flow.calls);
        if source_calls
            .windows(2)
            .any(|pair| pair[0].statement_index > pair[1].statement_index)
        {
            return None;
        }
        let first_call = source_calls.partition_point(|call| call.statement_index < binding_count);
        // A returned structural value is established by the ordinary operation
        // sequence before its return edge. Its operand calls belong to that
        // sequence too; excluding the tail statement loses their exact flow
        // occurrences when outer_calls reconstructs the constructor roots.
        let operation_end = if matches!(
            statements.get(terminator_index),
            Some(StatementNode::Expression(_))
        ) && facts
            .values
            .structural_values
            .root_at(state.symbol, u32::try_from(terminator_index).ok()?)
            .is_some()
            || matches!(statements.get(terminator_index), Some(StatementNode::Expression(expression))
                if matches!(program.expression_table.expression(*expression), ExpressionNode::Call(_)))
        {
            terminator_index.checked_add(1)?
        } else if matches!(
            result,
            checked_trees::CheckedControlResultPlan::Unit
                | checked_trees::CheckedControlResultPlan::Scalar { .. }
        ) && control::terminal_named_call(program, owner, state, terminator_index)
            .is_some()
        {
            // An unconditional terminal machine call is this state's last
            // operation (`terminal_calls`): the sequence plans it and counts
            // its occurrence, binding a scalar result as the state's returned
            // value, and the terminator completes after it.
            terminator_index.checked_add(1)?
        } else {
            terminator_index
        };
        let after_calls = source_calls.partition_point(|call| call.statement_index < operation_end);
        // Computation roots retain handles into this arena. Borrow the original
        // occurrences so their exact identity survives nested-call validation.
        trace.phase("state graph: outer calls");
        let calls = control::outer_calls_before_traced(
            program,
            facts,
            owner.symbol,
            state,
            &source_calls[first_call..after_calls],
            operation_end,
            trace,
        )?;
        // Selected operator and FMA applications are not yet routed to the
        // composed states, so the sequence sees none and rewrites no
        // parameter identity; the signature stays the one recorded above.
        let mut sequence_structural = structural.clone();
        let sequence = control::statement_sequence::build(
            program,
            facts,
            scalar_callees,
            shapes,
            owner,
            state,
            &mut sequence_structural,
            scalar,
            &state_entry_claims[state_index],
            &calls,
            &[],
            binding_count,
            call_frames,
            trace,
        )?;
        trace.phase("state graph: operation custody");
        let mut operations = sequence.operations;
        retire_guarded_reference_completion(
            facts,
            state,
            sequence.structural_result.as_ref(),
            &mut operations,
        )?;
        operation_custody::admit(program, state, &mut operations, trace)?;
        // A single-state body whose completion establishes a shared `&`
        // result — a whole view, a subslice, or a forwarded named view —
        // keeps the ordinary sequence's plan, which lowers that completion.
        // A graph for the same body would displace that plan (structural
        // results prefer the complete graph), and the composed route replays
        // only transition-tail `&mut` establishments. Multi-state bodies have
        // no ordinary plan to displace and keep this route.
        if states.len() < 2
            && sequence.structural_result.as_ref().is_some_and(|returned| {
                operations.iter().any(|operation| {
                    matches!(
                        (operation, &returned.source),
                        (
                            CheckedUnitEffectOperationPlan::EstablishReference { result, source },
                            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal,
                            },
                        ) if result.binding_ordinal == *binding_ordinal
                            && source.access == CheckedStructuralAccess::SharedBorrow
                    )
                })
            })
        {
            trace.phase("state graph: operation custody: single-state shared reference completion");
            return None;
        }
        let terminator = terminator::build(
            program,
            facts,
            scalar_callees,
            shapes,
            fused,
            state_index,
            state,
            &result,
            &signatures,
            &state_entry_claims[state_index],
            &sequence.structural_results,
            sequence.scalar_result.as_ref(),
            sequence.structural_result.as_ref(),
            sequence.next_scalar_binding,
            &mut operations,
            terminator_index,
            trace,
        )?;
        trace.phase("state graph: exit result locals");
        let disposable_locals = if matches!(
            terminator,
            CheckedComposedUnitControlTerminatorPlan::Jump { .. }
                | CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
                | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { .. }
                | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { .. }
                | CheckedComposedUnitControlTerminatorPlan::GuardedJumps { .. }
                | CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                | CheckedComposedUnitControlTerminatorPlan::ReturnUnit
                | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
        ) {
            // A ClosedSum subject's state-exit disposal is the case
            // dispatch's own consumption, already proved inside the
            // terminator build; exclude it from the result-local roster.
            let case_subject_local = match &terminator {
                CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } => {
                    match &subject.source {
                        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        } => operations
                            .iter()
                            .find_map(|operation| {
                                let (result, _) = operation.discardable_structural_result()?;
                                (result.binding_ordinal == *binding_ordinal)
                                    .then_some(result.statement_index)
                            })
                            .and_then(|statement_index| {
                                match statements.get(statement_index as usize) {
                                    Some(StatementNode::LocalData(local)) => Some(local.symbol),
                                    _ => None,
                                }
                            }),
                        _ => None,
                    }
                }
                _ => None,
            };
            crate::execution::terminal_cleanup::state_exit_result_locals(
                program,
                facts,
                owner,
                state,
                case_subject_local,
            )?
        } else {
            Vec::new()
        };
        result_custody::account(
            program,
            facts,
            owner,
            state,
            structural,
            &mut operations,
            &terminator,
            &disposable_locals,
            trace,
        )?;
        planned.push(CheckedComposedUnitControlStatePlan {
            state: state.symbol,
            structural_parameters: structural.clone(),
            scalar_parameters: scalar.clone(),
            erased_scalar_parameters:
                crate::execution::terminal_unit::types::erased_scalar_parameter_plans(
                    program, state,
                )?,
            erased_proof_parameters:
                crate::execution::terminal_unit::types::erased_proof_parameter_plans(
                    program, state,
                )?,
            requires: state_requires[state_index].clone(),
            entry_claims: state_entry_claims[state_index].clone(),
            bindings,
            binding_initializers,
            operations,
            terminator,
        });
    }
    // Persistent receivers keep their invocation place. Other structural
    // parameters retain explicit owned-value or borrowed-view edge custody.
    // Each guard marks its own phase so the omission roster names the
    // custody shape the route lacks, not just the phase. States outside the
    // entry successor closure never execute, so their custody rows cannot
    // disqualify the route — the shape check applies to live positions only.
    trace.phase("state graph: state signature: parameter custody shape");
    let Some(live_states) = CheckedComposedUnitControlStatePlan::live_mask(&planned) else {
        trace.phase("state graph: state signature: parameter custody shape: edge target missing");
        return None;
    };
    // A one-shot entry uses the invocation's original parameter declarations:
    // callers establish their exact qualifications. A reentered entry instead
    // becomes a block arrival and needs the same transport evidence as any
    // successor; do not grant its fresh arrival the invocation's assumptions.
    let invocation_only_entry = !planned
        .iter()
        .zip(&live_states)
        .filter(|(_, live)| **live)
        .flat_map(|(state, _)| state.successors())
        .any(|successor| successor.target_state == planned[0].state);
    for (state_index, &(_, state)) in fused.iter().enumerate() {
        if !live_states[state_index] {
            continue;
        }
        trace.state(u32::try_from(state_index).ok());
        for (parameter_index, parameter) in planned[state_index]
            .structural_parameters
            .iter()
            .enumerate()
        {
            let reference =
                || program.state_parameters(state)[parameter.position as usize].type_reference;
            if parameter.multiplicity != Multiplicity::Linear
                && !parameter.qualifications.is_empty()
                && !(state_index == 0 && invocation_only_entry)
                && !forwards_qualified_parameter(
                    shapes,
                    &planned,
                    &live_states,
                    state_index,
                    parameter_index,
                )
            {
                trace.phase(
                    "state graph: state signature: parameter custody shape: qualified non-linear parameter",
                );
                return None;
            }
            match (parameter.is_self, &parameter.access) {
                (
                    true,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow,
                ) => {}
                (true, _) => {
                    trace.phase(
                        "state graph: state signature: parameter custody shape: persistent receiver access",
                    );
                    return None;
                }
                (false, CheckedStructuralAccess::Owned) => {
                    // Primitive arrays and bounded byte owners carry their
                    // checked payload shape rather than a nominal identity.
                    // Edge custody still checks the exact source/destination;
                    // adding states must not remove the ordinary call carrier.
                    let owned_sequence = parameter.multiplicity == Multiplicity::Unrestricted
                        && !program.state_parameters(state)[parameter.position as usize].is_mutable
                        && (validation::is_closed_primitive_array_type(program, reference())
                            || (validation::bounded_byte_buffer_capacity(program, reference()).is_some()
                                && validation::has_plain_owned_contents_with_represented_constraints(
                                    program, reference(),
                                )));
                    if parameter.multiplicity != Multiplicity::Linear
                        && !owned_sequence
                        && !matches!(
                            program.type_reference_table.type_reference(reference()),
                            TypeReferenceNode::Named { .. }
                        )
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: owned non-linear unnamed type",
                        );
                        return None;
                    }
                    // A parameter carries an established value, not a fresh
                    // initializer. Exact edge custody retains represented byte
                    // predicates, including those below record/array children.
                    // A carrier of plain fields and shared views arrives as a
                    // whole move whose views keep their source loans' lineage:
                    // a descriptor copy rides any non-linear carrier, `copy`
                    // or affine, under the contents judgment the dispatch
                    // applies to the same carrier as a scrutinee subject.
                    // The one-shot entry names the caller's custody: its loan
                    // has no checked transfer to ride in on, so only the
                    // one-record-deep view shape the invocation can spell is
                    // admitted there — the wider contents family enters
                    // exclusively through established edge custody.
                    let view_carrier = if state_index == 0 && invocation_only_entry {
                        parameter.multiplicity == Multiplicity::Unrestricted
                            && validation::has_owned_or_shared_view_fields(program, reference())
                    } else {
                        validation::has_owned_or_shared_view_contents(program, reference())
                    };
                    if parameter.multiplicity != Multiplicity::Linear
                        && !owned_sequence
                        && !validation::has_plain_owned_contents_with_represented_constraints(
                            program,
                            reference(),
                        )
                        && !view_carrier
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: owned non-linear record contents",
                        );
                        return None;
                    }
                }
                (false, access) => {
                    if parameter.multiplicity != Multiplicity::Unrestricted {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: borrowed restricted parameter",
                        );
                        return None;
                    }
                    if !matches!(
                        access,
                        CheckedStructuralAccess::SharedBorrow
                            | CheckedStructuralAccess::MutableBorrow
                    ) {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: write-only borrow",
                        );
                        return None;
                    }
                    let reference = reference();
                    let borrowed_named_referent = matches!(
                        program.type_reference_table.type_reference(reference),
                        TypeReferenceNode::Reference { referee, .. }
                            if matches!(
                                program.type_reference_table.type_reference(*referee),
                                TypeReferenceNode::Named { .. }
                            )
                    );
                    // Fixed arrays have structural rather than nominal
                    // identity. Their registered shape and plain owned
                    // contents admit the same loan across a state edge as
                    // across an ordinary call; access and qualifications
                    // remain checked above and by each successor transfer.
                    let borrowed_array_referent = matches!(
                        program.type_reference_table.type_reference(reference),
                        TypeReferenceNode::Reference { referee, .. }
                            if matches!(
                                program.type_reference_table.type_reference(*referee),
                                TypeReferenceNode::FixedArray { .. }
                            ) && validation::has_plain_owned_contents_with_numeric_constraints(
                                program, *referee,
                            )
                    ) && matches!(
                        shapes
                            .types
                            .get(&parameter.type_identity)
                            .map(|plan| &plan.shape),
                        Some(checked_trees::CheckedUnitStructuralTypeShape::FixedArray { .. })
                    );
                    // A borrowed bounded owner (`&[u8; N] in Utf8`) names the
                    // owner's carrier; the reference is this parameter's loan.
                    if !borrowed_named_referent
                        && !borrowed_array_referent
                        && parameter.fused_service_erasure.is_none()
                        && !byte_sequence_carrier(program, reference, &[]).is_some_and(|carrier| {
                            carrier.is_borrowed()
                                || matches!(
                                    carrier,
                                    CheckedByteSequenceCarrier::BoundedOwned { .. }
                                )
                        })
                        && borrowed_slice_view_element(program, reference, &[]).is_none()
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: borrowed non-view carrier",
                        );
                        return None;
                    }
                }
            }
        }
    }
    // Retain only this machine's direct provider-field calls. Each ordinary
    // callee owns its own attachment requirements, even through a receiver loan.
    trace.phase("state graph: provider attachment requirements");
    trace.state(None);
    let provider_attachment_requirements = if let Some(identity) = &attachment {
        let flows = fused
            .iter()
            .zip(&planned)
            .map(|(&(owner, state), plan)| {
                let flow = state_flow(facts, owner.symbol, state.symbol)?;
                Some((
                    owner,
                    state,
                    facts.flow.control.calls.span_or_empty(flow.calls),
                    plan.operations.as_slice(),
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        checked_composed_provider_attachment_requirements(program, shapes, identity, &flows)?
    } else {
        Vec::new()
    };
    trace.phase("state graph: finish");
    let mut plan = composed_control::finish_state_graph(
        program,
        facts,
        machine,
        attachment,
        provider_attachment_requirements,
        planned,
    )?;
    plan.natural_ranks = natural_ranks;
    // A returned record holding references names the entry parameter each
    // reference leaf borrows from. Only the entry state's own completion can
    // name those parameters, so a reference record returned from any other
    // state has no sources to publish. A whole-reference result is itself
    // the only leaf: each returning state's own completion already spells
    // the exact parameter path its borrow loans — in machine parameter
    // positions — so the machine publishes the deduplicated candidate rows
    // from every independently checked return.
    let whole_reference = match &result {
        checked_trees::CheckedControlResultPlan::Structural(result) => shapes
            .types
            .get(&result.type_identity)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    checked_trees::CheckedUnitStructuralTypeShape::Reference { .. }
                )
            }),
        _ => false,
    };
    if whole_reference {
        trace.phase("state graph: reference result origins");
        plan.result_reference_sources = result_custody::reference_sources(&plan.states)?;
    } else if matches!(
        result,
        checked_trees::CheckedControlResultPlan::Structural(_)
    ) && super::reference_results::is_reference_record(program, states[0].return_type)
    {
        if is_reference(program, states[0].return_type) {
            let mut sources: Option<&[checked_trees::CheckedReferenceResultSourcePlan]> = None;
            for state_plan in &plan.states {
                let CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } =
                    &state_plan.terminator
                else {
                    continue;
                };
                match sources {
                    None => sources = Some(&returned.reference_sources),
                    Some(existing) if existing == returned.reference_sources => {}
                    Some(_) => return None,
                }
            }
            plan.result_reference_sources =
                sources.map(|sources| sources.to_vec()).unwrap_or_default();
        } else {
            if states.len() != 1 {
                return None;
            }
            plan.result_reference_sources =
                super::reference_results::returned_record_sources(program, &states[0])?;
        }
    }
    plan.result = result;
    Some(plan)
}

/// A sequence is shared by ordinary and composed planning, so it retains the
/// syntactic tail's reference completion. Guarded composed control owns every
/// return arm instead. Remove only the exact sequence result binding when its
/// source coordinate and ingress also occur in the guarded return roster;
/// ordinary and unconditional completions keep their producer unchanged.
fn retire_guarded_reference_completion(
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    returned: Option<&checked_trees::CheckedUnitStructuralReturnPlan>,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let Some(checked_trees::CheckedUnitStructuralReturnPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
        reference_sources,
        ..
    }) = returned
    else {
        return Some(());
    };
    let mut tails = facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    let Some(tail) = tails.next() else {
        return Some(());
    };
    if tails.next().is_some() {
        return None;
    }
    let arms = facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(tail.arms)?;
    let mut returned_statements = arms
        .iter()
        .map(|arm| &arm.destination)
        .chain(tail.fallback.iter())
        .map(|destination| match destination {
            checked_trees::CheckedScalarBranchDestination::Return {
                statement_ordinal, ..
            } => Some(*statement_ordinal),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    returned_statements.sort_unstable();
    returned_statements.dedup();
    let mut matches = operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            let CheckedUnitEffectOperationPlan::EstablishReference { result, source } = operation
            else {
                return None;
            };
            (result.binding_ordinal == *binding_ordinal
                && returned_statements.contains(&result.statement_index)
                && reference_sources
                    .iter()
                    .any(|candidate| candidate.path.is_empty() && candidate.source == *source))
            .then_some(index)
        });
    let Some(index) = matches.next() else {
        return Some(());
    };
    if matches.next().is_some() {
        return None;
    }
    operations.remove(index);
    Some(())
}

/// A returning exit disposes whole roots only: a partial affine drop or a
/// residual discard at the return has no Terminal return-cleanup row here.
#[allow(clippy::too_many_arguments)]
/// A shared loan freezes its referent. A mutable bounded byte owner instead
/// preserves its primitive byte predicates through independently checked stores.
/// Neither rule grants transport for arbitrary mutable qualified values.
/// An immutable owned byte parameter likewise forwards its established value
/// whole, with exactly the same primitive predicate roster on every arrival.
/// A successor's qualified parameter is admitted only when every live
/// edge into its state forwards a source parameter with exactly the same
/// carrier, access, multiplicity and qualifications, or, for a shared byte
/// view, establishes an authored literal under those qualifications: each
/// is then a byte-predicate domain without establishment routes, which
/// Terminal verification decides from the literal's octets at the edge.
/// The entry state keeps its invocation rule: a re-entered entry is a
/// separate arrival.
fn forwards_qualified_parameter(
    shapes: &ShapeCollector<'_>,
    planned: &[CheckedComposedUnitControlStatePlan],
    live: &[bool],
    target_index: usize,
    parameter_index: usize,
) -> bool {
    let Some(target) = planned.get(target_index) else {
        return false;
    };
    let Some(parameter) = target.structural_parameters.get(parameter_index) else {
        return false;
    };
    if target_index == 0
        || parameter.is_self
        || !(parameter.access == CheckedStructuralAccess::SharedBorrow
            || (matches!(
                parameter.access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::Owned
            ) && parameter.projected_qualifications.is_empty()
                && shapes
                    .types
                    .get(&parameter.type_identity)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                                CheckedByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                    })
                && parameter.qualifications.iter().all(|qualification| {
                    shapes.domains.iter().any(|domain| {
                        domain.domain == *qualification
                            && domain.carrier_type_identity == parameter.type_identity
                            && domain.byte_predicate.is_some()
                            && domain.establishment_routes.is_empty()
                    })
                })))
        || parameter.multiplicity != Multiplicity::Unrestricted
    {
        return false;
    }
    let literal_establishes_qualifications = parameter.access
        == CheckedStructuralAccess::SharedBorrow
        && parameter.projected_qualifications.is_empty()
        && shapes
            .types
            .get(&parameter.type_identity)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                        CheckedByteSequenceCarrier::BorrowedView { .. }
                    )
                )
            })
        && parameter.qualifications.iter().all(|qualification| {
            shapes.domains.iter().any(|domain| {
                domain.domain == *qualification
                    && domain.carrier_type_identity == parameter.type_identity
                    && domain.byte_predicate.is_some()
                    && domain.establishment_routes.is_empty()
            })
        });
    planned
        .iter()
        .zip(live)
        .filter(|(_, live)| **live)
        .all(|(source, _)| {
            source
                .successors()
                .into_iter()
                .filter(|successor| successor.target_state == target.state)
                .all(|successor| {
                    successor.transfers.iter().any(|transfer| {
                        transfer.target_parameter_index as usize == parameter_index
                            && match transfer.source {
                                checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral { .. } => {
                                    literal_establishes_qualifications
                                }
                                checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                                    source.structural_parameters.get(index as usize).is_some_and(|forwarded| {
                                        !forwarded.is_self
                                            && forwarded.type_identity == parameter.type_identity
                                            && forwarded.access == parameter.access
                                            && forwarded.multiplicity == parameter.multiplicity
                                            && forwarded.qualifications == parameter.qualifications
                                            && forwarded.projected_qualifications
                                                == parameter.projected_qualifications
                                    })
                                }
                                _ => false,
                            }
                    })
                })
        })
}

fn return_cleanup_is_whole(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
    shapes: &ShapeCollector<'_>,
) -> Option<()> {
    if facts.flow.ownership.permissions.iter().any(|(_, event)| {
        event.machine_symbol == machine.symbol
            && event.state_symbol == state.symbol
            && event.kind == PermissionEventKind::AffineDrop
            && !event.segments.is_empty()
    }) {
        return None;
    }
    let (_, residual_affine_discards, _) = return_unit_affine_discards(
        program,
        facts,
        machine.symbol,
        state.symbol,
        structural,
        program.state_parameters(state),
        operations,
        &[],
        &shapes.types,
    )?;
    residual_affine_discards.is_empty().then_some(())
}

/// The exact Boolean value one authored guard retains at its `Guard`
/// coordinate: the pure expression, or the unique computation root this
/// machine owns there (a selected comparison, a call, or any other checked
/// computation). The computation producer records a root only where no pure
/// expression exists, so the coordinate names at most one of the two forms.
fn retained_guard(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
) -> Option<checked_trees::CheckedCallScalarArgument> {
    let role = CheckedScalarExpressionRole::Guard;
    if let Some(expression) =
        facts
            .values
            .scalar_expressions
            .expression_at(state, statement_ordinal, role)
    {
        return Some(checked_trees::CheckedCallScalarArgument::Pure(
            expression.clone(),
        ));
    }
    let computations = &facts.values.scalar_computations;
    let root = computations.root_at(state, statement_ordinal, role)?;
    (root.machine == machine && computations.nodes.is_valid(root.root)).then_some(
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    )
}

fn guard_is_boolean(facts: &CheckFacts, guard: &checked_trees::CheckedCallScalarArgument) -> bool {
    match guard {
        checked_trees::CheckedCallScalarArgument::Pure(expression) => {
            expression.primitive_type() == Some(PrimitiveType::Bool)
        }
        checked_trees::CheckedCallScalarArgument::Computation(root) => {
            facts
                .values
                .scalar_computations
                .nodes
                .get(*root)
                .primitive_type
                == PrimitiveType::Bool
        }
    }
}

/// The branch-free scalar initializers ahead of ordinary statement sequencing.
/// A short-circuit value ends this prefix, even when it has no calls: the
/// sequence's scalar-local operation already owns its conditional evaluation.
/// Keep the remaining bindings in that sequence too, preserving source order
/// and their references to the completed Boolean value.
fn prefix_initializers(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    bindings: &[CheckedScalarBinding],
    trace: &control::LocalConstructionTrace,
) -> Option<Vec<CheckedScalarExpression>> {
    use checked_trees::CheckedScalarBindingDestination;

    let statements = program.statement_table.statements(state.statement_nodes);
    let mut immutable_ordinal = 0u32;
    let mut initializers = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let mark = |phase| {
            trace.phase(phase);
            trace.statement(Some(binding.statement_ordinal));
        };
        mark("state graph: prefix initializers: binding value");
        if binding.value != CheckedScalarBindingValue::Expression {
            return None;
        }
        mark("state graph: prefix initializers: binding destination");
        let (role, destination) = match binding.destination {
            CheckedScalarBindingDestination::Immutable => {
                let StatementNode::LocalData(local) =
                    statements.get(binding.statement_ordinal as usize)?
                else {
                    return None;
                };
                let role = CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: immutable_ordinal,
                };
                immutable_ordinal = immutable_ordinal.checked_add(1)?;
                (role, local.symbol)
            }
            CheckedScalarBindingDestination::StorageInitialize { symbol } => {
                (CheckedScalarExpressionRole::StorageInitializer, symbol)
            }
            CheckedScalarBindingDestination::StorageAssign { symbol } => {
                (CheckedScalarExpressionRole::AssignmentValue, symbol)
            }
        };
        mark("state graph: prefix initializers: binding statement");
        let expression = match statements.get(binding.statement_ordinal as usize)? {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Assignment(assignment) => assignment.value,
            _ => return None,
        };
        mark("state graph: prefix initializers: bound expression");
        let (custody, initializer) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            binding.statement_ordinal,
            role,
        )?;
        mark("state graph: prefix initializers: custody agreement");
        if custody.expression != expression || custody.destination != destination {
            return None;
        }
        mark("state graph: prefix initializers: initializer type");
        if crate::values::scalar_expression_type(initializer) != Some(binding.primitive_type) {
            return None;
        }
        mark("state graph: prefix initializers: short-circuit boolean");
        if matches!(initializer, CheckedScalarExpression::Boolean(boolean)
                if checked_boolean_contains_short_circuit(boolean))
        {
            break;
        }
        initializers.push(initializer.clone());
    }
    Some(initializers)
}

/// A call retained inside a value-returning terminator (`Guarded` arm
/// payloads, `ConditionalReturn`'s value arm) consumes its producers through
/// owned whole arguments the same way an in-sequence call does: the argument
/// names the exact result binding it moves into the call's custody.
fn terminator_call_consumes_result(
    operation: &CheckedUnitEffectOperationPlan,
    binding_ordinal: u32,
) -> bool {
    operation
        .call_structural_arguments()
        .iter()
        .any(|argument| {
            argument.source_structural_result_binding_ordinal() == Some(binding_ordinal)
                && argument.access == CheckedStructuralAccess::Owned
                && argument.path.is_empty()
        })
}

/// Whole owned call arguments consume affine and linear results. Accounting
/// requires one disposition, and lowering rejoins a moved local's exact
/// establishment and call-transfer receipts instead of state-exit cleanup.
/// An inline argument construction also belongs to its consuming call when
/// copyable; other unrestricted locals retain their reusable statement owner.
fn call_owns_result(producer: &CheckedUnitEffectOperationPlan) -> bool {
    match producer {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            operand_source,
            ..
        } => {
            result.multiplicity != Multiplicity::Unrestricted
                || matches!(
                    operand_source,
                    Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
                )
        }
        // Any call with a structural result, ordinary or boundary.
        CheckedUnitEffectOperationPlan::Call(call) => call
            .structural_result()
            .is_some_and(|result| result.multiplicity != Multiplicity::Unrestricted),
        _ => false,
    }
}

/// An available structural binding, independently of whether its producer
/// carries disposal debt. An actual automatic return discard retires it.
fn retained_structural_result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&checked_trees::CheckedUnitStructuralResultBindingPlan> {
    if operation
        .discardable_structural_result()
        .is_some_and(|(_, discard)| discard)
    {
        return None;
    }
    operation.structural_result_binding()
}

/// An operand-owned construction retained beside the call that consumes it:
/// `operand_source` names the consuming call's coordinate and formal
/// position, so its statement is the call's — any local declared on that
/// statement is a sibling, never this construction's binding. Such rows must
/// not stand in for a local's unique producer.
fn operand_owned_construction(operation: &CheckedUnitEffectOperationPlan) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            operand_source: Some(_),
            ..
        }
    )
}

/// The call sequencer already joined each operand to its loan or consumption.
/// A state edge does not change that call's argument custody. Boundary and
/// ordinary calls share this rule; their receipts and claim transfers remain
/// separate obligations at their respective consumers.
fn call_argument_admitted(
    argument: &CheckedUnitStructuralArgumentPlan,
    earlier: &[CheckedUnitEffectOperationPlan],
) -> bool {
    if argument.access != CheckedStructuralAccess::Owned {
        // Loan lifetime and restoration remain the sequencer's obligations;
        // borrowing does not consume a graph-owned structural result.
        return true;
    }
    let Some(ordinal) = argument.source_structural_result_binding_ordinal() else {
        // Parameters, checked local storage and inline operands retain their
        // sequencer custody. In particular an inline value is established at
        // the call, not by a fictitious earlier result. Only an explicit result
        // binding introduces the additional graph disposition checked below.
        return true;
    };
    let mut producers = earlier.iter().filter_map(|operation| {
        let result = operation.structural_result_binding()?;
        (result.binding_ordinal == ordinal).then_some((operation, result))
    });
    let Some((producer, result)) = producers.next() else {
        return false;
    };
    producers.next().is_none()
        && argument.path.is_empty()
        && result.type_identity == argument.type_identity
        && retained_structural_result(producer).is_some()
        // Copies leave the original for its existing state-exit accounting;
        // affine/linear transfers use the same consumption rule as that
        // accounting, which independently requires exactly one disposition.
        && (result.multiplicity == Multiplicity::Unrestricted || call_owns_result(producer))
}

/// Name the first unmet call requirement using the same argument admission
/// as the operation guard, then distinguish its unsupported custody.
fn call_custody_refusal(
    structural_arguments: &[CheckedUnitStructuralArgumentPlan],
    claim_transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    earlier: &[CheckedUnitEffectOperationPlan],
) -> &'static str {
    if !claim_transfers.is_empty() {
        return "state graph: operation custody: call claim transfers";
    }
    for argument in structural_arguments {
        if call_argument_admitted(argument, earlier) {
            continue;
        }
        if argument.access == CheckedStructuralAccess::Owned
            && let Some(ordinal) = argument.source_structural_result_binding_ordinal()
        {
            let producer = earlier.iter().rev().find_map(|operation| {
                operation
                    .structural_result_binding()
                    .filter(|result| result.binding_ordinal == ordinal)
            });
            return match producer.map(|result| result.multiplicity) {
                None => "state graph: operation custody: call owned result without a producer",
                Some(_) => "state graph: operation custody: call owned local result",
            };
        }
        return "state graph: operation custody: unit call arguments";
    }
    "state graph: operation custody: unit call arguments"
}

/// The per-state `self` retention set `build_traced` closes over.
///
/// Ordinary and composed signatures share one obligation: `self` stays a
/// structural parameter while the graph can name its place. A state
/// reading, storing, projecting, or lending receiver storage needs it
/// directly (`receiver_calls::uses_receiver_storage`); a receiver-rooted
/// borrow, a whole-`self` operand, or a receiver-rooted result needs it too
/// (`receiver_rooted_operand`), none of which an argument access or scalar
/// row can see. A call whose receiver roots at `self` keeps the operand
/// slot: callee signatures are still open while this graph builds, and
/// `receiver_calls::reconcile` can only insert the operand when this
/// signature kept a `self` place for it. The last clause flows retention
/// back along named edges — a `self`-keeping successor is fed from the
/// source's own place.
fn retained_receivers(
    program: &TypedTrees,
    facts: &CheckFacts,
    fused: &FusedStates<'_>,
) -> Vec<bool> {
    let mut retained = fused
        .iter()
        .map(|&(owner, state)| {
            crate::execution::terminal_unit::receiver_calls::uses_receiver_storage(
                program, facts, owner, state,
            ) || crate::execution::terminal_unit::receiver_calls::calls_self_receiver(
                program, facts, owner, state,
            ) || receiver_rooted_operand(program, facts, owner, state)
        })
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for (source_index, &(_, source)) in fused.iter().enumerate() {
            if retained[source_index] {
                continue;
            }
            let feeds_retained = program
                .statement_table
                .statements(source.statement_nodes)
                .iter()
                .any(|statement| {
                    let StatementNode::Transition(transition) = statement else {
                        return false;
                    };
                    [transition.target, transition.continuation]
                        .into_iter()
                        .filter(|target| {
                            program.statement_table.transition_target_is_valid(*target)
                        })
                        .any(|target| {
                            let TransitionTargetNode::Named { path, .. } =
                                program.statement_table.transition_target(target)
                            else {
                                return false;
                            };
                            fused_target_index(program, fused, path.symbol)
                                .is_some_and(|index| retained[index])
                        })
                });
            if feeds_retained {
                retained[source_index] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    retained
}

/// Whether a statement or successor operand in `state` names a place rooted
/// at `self`: a receiver-rooted borrow (`let view = &mut self.field`), a
/// whole-`self` operand (`f(self)`), a receiver loan, a root binding, or a
/// successor operand or result spelled through `self`. Each needs the
/// structural `self` place, yet none leaves the receiver-field argument
/// access or scalar read `uses_receiver_storage` watches for.
fn receiver_rooted_operand(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> bool {
    let receiver_roots = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_self)
        .map(|parameter| parameter.symbol)
        .chain(std::iter::once(machine.symbol))
        .collect::<Vec<_>>();
    // `uses_receiver_storage` already counts a lent receiver field by its
    // attached-data parent. The complementary shapes are a whole-`self`
    // operand and any receiver-rooted loan, either of which roots directly
    // at `self` or at an attached-data member.
    let attached = machine.attached_data_symbol;
    let receiver_rooted_symbol = |symbol: facts::PlaceRoot| {
        let facts::PlaceRoot::Symbol(symbol) = symbol else {
            return false;
        };
        receiver_roots.contains(&symbol)
            || (symbol.is_valid() && program.symbols.get(symbol).parent == attached)
    };
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|borrow| {
            borrow.machine_symbol == machine.symbol && borrow.state_symbol == state.symbol
        });
    if let Some(borrow_state) = borrow_state {
        if facts
            .borrow
            .calls
            .span_or_empty(borrow_state.calls)
            .iter()
            .flat_map(|call| facts.borrow.argument_accesses.span_or_empty(call.accesses))
            .any(|access| {
                access.root_symbol.is_valid() && receiver_roots.contains(&access.root_symbol)
            })
        {
            return true;
        }
        if facts
            .borrow
            .loans
            .span_or_empty(borrow_state.loans)
            .iter()
            .any(|loan| {
                receiver_roots.contains(&loan.root_symbol)
                    || receiver_roots.contains(&loan.owner_symbol)
                    || receiver_roots.contains(&loan.source_owner_symbol)
                    || (loan.root_symbol.is_valid()
                        && program.symbols.get(loan.root_symbol).parent == attached)
            })
        {
            return true;
        }
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .any(|(index, statement)| {
            let rooted = |expression: typed_trees::expression::ExpressionHandle| {
                expression.is_valid()
                    && crate::flow::canonical_place_from_expression_in_state(
                        program,
                        state.symbol,
                        index,
                        expression,
                    )
                    .is_some_and(|place| receiver_rooted_symbol(place.root))
            };
            match statement {
                StatementNode::RootBinding(binding) => {
                    rooted(binding.receiver) || rooted(binding.implementation_operand)
                }
                StatementNode::AssemblyFact(fact) => rooted(fact.expression),
                StatementNode::Call(call) => program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| rooted(*argument)),
                StatementNode::Assignment(assignment) => {
                    rooted(assignment.target) || rooted(assignment.value)
                }
                StatementNode::LocalData(local) => rooted(local.initial_value),
                StatementNode::Expression(expression) => rooted(*expression),
                StatementNode::Transition(transition) => {
                    [transition.target, transition.continuation]
                        .into_iter()
                        .filter(|target| {
                            program.statement_table.transition_target_is_valid(*target)
                        })
                        .any(
                            |target| match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Named { arguments, .. } => program
                                    .statement_table
                                    .expression_handles(*arguments)
                                    .iter()
                                    .any(|argument| rooted(*argument)),
                                TransitionTargetNode::Value(expression) => rooted(*expression),
                                _ => false,
                            },
                        )
                }
            }
        })
}

type Signature = (
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
);

/// The terminator family an edge is built for. The trace names the guard
/// that declined the edge beneath that family's phase; the edge's statement
/// ordinal separates the true and false successors of one conditional.
#[derive(Clone, Copy)]
pub(super) enum SuccessorEdge {
    Jump,
    Conditional,
    GuardedJump,
    ClosedCase,
}

#[derive(Clone, Copy)]
enum SuccessorGuard {
    TransitionForm,
    TargetState,
    ArgumentCount,
    StructuralArgument,
    ReceiverTransfer,
    SubsliceTransfer,
    LiteralTransfer,
    ResultTransfer,
    CasePayloadTransfer,
    EstablishedArgument,
    ProjectedCopy,
    LoanTransfer,
    ParameterTransfer,
    ScalarArguments,
    ErasedArguments,
    ErasedProofArguments,
    EdgeCleanup,
    EdgeCleanupProjected,
    EdgeCleanupStatePlan,
    EdgeCleanupRow,
    EdgeCleanupTarget,
}

impl SuccessorEdge {
    /// The trace phase naming `guard` on this successor edge. Every phase is
    /// `state graph: terminator: <edge>: <guard>`, spelled at compile time.
    fn phase(self, guard: SuccessorGuard) -> &'static str {
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
                    SuccessorGuard::ReceiverTransfer => guard_phase!($edge, "receiver transfer"),
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
                    SuccessorGuard::ProjectedCopy => guard_phase!($edge, "projected copy"),
                    SuccessorGuard::LoanTransfer => guard_phase!($edge, "loan transfer"),
                    SuccessorGuard::ParameterTransfer => guard_phase!($edge, "parameter transfer"),
                    SuccessorGuard::ScalarArguments => guard_phase!($edge, "scalar arguments"),
                    SuccessorGuard::ErasedArguments => guard_phase!($edge, "erased arguments"),
                    SuccessorGuard::ErasedProofArguments => {
                        guard_phase!($edge, "erased proof arguments")
                    }
                    SuccessorGuard::EdgeCleanup => guard_phase!($edge, "edge cleanup"),
                    SuccessorGuard::EdgeCleanupProjected => {
                        guard_phase!($edge, "edge cleanup: projected row shadows")
                    }
                    SuccessorGuard::EdgeCleanupStatePlan => {
                        guard_phase!($edge, "edge cleanup: no state plan")
                    }
                    SuccessorGuard::EdgeCleanupRow => {
                        guard_phase!($edge, "edge cleanup: no edge row")
                    }
                    SuccessorGuard::EdgeCleanupTarget => {
                        guard_phase!($edge, "edge cleanup: target mismatch")
                    }
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
pub(super) struct EdgeCalls<'a> {
    pub(super) scalar_callees: ScalarCalleePlans<'a>,
    pub(super) entry_claims: &'a [checked_trees::CheckedUnitEntryClaimPlan],
    pub(super) local_results: &'a [(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
}

fn successor(
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
    let cleanup = if cleanups
        .for_projected_edge(machine.symbol, source.symbol, ordinal)
        .is_some()
    {
        trace.phase(edge.phase(SuccessorGuard::EdgeCleanupProjected));
        return None;
    } else if cleanups.for_state(machine.symbol, source.symbol).is_none() {
        trace.phase(edge.phase(SuccessorGuard::EdgeCleanupStatePlan));
        return None;
    } else {
        match cleanups.for_edge(machine.symbol, source.symbol, ordinal) {
            Some(cleanup) => cleanup,
            None => {
                trace.phase(edge.phase(SuccessorGuard::EdgeCleanupRow));
                return None;
            }
        }
    };
    if cleanup.target_state != successor.target_state {
        trace.phase(edge.phase(SuccessorGuard::EdgeCleanupTarget));
        return None;
    }
    // An owned parameter the target does not receive dies on this edge. The
    // cleanup evidence names exactly those positions; the edge carries them
    // so lowering disposes each where control leaves the state.
    let mut successor = successor;
    successor.trivial_affine_discard_parameter_positions =
        cleanup.trivial_affine_discard_parameter_positions.clone();
    Some(successor)
}

// Operand identity is shared by ordinary and closed-case edges. The caller
// separately admits either whole-parameter cleanup or exact result-local
// cleanup; constructing bindings establishes neither cleanup contract.
#[allow(clippy::too_many_arguments)]
fn successor_bindings(
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
    if arguments.len()
        != target_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
    {
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
    let argument_at = |position: u32| {
        let position = target_parameters
            .iter()
            .take(position as usize)
            .filter(|parameter| !parameter.is_self)
            .count();
        arguments.get(position).copied()
    };
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
            if target.is_self {
                mark(SuccessorGuard::ReceiverTransfer);
                let Some(source_index) = source_structural
                    .iter()
                    .position(|parameter| parameter.is_self)
                else {
                    // A source state that never declared `self` still rides
                    // the machine's ambient receiver: the edge forwards the
                    // entry receiver descriptor rather than a source
                    // parameter, under the same access narrowing a carried
                    // `self` would take.
                    let mut receiver = signatures
                        .first()?
                        .0
                        .iter()
                        .find(|parameter| parameter.is_self)?
                        .clone();
                    if receiver.access == CheckedStructuralAccess::MutableBorrow
                        && target.access == CheckedStructuralAccess::SharedBorrow
                    {
                        receiver.access = CheckedStructuralAccess::SharedBorrow;
                    }
                    if receiver != *target {
                        return None;
                    }
                    return Some(CheckedStructuralControlTransferPlan {
                        source:
                            checked_trees::CheckedStructuralControlTransferSourcePlan::MachineReceiver,
                        target_parameter_index: u32::try_from(target_index).ok()?,
                    });
                };
                // The receiver keeps its invocation place, but an edge may
                // restrict mutable access to a shared state view. Every other
                // part of its custody remains exact; the reverse gains authority.
                let mut transferred = source_structural[source_index].clone();
                if transferred.access == CheckedStructuralAccess::MutableBorrow
                    && target.access == CheckedStructuralAccess::SharedBorrow
                {
                    transferred.access = CheckedStructuralAccess::SharedBorrow;
                }
                if transferred != *target {
                    return None;
                }
                return Some(CheckedStructuralControlTransferPlan {
                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                        index: u32::try_from(source_index).ok()?,
                    },
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            mark(SuccessorGuard::StructuralArgument);
            let expression = argument_at(target.position)?;
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
                if target.is_self
                    || target.access != CheckedStructuralAccess::SharedBorrow
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
            let mutable_local = source_index == 0
                && matches!(edge, SuccessorEdge::Jump)
                && target.access == CheckedStructuralAccess::MutableBorrow
                && target.multiplicity == Multiplicity::Unrestricted
                && matches!(program.expression_table.expression(expression), ExpressionNode::Borrow(borrow)
                    if borrow.access == language_core::ReferenceAccess::Mutable);
            if target.access == CheckedStructuralAccess::Owned || shared_view || mutable_view || mutable_local {
                mark(SuccessorGuard::ResultTransfer);
                let place = crate::flow::canonical_place_from_expression_in_state(program, source.symbol, ordinal as usize, expression)?;
                if place.segments.is_empty() {
                    let mut matches = operations.iter()
                        .filter(|operation| !operand_owned_construction(operation))
                        .filter_map(retained_structural_result)
                        .filter(|result| result.statement_index < ordinal && matches!(program.statement_table.statements(source.statement_nodes).get(result.statement_index as usize), Some(StatementNode::LocalData(local)) if place.root == facts::PlaceRoot::Symbol(local.symbol)));
                    if let Some(result) = matches.next() {
                        if matches.next().is_some() { return None; }
                        if result.type_identity != target.type_identity
                            || (!mutable_local && !mutable_view && result.multiplicity != target.multiplicity) { return None; }
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
                        if mutable_local {
                            let StatementNode::LocalData(local) = program.statement_table.statements(source.statement_nodes).get(result.statement_index as usize)? else { return None; };
                            let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else { return None; };
                            if (!local.is_mutable && local.initial_value.is_valid())
                                || !matches!(program.expression_table.expression(borrow.target), ExpressionNode::Name(name)
                                    if name.symbol == local.symbol && name.head_symbol == local.symbol
                                        && program.expression_table.name_path_members(name.members).len() == 1)
                                || !matches!(result.multiplicity, Multiplicity::Affine | Multiplicity::Unrestricted)
                                || !validation::has_plain_owned_contents_with_numeric_constraints(program, local.type_reference)
                                || super::types::type_graph_requires_nominal_drop(program, local.type_reference)
                            { return None; }
                        }
                        return Some(CheckedStructuralControlTransferPlan {
                            source: checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal: result.binding_ordinal },
                            target_parameter_index: u32::try_from(target_index).ok()?,
                        });
                    }
                }
            }
            if let Some((subject_place, selected_case)) = &case_test {
                mark(SuccessorGuard::CasePayloadTransfer);
                let place = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    source.symbol,
                    ordinal as usize,
                    expression,
                )?;
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
            if target.access == CheckedStructuralAccess::Owned
                && let Some(root) = facts.values.structural_values.root_for_expression(
                    source.symbol,
                    ordinal,
                    expression,
                )
                && root.machine == machine.symbol
                && program.normalized_type_identity(root.type_reference).into_string()
                    == target.type_identity
            {
                // Edge-local bindings number after the state's own results,
                // so no source result ordinal is reused on this edge.
                // Scratch includes the predecessor's result producers so the
                // common planner can resolve and consume local operands. Its
                // changed discard flags never escape into sibling edges;
                // selected-edge cleanup owns those predecessor moves.
                let mut operand_calls = operations.to_vec();
                let mut result_count = returns::next_result_ordinal(operations)?;
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
                    multiplicity: target.multiplicity,
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
            mark(SuccessorGuard::ProjectedCopy);
            // `parameter.field` supplied to an unrestricted parameter of
            // the leaf's own declared access copies that subtree out of a
            // live source parameter into fresh storage the target binds;
            // the source keeps its whole custody. A view-typed leaf's copy
            // forwards the descriptor — the view's loan keeps its lineage.
            if target.multiplicity == Multiplicity::Unrestricted
                && let Some(subject) = projected_copy_subject(
                    program,
                    machine,
                    source,
                    source_structural,
                    ordinal,
                    expression,
                    target,
                )
                && subject.access == target.access
            {
                return Some(CheckedStructuralControlTransferPlan {
                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedCopy {
                        subject,
                    },
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            mark(SuccessorGuard::LoanTransfer);
            // An authored `&place` / `&mut place` argument loans the named
            // subtree into the borrowed target parameter: the successor
            // argument keeps the source's rooted place and walks the
            // projection path below it, so the loan names the same storage
            // the borrow does rather than a fresh establishment. This arm
            // must run before the positional parameter transfer: a borrowed
            // argument that happens to share its target's type and access
            // would otherwise forward whatever parameter occupies the
            // matching position instead of the storage the borrow names.
            if target.multiplicity == Multiplicity::Unrestricted
                && let ExpressionNode::Borrow(borrow) =
                    program.expression_table.expression(expression)
            {
                let expected_access = match borrow.access {
                    language_core::ReferenceAccess::Shared => {
                        CheckedStructuralAccess::SharedBorrow
                    }
                    language_core::ReferenceAccess::Mutable => {
                        CheckedStructuralAccess::MutableBorrow
                    }
                    language_core::ReferenceAccess::WriteOnly => return None,
                };
                if target.access == expected_access
                    && let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                        program,
                        source.symbol,
                        ordinal as usize,
                        borrow.target,
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
                                super::types::parameter_root_symbol(machine.symbol, parameter)
                                    == root_symbol
                                    || parameter.symbol == root_symbol
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
                    let mutable_loan_permitted = match expected_access {
                        CheckedStructuralAccess::MutableBorrow => {
                            |root_access| {
                                matches!(
                                    root_access,
                                    CheckedStructuralAccess::Owned
                                        | CheckedStructuralAccess::MutableBorrow
                                )
                            }
                        }
                        _ => |_| true,
                    };
                    if let Some((source_index, source_parameter)) = parameter_subject
                    {
                        if mutable_loan_permitted(source_parameter.access)
                            && source_parameter.access != CheckedStructuralAccess::WriteOnlyBorrow
                            && let Some((projected_type, path)) =
                                calls::projected_borrowed_path(
                                    program,
                                    machine.symbol,
                                    source.symbol,
                                    ordinal as usize,
                                    &place,
                                )
                            // A projected loan keeps its original storage
                            // root and exact path across the edge. Case steps
                            // use this same route, including deeper payload
                            // fields; receiving verification must prove each
                            // selected case active before rebinding the loan.
                            // Reference-bearing storage still needs its own
                            // carrier custody and is not admitted here.
                            && path.iter().all(|segment| {
                                matches!(
                                    segment,
                                    CheckedUnitStructuralPathSegment::Field(_)
                                        | CheckedUnitStructuralPathSegment::Case(_)
                                        | CheckedUnitStructuralPathSegment::FixedIndex(_)
                                )
                            })
                            && calls::argument_presentation::is_admitted(
                                program,
                                projected_type,
                                target_parameters.get(target.position as usize)?.type_reference,
                                &target.type_identity,
                                &[],
                            )
                            && {
                                // Owned roots lend only the projected subtree.
                                // Borrowed roots retain their existing referent
                                // custody: inspect that storage, not the outer
                                // reference shell representing the parameter's
                                // loan. References stored inside the referent
                                // still make it ineligible for this transfer.
                                let identity = match source_parameter.access {
                                    CheckedStructuralAccess::SharedBorrow
                                    | CheckedStructuralAccess::MutableBorrow => {
                                        source_parameter.type_identity.clone()
                                    }
                                    _ => program
                                        .normalized_type_identity(projected_type)
                                        .into_string(),
                                };
                                !declared_contains_reference(shapes, &identity)
                            }
                        {
                            let carrier_identity = if path.is_empty() {
                                String::new()
                            } else {
                                shapes.add_reference_type(
                                    target_parameters
                                        .get(target.position as usize)?
                                        .type_reference,
                                    &[],
                                )?
                            };
                            return Some(CheckedStructuralControlTransferPlan {
                                source: checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                                    subject: CheckedUnitStructuralArgumentPlan {
                                        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                            parameter_index: u32::try_from(source_index).ok()?,
                                        },
                                        path,
                                        type_identity: target.type_identity.clone(),
                                        access: expected_access,
                                    },
                                    carrier_identity,
                                },
                                target_parameter_index: u32::try_from(target_index).ok()?,
                            });
                        }
                    } else if let Some((local, result)) = local_subject() {
                        let mutable_local_loan = expected_access
                            != CheckedStructuralAccess::MutableBorrow
                            || (local.is_mutable
                                && matches!(
                                    result.multiplicity,
                                    Multiplicity::Affine | Multiplicity::Unrestricted
                                )
                                && validation::has_plain_owned_contents_with_numeric_constraints(
                                    program,
                                    local.type_reference,
                                )
                                && !super::types::type_graph_requires_nominal_drop(
                                    program,
                                    local.type_reference,
                                ));
                        if mutable_local_loan
                            && let Some((projected_type, path)) =
                                calls::projected_borrowed_path(
                                    program,
                                    machine.symbol,
                                    source.symbol,
                                    ordinal as usize,
                                    &place,
                                )
                            && path.iter().all(|segment| {
                                matches!(
                                    segment,
                                    CheckedUnitStructuralPathSegment::Field(_)
                                        | CheckedUnitStructuralPathSegment::Case(_)
                                        | CheckedUnitStructuralPathSegment::FixedIndex(_)
                                )
                            })
                            // A state argument has the same presentation as a
                            // call argument. Fixed byte references retain a
                            // reference identity while their local backing is
                            // an array; normalized-string equality incorrectly
                            // rejected that ordinary loan. Access, live-home
                            // and reference-content checks remain independent.
                            && calls::argument_presentation::is_admitted(
                                program,
                                projected_type,
                                target_parameters.get(target.position as usize)?.type_reference,
                                &target.type_identity,
                                &[],
                            )
                            && !declared_contains_reference(
                                shapes,
                                &program
                                    .normalized_type_identity(projected_type)
                                    .into_string(),
                            )
                        {
                            let carrier_identity = if path.is_empty() {
                                String::new()
                            } else {
                                shapes.add_reference_type(
                                    target_parameters
                                        .get(target.position as usize)?
                                        .type_reference,
                                    &[],
                                )?
                            };
                            return Some(CheckedStructuralControlTransferPlan {
                                source: checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                                    subject: CheckedUnitStructuralArgumentPlan {
                                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                            binding_ordinal: result.binding_ordinal,
                                        },
                                        path,
                                        type_identity: target.type_identity.clone(),
                                        access: expected_access,
                                    },
                                    carrier_identity,
                                },
                                target_parameter_index: u32::try_from(target_index).ok()?,
                            });
                        }
                    }
                }
            }
            mark(SuccessorGuard::ParameterTransfer);
            let source_position = source_position(target.position)?;
            let source_index = source_structural
                .iter()
                .position(|parameter| parameter.position as usize == source_position)?;
            let source = &source_structural[source_index];
            // A state edge transfers the same loan into its successor. It may
            // restrict exclusive access to observation, never restore mutation.
            let access_matches = source.access == target.access
                || (source.access == CheckedStructuralAccess::MutableBorrow
                    && target.access == CheckedStructuralAccess::SharedBorrow);
            if source.type_identity != target.type_identity || !access_matches {
                return None;
            }
            Some(CheckedStructuralControlTransferPlan {
                source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                    index: u32::try_from(source_index).ok()?,
                },
                target_parameter_index: u32::try_from(target_index).ok()?,
            })
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

/// The rooted place an owned `parameter.field` argument copies from.
///
/// The argument must name a member projection below one readable source
/// parameter, and the projected subtree must be a whole unrestricted
/// plain-owned value of exactly the target's type: copying it observes the
/// source without moving any custody out of it. A local root, an indexed
/// projection or a projection reaching borrowed storage keeps its own route.
fn projected_copy_subject(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    source_structural: &[CheckedUnitStructuralParameterPlan],
    ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
    target: &CheckedUnitStructuralParameterPlan,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_)
    ) {
        return None;
    }
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
    let (position, _) =
        program
            .state_parameters(source)
            .iter()
            .enumerate()
            .find(|(_, parameter)| {
                super::types::parameter_root_symbol(machine.symbol, parameter) == root_symbol
                    || parameter.symbol == root_symbol
            })?;
    let (parameter_index, parameter) = source_structural
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.position as usize == position)?;
    if parameter.access == CheckedStructuralAccess::WriteOnlyBorrow {
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
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
        || program.type_multiplicity(projected_type) != Multiplicity::Unrestricted
        // Represented field predicates remain in the copied declaration.
        // A `[copy]` carrier of owned fields and shared views copies its
        // view descriptors whole — and a leaf that is itself a view
        // forwards its descriptor the same way: each copied view keeps its
        // source loan's lineage, and copy permission neither establishes
        // a field's predicates nor extends the loan.
        || !(validation::has_plain_owned_contents_with_represented_constraints(program, projected_type)
            || validation::has_owned_or_shared_view_contents(
                program,
                projected_type,
            ))
        || {
            // A borrowed target names its referent, so the leaf matches
            // the target when the leaf's own referent does.
            let identity = match validation::reference_shell(program, projected_type) {
                Some(shell) => program.normalized_type_identity(shell.referee).into_string(),
                None => program
                    .normalized_type_identity(projected_type)
                    .into_string(),
            };
            identity != target.type_identity
        }
    {
        return None;
    }
    let access = super::types::structural_access_for_type_reference(program, projected_type)?;
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        path,
        type_identity: target.type_identity.clone(),
        access,
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
    let normalized = |reference| program.normalized_type_identity(reference).into_string();
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
        (validation::has_plain_owned_contents_with_numeric_constraints(
            program,
            field.type_reference,
        ) || validation::has_owned_or_shared_view_contents(program, field.type_reference))
            && normalized(field.type_reference) == target.type_identity
    } else {
        target.access == CheckedStructuralAccess::SharedBorrow
            && super::types::structural_access_for_type_reference(program, field.type_reference)
                == Some(CheckedStructuralAccess::SharedBorrow)
            && shared_referee.is_some_and(|referee| normalized(referee) == target.type_identity)
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
            super::types::terminal_field_identity(program, *symbol)
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
        let mut leaf_type = subject_type;
        while let TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } = program.type_reference_table.type_reference(leaf_type)
        {
            leaf_type = *base_type;
        }
        normalized(leaf_type)
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
