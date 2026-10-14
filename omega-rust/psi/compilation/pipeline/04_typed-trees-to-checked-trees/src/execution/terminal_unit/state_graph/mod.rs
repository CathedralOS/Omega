//! State-local calls and explicit successor bindings, independent of graph shape.
//!
//! `build_traced` plans a state machine as a composed Unit control
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
    CheckedGuardedJumpPlan, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, ExpressionNode, Multiplicity, PermissionEventKind,
    PermissionEventSource, StatementNode, TransitionExit, TransitionGuardNode,
    TransitionTargetNode, TypeReferenceNode,
};
use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::calls::{
    entry_claims, free_structural_scalar_signature_traced, structural_scalar_signature_traced,
};
use crate::execution::terminal_unit::providers::checked_composed_provider_attachment_requirements;
use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders, state_flow};
use crate::execution::terminal_unit::types::{borrowed_slice_view_element, byte_sequence_carrier};
use crate::execution::terminal_unit::{composed_control, control};

mod closed_sum;
mod fused;
mod guards;
mod local_results;
mod operation_custody;
mod result_custody;
pub(super) mod returns;
mod successors;
mod terminal_calls;
mod terminator;

use fused::fused_states;
pub(super) use fused::{FusedStates, fused_target_index, terminator_statement_index};
use guards::checked_qualified_arrivals;
pub(super) use guards::{guard_is_boolean, retained_guard};
pub(super) use result_custody::retained_structural_result;
pub(super) use successors::{
    EdgeCalls, SuccessorEdge, SuccessorGuard, successor, successor_bindings,
};

