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
    CheckedGuardedJumpPlan, CheckedScalarBinding, CheckedScalarBindingValue,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, ExpressionNode, Multiplicity, PermissionEventKind,
    PermissionEventSource, StatementNode, TransitionExit, TransitionGuardNode,
    TransitionTargetNode, TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::calls::{
    entry_claims, free_structural_scalar_signature_traced, structural_scalar_signature_traced,
};
use crate::execution::terminal_unit::providers::checked_composed_provider_attachment_requirements;
use crate::execution::terminal_unit::returns::checked_boolean_contains_short_circuit;
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_reference, machine_binders, state_flow,
};
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
use guards::{forwards_qualified_parameter, retire_guarded_reference_completion};
pub(super) use guards::{guard_is_boolean, retained_guard};
pub(super) use result_custody::retained_structural_result;
pub(super) use successors::{
    EdgeCalls, SuccessorEdge, SuccessorGuard, successor, successor_bindings,
};

/// Plans explicit state signatures and transitions, tracing the first declined operation.
pub(super) fn build_traced(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
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
    let result = returns::signature(program, shapes, states[0].return_type)?;
    if matches!(&result, checked_trees::CheckedControlResultPlan::Structural(result)
        if result.multiplicity == Multiplicity::Linear)
        && !program.machine_contracts(machine).is_empty()
    {
        // Parameter qualifications are transported exactly below. Additional
        // authored contracts still need their own retained proof obligations.
        return None;
    }
    // State count and the presence of an aggregate initializer do not choose
    // cleanup semantics. Even one Unit state can owe executable normal-exit
    // actions. Plan its graph here; the shared candidate selection keeps one
    // body when the ordinary sequence also supplies a complete implementation.
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
    let natural_ranks = if fused.len() != states.len()
        && fused
            .iter()
            .any(|(owner, _)| owner.termination_plan.implementation_witness.is_some())
    {
        // Fusion must retain the whole component's judgment, including when
        // an unranked dispatch machine reaches ranked callees. Acyclic exits
        // carry no rank; the existing fused checker proves every cyclic edge.
        crate::checks::termination::proven_fused_state_natural_ranks_with_call_frames(
            program,
            &fused
                .iter()
                .map(|(_, state)| state.symbol)
                .collect::<Vec<_>>(),
            call_frames,
        )?
    } else if machine.termination_plan.implementation_witness.is_some() {
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
    for &(owner, state) in fused.iter() {
        trace.phase("state graph: state signature: result and contracts");
        trace.state(Some(state.symbol));
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
            validation::state_declaration_predicates(program, &facts.operators, state)
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
        trace.phase("state graph: prefix initializers");
        trace.state(Some(state.symbol));
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
            .is_some_and(|target| fused_target_index(program, &fused, target).is_none())
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
            normal_exit_cleanups: Vec::new(),
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
            terminator_statement_ordinal,
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
        trace.state(Some(state.symbol));
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
                    // The one-shot entry names the caller's custody: its loan
                    // has no checked transfer to ride in on, so only the
                    // one-record-deep view shape the invocation can spell is
                    // admitted there — the wider contents family enters
                    // exclusively through established edge custody.
                    let view_carrier = if state_index == 0 && invocation_only_entry {
                        parameter.multiplicity == Multiplicity::Unrestricted
                            && validation::has_owned_or_shared_view_fields(program, reference())
                    } else {
                        validation::has_contents(
                            program,
                            reference(),
                            validation::ContentsRequirement::OwnedOrSharedView,
                        )
                    };
                    // A whole arrival carries its owner's eventual cleanup
                    // obligation. Its hook does not make the storage invalid;
                    // exact selected normal-edge actions dispose only what
                    // remains after transfers and returned-value custody.
                    if parameter.multiplicity != Multiplicity::Linear
                        && !validation::has_contents(program, reference(), validation::ContentsRequirement::CleanupOwned)
                        && !validation::has_contents(
                            program,
                            reference(), validation::ContentsRequirement::RepresentedOwned)
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
                            ) && validation::has_contents(
                                program, *referee, validation::ContentsRequirement::NumericOwned)
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
                        && !borrowed_primitive_referent
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
    if whole_reference {
        trace.phase("state graph: reference result origins");
        plan.result_reference_sources =
            validation::reference_result_custody::returned_reference_sources(program, &states[0])?;
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

type Signature = (
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
);
