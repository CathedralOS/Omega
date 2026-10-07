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
    TransitionTargetNode, TypeReferenceNode, TypedTrees, calls,
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
mod terminal_calls;
mod terminator;

use fused::fused_states;
pub(super) use fused::{FusedStates, fused_target_index, terminator_statement_index};
use guards::{forwards_qualified_parameter, retire_guarded_reference_completion};
pub(super) use guards::{guard_is_boolean, retained_guard};

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
    // Custody belongs to the produced value, not its source idiom. An
    // extracted old referent owns the same affine obligation as a call or
    // constructor result. Unrestricted inline operands alone retain their
    // consuming-call establishment rather than a reusable local binding.
    producer
        .structural_result_binding()
        .is_some_and(|result| result.multiplicity != Multiplicity::Unrestricted)
        || matches!(
            producer,
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                operand_source: Some(
                    checked_trees::CheckedArrayConstructionSource::CallArgument { .. }
                ),
                ..
            }
        )
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
        && retained_structural_result(producer).is_some()
        // The argument planner matched the selected subtree's declaration
        // and exact transfer. Graph custody retains its owner binding; a
        // projection does not turn that binding into a whole-value argument.
        // Copies leave the original for its existing state-exit accounting;
        // affine/linear transfers use the same consumption rule as that
        // accounting, which independently requires exactly one disposition.
        && (result.multiplicity == Multiplicity::Unrestricted || call_owns_result(producer))
}

/// Name the first unmet call requirement using the same argument admission
/// as the operation guard, then distinguish its unsupported custody.
fn call_custody_refusal(
    structural_arguments: &[CheckedUnitStructuralArgumentPlan],
    earlier: &[CheckedUnitEffectOperationPlan],
) -> &'static str {
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
                        || super::types::base_type_identity(program, root.type_reference, &[])?
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
                                super::types::parameter_is_root(parameter, root_symbol)
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
                                && !super::types::type_graph_requires_nominal_drop(
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
        .find(|(_, parameter)| super::types::parameter_is_root(parameter, root_symbol));
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
            || super::types::base_type_identity(program, local.type_reference, &[])?
                != result.type_identity
        {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            super::types::structural_access_for_type_reference(program, local.type_reference)?,
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
        || super::types::base_type_identity(program, projected_type, &[])
            .is_none_or(|identity| identity != target.type_identity)
    {
        return None;
    }
    let access = super::types::structural_access_for_type_reference(program, projected_type)?;
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
        .or_else(|| super::types::base_type_identity(program, projected_type, &[]));
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
        super::types::base_type_identity(program, reference, &[])
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
            && super::types::structural_access_for_type_reference(program, field.type_reference)
                == Some(CheckedStructuralAccess::SharedBorrow)
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