/// The result a state graph describes for `machine`, or `None` when the
/// machine's result is outside the graph's result vocabulary. Body routing asks
/// the same question before any builder runs.
pub(super) fn result_plan(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<checked_trees::CheckedControlResultPlan> {
    let entry = program.machine_states(machine).first()?;
    let result = returns::signature(program, shapes, entry.return_type)?;
    crate::execution::terminal_unit::types::register_authored_result_domains(
        program,
        shapes,
        machine,
        entry,
        &crate::execution::terminal_unit::types::machine_binders(program, machine),
    )?;
    // Parameter qualifications are transported exactly. Additional authored
    // contracts on a linear result still need their own retained proof
    // obligations.
    (!(matches!(&result, checked_trees::CheckedControlResultPlan::Structural(result)
        if result.multiplicity == Multiplicity::Linear)
        && !program.machine_contracts(machine).is_empty()))
    .then_some(result)
}

/// Read one retained source observation roster for the authored owners in this
/// composed graph. Fusion changes control placement, never the chosen measure
/// coordinate or the source judgment. Weak call-component edges come from its
/// same retained complete decision.
pub(crate) fn natural_rank_plans(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> Option<(
    Vec<checked_trees::CheckedStateNaturalRank>,
    Vec<checked_trees::CheckedRankEdgeCoordinate>,
)> {
    let fused = fused_states(program, machine);
    let members = fused
        .iter()
        .map(|(_, state)| state.symbol)
        .collect::<Vec<_>>();
    let mut ranks = Vec::new();
    for (owner, state) in &fused {
        if owner.termination_plan.implementation_witness.is_none() {
            continue;
        }
        let judgment = admitted.judgment_for_body(owner.symbol, owner.states)?;
        if !judgment.local_terminates {
            return None;
        }
        let observations = admitted.observations.span(judgment.observations)?;
        let relevance = validation::rank_relevance(program, &[owner.symbol])?;
        if relevance.requires_rank(state.symbol)
            && !observations.iter().any(|rank| rank.state == state.symbol)
        {
            return None;
        }
        ranks.extend(
            observations
                .iter()
                .filter(|rank| rank.state == state.symbol)
                .cloned(),
        );
    }
    ranks.sort_by_key(|rank| (rank.state.arena_index(), rank.state.generation()));
    for same_state in ranks.windows(2) {
        if same_state[0].state == same_state[1].state && same_state[0] != same_state[1] {
            return None;
        }
    }
    ranks.dedup();
    let mut weak = Vec::new();
    for component in validation::retained_runtime_natural_components(admitted) {
        if !component
            .states
            .iter()
            .any(|rank| members.contains(&rank.state))
        {
            continue;
        }
        if !component
            .states
            .iter()
            .all(|rank| members.contains(&rank.state))
            || !component
                .edges
                .iter()
                .all(|edge| members.contains(&edge.source) && members.contains(&edge.target))
        {
            return None;
        }
        // Every projected component state keeps the exact source observation
        // which its accepted coordinate witness produced.
        if component.states.iter().any(|state| {
            !ranks.iter().any(|rank| {
                rank.state == state.state
                    && rank.parameter == state.parameter
                    && rank.parameter_position == state.parameter_position
            })
        }) {
            return None;
        }
        weak.extend(
            component
                .edges
                .iter()
                .filter(|edge| !edge.strict)
                .map(|edge| checked_trees::CheckedRankEdgeCoordinate {
                    source_state: edge.source,
                    target_state: edge.target,
                    statement_ordinal: edge.statement_ordinal,
                }),
        );
    }
    weak.sort_by_key(|edge| {
        (
            edge.source_state.arena_index(),
            edge.source_state.generation(),
            edge.statement_ordinal,
            edge.target_state.arena_index(),
            edge.target_state.generation(),
        )
    });
    weak.dedup();
    Some((ranks, weak))
}

/// Plans explicit state signatures and transitions, tracing the first declined operation.
pub(super) fn build_traced(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let fused_states = fused_states(program, machine);
    let fused = &fused_states;
    trace.phase("state graph: shape");
    trace.state(None);
    let states = program.machine_states(machine);
    if states.is_empty() || !machine_binders(program, machine).is_empty() {
        return None;
    }
    trace.phase("state graph: result signature");
    let result = result_plan(program, shapes, machine)?;
    // State count and the presence of an aggregate initializer do not choose
    // cleanup semantics. Even one Unit state can owe executable normal-exit
    // actions. Plan its graph here; the shared candidate selection keeps one
    // body when the ordinary sequence also supplies a complete implementation.
    if let checked_trees::CheckedControlResultPlan::Scalar { .. } = result {
        // This Unit body is the one lowering of a scalar-result state graph.
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
    let (mut natural_ranks, preserving_rank_edges) =
        natural_rank_plans(program, machine, &facts.termination.rank_arrivals)?;
    for rank in &mut natural_ranks {
        let (owner, _) = validation::find_state(program, rank.state)?;
        let root = facts.values.scalar_computations.root_at(
            rank.state,
            0,
            checked_trees::CheckedScalarExpressionRole::RankObservation,
        )?;
        if root.machine != owner.symbol {
            return None;
        }
        rank.observation = root.root;
    }
    let mut attachment = None;
    let mut signatures = Vec::new();
    let mut state_entry_claims = Vec::new();
    let mut state_requires = Vec::new();
    for &(owner, state) in fused.iter() {
        trace.phase("state graph: state signature: result and contracts");
        trace.state(Some(state.symbol));
        if returns::signature(program, shapes, state.return_type)? != result {
            trace.phase("state graph: state signature: result differs");
            return None;
        }
        // Membership contracts still restate the exact declared parameter
        // qualifications. Closed requires retain their runtime projections;
        // other authored expressions retain their source identity. Emission
        // consumes them only through complete occurrence-bound arrival
        // decisions, not by independently proving them again or replacing
        // them with a scalar tautology. Declaration projections remain required.
        let Some(contract_predicates) =
            validation::structural_state_contract_scalar_predicates(program, state)
        else {
            trace.phase("state graph: state signature: contract shape unadmitted");
            return None;
        };
        let mut requires = Vec::with_capacity(contract_predicates.len());
        for row in contract_predicates {
            let source_expression = match &row {
                validation::StateScalarContractRow::Expression(expression) => Some(*expression),
                _ => None,
            };
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
                validation::StateScalarContractRow::DeclaredRange {
                    parameter,
                    minimum,
                    maximum,
                    end_inclusive,
                } => crate::values::lower_state_declared_range_predicate(
                    program,
                    &facts.operators,
                    owner,
                    state,
                    parameter,
                    minimum,
                    maximum,
                    end_inclusive,
                ),
                validation::StateScalarContractRow::Domain {
                    parameter,
                    constraint,
                } => crate::values::lower_state_domain_predicate(
                    program,
                    state,
                    parameter,
                    &constraint,
                ),
                validation::StateScalarContractRow::Finite { parameter } => {
                    validation::state_finite_predicate(program, state, parameter)
                }
            };
            let requirement = match (predicate, source_expression) {
                (Some(predicate), _) => checked_trees::CheckedStateContractRequirement::Closed(
                    checked_trees::ClosedScalarContractValue::Predicate(predicate),
                ),
                (None, Some(expression)) => {
                    checked_trees::CheckedStateContractRequirement::SourceExpression(expression)
                }
                (None, None) => {
                    trace.phase(
                        "state graph: state signature: declaration predicate lowering failed",
                    );
                    return None;
                }
            };
            requires.push(requirement);
        }
        // An authored state transition closes the declared-domain window of
        // each readable argument. These are destination arrival obligations,
        // not facts inherited from the machine's first invocation or a loop's
        // previous iteration. Synthetic expression continuations are not
        // authored state boundaries and receive no such rows here.
        requires.extend(
            validation::state_declaration_predicates(program, &facts.operators, state)
                .into_iter()
                .map(|row| {
                    checked_trees::CheckedStateContractRequirement::Closed(
                        checked_trees::ClosedScalarContractValue::Predicate(row.predicate),
                    )
                }),
        );
        state_requires.push(requires);
        trace.phase("state graph: state signature: parameter signature");
        let (structural, scalar) = if owner.attached_data.is_some() {
            let (identity, structural, scalar) =
                structural_scalar_signature_traced(program, shapes, owner, state, &[], trace)?;
            // One graph can publish only one attachment identity. Static
            // qualification contributes no attachment storage.
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
        trace.state(Some(state.symbol));
        let (structural, scalar) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        let terminator_index = terminator_statement_index(statements, &result);
        let terminator_statement_ordinal = u32::try_from(terminator_index).ok()?;
        trace.phase("state graph: state flow");
        let flow = state_flow(facts, owner.symbol, state.symbol)?;
        let source_calls = facts.flow.control.calls.span_or_empty(flow.calls);
        if source_calls
            .windows(2)
            .any(|pair| pair[0].statement_index > pair[1].statement_index)
        {
            return None;
        }
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
        } else if control::terminal_named_call(program, owner, state, terminator_index)
            .is_some_and(|target| fused_target_index(program, &fused, target).is_none())
        {
            // An unconditional terminal machine call is this state's last
            // operation (`terminal_calls`), whatever the machine's result: the
            // sequence plans it and counts its occurrence, binding a value
            // result as the state's returned value, and the terminator
            // completes after it.
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
            &source_calls[..after_calls],
            operation_end,
            trace,
        )?;
        // Selected operator and FMA applications are not yet routed to the
        // composed states, so the sequence sees none and rewrites no
        // parameter identity; the signature stays the one recorded above.
        let mut sequence_structural = structural.clone();
        trace.phase("state graph: parameter storage");
        // Composed graphs admit no machine binders (see the entry guard).
        let parameter_storage = crate::execution::terminal_unit::types::parameter_storage_plans(
            program,
            shapes,
            state,
            &[],
            scalar,
        )?;
        let sequence = control::statement_sequence::build(
            program,
            facts,
            scalar_callees,
            shapes,
            owner,
            state,
            &mut sequence_structural,
            scalar,
            &parameter_storage,
            &state_entry_claims[state_index],
            &calls,
            &[],
            0,
            operation_end,
            call_frames,
            trace,
        )?;
        trace.phase("state graph: operation custody");
        let mut operations = sequence.operations;
        let place_loans = sequence.place_loans;
        operation_custody::admit(program, state, &mut operations, trace)?;
        // Completion custody, not state count or borrow spelling, determines
        // the returned value. The receiving graph rejoins its exact producer
        // and independently validates the source reference's access and path.
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
        // The prefix still owes its exact holes. A normal successor must
        // receive established storage; an all-crash roster returns nothing
        // and owns no repair or cleanup successor. Guarded crash spelling
        // does not exempt a returning sibling.
        trace.phase("state graph: borrowed windows at selected exits");
        if !sequence.windows.is_closed()
            && !terminator.abandons_custody(&facts.flow.guarded_exits)?
        {
            return None;
        }
        result_custody::account(
            program,
            facts,
            owner,
            state,
            structural,
            &sequence.structural_results,
            &mut operations,
            &terminator,
            trace,
        )?;
        planned.push(CheckedComposedUnitControlStatePlan {
            normal_exit_cleanups: Vec::new(),
            state: state.symbol,
            structural_parameters: structural.clone(),
            scalar_parameters: scalar.clone(),
            parameter_storage,
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
            operations,
            place_loans,
            terminator,
            terminator_statement_ordinal,
            statement_count: u32::try_from(statements.len()).ok()?,
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
    for (state_index, &(owner, state)) in fused.iter().enumerate() {
        if !live_states[state_index] {
            continue;
        }
        trace.state(Some(state.symbol));
        let qualified_arrival =
            planned[state_index]
                .structural_parameters
                .iter()
                .any(|parameter| {
                    !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                });
        if qualified_arrival
            && !(state_index == 0 && invocation_only_entry)
            && !checked_qualified_arrivals(facts, &planned, &live_states, state_index)
        {
            trace
                .phase("state graph: state signature: qualified parameter lacks a checked arrival");
            return None;
        }
        for parameter in &planned[state_index].structural_parameters {
            let reference =
                || program.state_parameters(state)[parameter.position as usize].type_reference;
            match parameter.access {
                CheckedStructuralAccess::Owned => {
                    // The registered structural shape carries records and
                    // fixed arrays alike. Admission depends on their recursive
                    // contents and custody, not a nominal type spelling.
                    // A parameter carries an established value, not a fresh
                    // initializer. Exact edge custody retains represented byte
                    // predicates, including those below record/array children.
                    // A carrier of plain fields and shared views arrives as a
                    // whole move whose views keep their source loans' lineage:
                    // a descriptor copy rides any non-linear carrier, `copy`
                    // or affine, under the contents judgment the dispatch
                    // applies to the same carrier as a scrutinee subject.
                    // Invocation entry and state arrivals both retain every
                    // borrowed field's opaque ingress identity. The source
                    // loan roster supplies this authority, not copy permission
                    // on an enclosing record or the descriptor's geometry.
                    let source_parameter =
                        &program.state_parameters(state)[parameter.position as usize];
                    let view_carrier = validation::has_contents(
                        program,
                        reference(),
                        &[],
                        validation::ContentsRequirement::OwnedOrSharedView,
                    )
                        && validation::reference_result_custody::stored_carrier_ingress_is_complete(
                            program,
                            facts,
                            owner.symbol,
                            state.symbol,
                            source_parameter.symbol,
                            reference(),
                        );
                    // A whole arrival carries its owner's eventual cleanup
                    // obligation. Its hook does not make the storage invalid;
                    // exact selected normal-edge actions dispose only what
                    // remains after transfers and returned-value custody.
                    if parameter.multiplicity != Multiplicity::Linear
                        && ![
                            validation::ContentsRequirement::CleanupOwned,
                            validation::ContentsRequirement::RepresentedOwned,
                            // Finite storage arrives as one whole move even
                            // when an affine root contains linear descendants.
                            // Edge custody separately transports their claims.
                            validation::ContentsRequirement::StructuralConstruction,
                        ]
                        .into_iter()
                        .any(|requirement| validation::has_contents(
        program,
        reference(),
        &[],
        requirement
    ))
                        && !view_carrier
                        // Stored-reference carriers use the ordinary call
                        // contract. State edges move their descriptors and
                        // retain loan provenance; they never copy referents.
                        && !validation::reference_result_custody::is_reference_record(
                            program, reference(),
                        )
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: owned non-linear record contents",
                        );
                        return None;
                    }
                }
                _ => {
                    if parameter.multiplicity != Multiplicity::Unrestricted {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: borrowed restricted parameter",
                        );
                        return None;
                    }
                    let reference = reference();
                    // Qualifications restrict the arriving value, not the
                    // carrier its loan lends. Inspect the declared shell once;
                    // access and qualification transport remain checked above
                    // and by each successor's custody judgment.
                    let borrowed_shell = validation::reference_shell(program, reference);
                    let borrowed_carrier = borrowed_shell
                        .map(|shell| validation::peel_declared_constraints(program, shell.referee));
                    // A primitive referent is storage just like a record leaf.
                    // State arrivals retain the loan's declared access; they
                    // do not need readable contents to carry its address.
                    let borrowed_primitive_referent = matches!(
                        shapes
                            .types
                            .get(&parameter.type_identity)
                            .map(|plan| &plan.shape),
                        Some(checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(_))
                    );
                    // A nominal referent, including a generic data
                    // application such as `&FixedBuffer<2>` in a
                    // specialization.
                    let borrowed_named_referent = matches!(
                        borrowed_carrier
                            .map(|carrier| program.type_reference_table.type_reference(carrier)),
                        Some(TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. })
                    );
                    // Fixed arrays have structural rather than nominal
                    // identity. Their registered shape and plain owned
                    // contents admit the same loan across a state edge as
                    // across an ordinary call; access and qualifications
                    // remain checked above and by each successor transfer.
                    let borrowed_array_referent = matches!(
                                        borrowed_shell,
                                        Some(shell)
                                            if matches!(
                                                borrowed_carrier.map(|carrier| program.type_reference_table.type_reference(carrier)),
                                                Some(TypeReferenceNode::FixedArray { .. })
                                            ) && validation::has_contents(
                        program,
                        shell.referee,
                        &[],
                        validation::ContentsRequirement::NumericOwned
                    )
                                    )
                        && matches!(
                            shapes
                                .types
                                .get(&parameter.type_identity)
                                .map(|plan| &plan.shape),
                            Some(checked_trees::CheckedUnitStructuralTypeShape::FixedArray { .. })
                        );
                    // A borrowed bounded owner (`&[u8; N] in Utf8`) names the
                    // owner's carrier; the reference is this parameter's loan.
                    if !borrowed_named_referent
                        && !borrowed_primitive_referent
                        && !borrowed_array_referent
                        && parameter.binding_carrier.is_none()
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
        checked_composed_provider_attachment_requirements(program, shapes, *identity, &flows)?
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
    plan.preserving_rank_edges = preserving_rank_edges;
    // The machine result contract is relative to invocation parameters.
    // Returning states retain their own local origins; the shared result query
    // substitutes every internal edge before publishing the entry relation.
    // Equal parameter ordinals in different states are not equal places.
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
    if whole_reference
        || (matches!(
            result,
            checked_trees::CheckedControlResultPlan::Structural(_)
        ) && super::reference_results::is_reference_record(program, states[0].return_type))
    {
        trace.phase("state graph: reference result origins");
        plan.result_reference_sources =
            validation::reference_result_custody::captured_return_sources(
                program, facts, &states[0],
            )?;
    }
    plan.result = result;
    Some(plan)
}

type Signature = (
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
);
