//! Rejoin every retained state, call, and edge before admitting the graph.
use super::super::super::super::{CheckedComposedUnitControlTerminatorPlan, PrimitiveType};
use super::super::super::{
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedScalarExpressionRole,
    Multiplicity, unsupported,
};
use super::super::{CheckedTrees, LoweringError};
use super::{
    CheckedComposedUnitControlMachinePlan, claims, edges, entry_aliases, exclusive_handoffs,
    scalars, topology,
};
use checked_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

/// Custody, not topology, selects this emitter. Once selected, failed source
/// rejoining must not retry an older matcher that ignores additional statements.
pub(in crate::unit::attached_unit::composed_control) fn has_shared_graph_custody(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
) -> bool {
    let Some(machine) = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
    else {
        return false;
    };
    // Authored lifetime binders erase at checking: the retained plan's
    // type identities and custody paths never spell an 'a name. Generic
    // type parameters still need instantiation machinery and stay out.
    checked.machine_type_parameters(machine).is_empty() && plan.body_qualifications.is_empty() && {
        // Unreachable states never execute: their custody rows cannot
        // disqualify the route. Reachable positions still enforce every
        // parameter's custody shape. A missing edge target is topology
        // drift, not custody — defer to `admit`'s authored-roster checks
        // for the precise drift diagnostic.
        let Ok(live) = topology::live(plan) else {
            return true;
        };
        let qualified_invocation = invocation_only_entry(plan, &live);
        plan.states
            .iter()
            .zip(&live)
            .enumerate()
            .filter_map(|(index, (state, live))| live.then_some((index, state)))
            .all(|(index, state)| {
                state.structural_parameters.iter().enumerate().all(
                    |(parameter_index, parameter)| {
                        ((parameter.multiplicity == Multiplicity::Unrestricted
                            && parameter.access != checked_trees::CheckedStructuralAccess::Owned)
                            || parameter.access == checked_trees::CheckedStructuralAccess::Owned)
                            && (parameter.qualifications.is_empty()
                                || parameter.multiplicity == Multiplicity::Linear
                                || (index == 0 && qualified_invocation)
                                || forwards_qualified_parameter(
                                    checked,
                                    plan,
                                    &live,
                                    index,
                                    parameter_index,
                                ))
                    },
                )
            })
            && claim_transport_supported(checked, plan, &live)
    }
}

/// A state's entry claims are established at every admission of that state,
/// so a claim carried across an edge keeps the machine parameter's place
/// instead of becoming a fresh block parameter. Successor transport must
/// resolve each claim onto an entry parameter place regardless of topology.
fn claim_transport_supported(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
) -> bool {
    if plan
        .states
        .iter()
        .all(|state| state.entry_claims.is_empty())
    {
        return true;
    }
    claims::resolve(checked, plan, live).is_ok()
}

pub(in crate::unit::attached_unit) struct AdmittedGraph<'a> {
    /// Entry-state successor-closure mask over the full `plan.states` roster;
    /// dead positions keep their index alignment but emit no block.
    pub(super) live: Vec<bool>,
    /// Each plan position's authored `(owner machine, state)`: a fused roster
    /// mixes owners, so emission resolves both through this row rather than
    /// assuming the dispatch machine's own state list.
    pub(super) authored: Vec<(
        &'a checked_trees::machine::Machine,
        &'a checked_trees::state::State,
    )>,
    pub(super) claim_transport: claims::ClaimTransport,
    /// Per plan state, the shared parameters declared as the whole exclusive
    /// loan every arrival forwards (`exclusive_handoffs`).
    pub(super) exclusive_handoffs: Vec<std::collections::BTreeSet<u32>>,
    pub(in crate::unit::attached_unit::composed_control) boundaries:
        Vec<(&'a CheckedBoundaryMachinePlan, String)>,
}

pub(in crate::unit::attached_unit::composed_control) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedGraph<'a>, LoweringError> {
    super::super::admission::validate_contract(checked, plan)?;
    if plan.states.is_empty()
        || !plan.body_qualifications.is_empty()
        || checked
            .facts
            .qualifications
            .for_machine(plan.machine)
            .is_some_and(|fact| {
                fact.body_committed.iter().any(|domain| {
                    !matches!(
                        *domain,
                        language_semantics::SemanticDomainTable::WRAPPING
                            | language_semantics::SemanticDomainTable::SATURATING
                            | language_semantics::SemanticDomainTable::TRAPPING
                    )
                })
            })
    {
        return unsupported("Unit graph has unsupported qualification or provider custody");
    }
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .ok_or(LoweringError::Unsupported(
            "Unit graph has no authored machine",
        ))?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !checked.machine_type_parameters(machine).is_empty()
    {
        return unsupported("Unit graph requires a closed checked body");
    }
    // Whole linear transport retains declaration qualifications and claim
    // conservation, not an arbitrary authored machine precondition package.
    if matches!(&plan.result, checked_trees::CheckedControlResultPlan::Structural(result)
        if result.multiplicity == Multiplicity::Linear)
        && !checked.machine_contracts(machine).is_empty()
    {
        return unsupported("linear structural graph has unrepresented authored machine contracts");
    }
    super::ranking::validate_witness(checked, machine, plan)?;
    let attachment = super::super::admission::exact_attachment(checked, plan)?;
    // A fused roster admits foreign states: each position resolves its own
    // authored owner, no state may repeat, and the dispatch machine's authored
    // states still all appear in authored order.
    let authored = plan
        .states
        .iter()
        .map(|state| crate::declarations::state_declaration(checked, state.state))
        .collect::<Result<Vec<_>, _>>()?;
    let entry = checked
        .machine_states(machine)
        .first()
        .ok_or(LoweringError::Unsupported(
            "Unit graph has no authored entry state",
        ))?;
    if let Some(identity) = plan.result.structural_identity() {
        let mut declarations = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .filter(|declaration| declaration.identity == identity);
        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
            "Unit graph result carrier declaration is absent",
        ))?;
        if declarations.next().is_some() {
            return unsupported("Unit graph result carrier declaration is ambiguous");
        }
        // References publish a referent-origin interface. Element views and
        // byte sequences retain their own descriptor custody even when their
        // authored type also has a borrow shell.
        if matches!(
            declaration.shape,
            checked_trees::CheckedUnitStructuralTypeShape::Reference { .. }
        ) {
            super::guarded::validate_reference_roster(checked, entry)?;
        }
    }
    // Reachability is pure topology on the checked plan. States outside the
    // entry successor closure never execute: their custody rows cannot
    // disqualify the route and emission prunes them.
    let live = topology::live(plan)?;
    let qualified_invocation = invocation_only_entry(plan, &live);
    for (state_index, ((owner, source), state)) in authored.iter().zip(&plan.states).enumerate() {
        crate::unit::attached_unit::claims::validate_entry_claims(
            checked,
            owner.symbol,
            source,
            &state.structural_parameters,
            &state.entry_claims,
        )?;
        // `requires` rows that lower into closed scalar predicates stay on
        // the plan as retained state requires and emit as header invariants;
        // every other authored contract row is still rejected outright.
        let authored =
            validation::structural_state_contract_scalar_predicates(&checked.typed, source).ok_or(
                LoweringError::Unsupported(
                    "structural graph state has unrepresented authored contracts",
                ),
            )?;
        let declaration_rows = validation::state_declaration_predicates(
            &checked.typed,
            &checked.facts.operators,
            source,
        )
        .into_iter()
        .map(|row| {
            Some(checked_trees::ClosedScalarContractValue::Predicate(
                row.predicate,
            ))
        })
        .collect::<Vec<_>>();
        if state.requires.len() != authored.len() + declaration_rows.len()
            || !state.requires[..authored.len()].iter().all(Option::is_some)
            || state.requires[authored.len()..] != declaration_rows
        {
            return unsupported("structural graph state declaration predicates differ from source");
        }
        if !super::returns::signature_matches(checked, source, &plan.result) {
            return unsupported("structural graph result signature disagrees with source");
        }
        if crate::unit::attached_unit::parameters::checked_scalar_source_parameters(
            checked, source,
        )? != state.scalar_parameters
        {
            return unsupported("Unit graph scalar signature disagrees with source");
        }
        let source_parameters = checked.state_parameters(source);
        // Erased bindings own no plan entry.
        let retained_parameters = source_parameters
            .iter()
            .filter(|parameter| !parameter.relevance.is_erased())
            .count();
        if state.structural_parameters.len() + state.scalar_parameters.len() != retained_parameters
            || state
                .structural_parameters
                .windows(2)
                .any(|pair| pair[0].position >= pair[1].position)
        {
            return unsupported("Unit graph signature dropped or duplicated a parameter");
        }
        for (parameter_index, parameter) in state.structural_parameters.iter().enumerate() {
            let source = source_parameters.get(parameter.position as usize).ok_or(
                LoweringError::Unsupported("Unit graph view parameter position is invalid"),
            )?;
            if validation::structural_result_projected_qualifications(
                &checked.typed,
                source.type_reference,
            )
            .ok()
            .as_ref()
                != Some(&parameter.projected_qualifications)
            {
                return unsupported(
                    "Unit graph projected parameter qualifications differ from source",
                );
            }
            if !live[state_index] {
                // An unreachable state's parameter custody rows never
                // execute: they cannot disqualify the route.
                continue;
            }
            if let Some(carrier) = checked
                .bound_service_parameter_carrier(source.type_reference)
                .map_err(|_| {
                    LoweringError::Unsupported("Unit graph has an invalid Binding carrier")
                })?
            {
                // Service authority comes from this state's exact source receipt,
                // never from an empty storage shape. Ordinary edge custody still
                // checks which existing occurrence supplies the next state.
                crate::unit::attached_unit::parameters::validate_service_parameter(
                    parameter, &carrier,
                )?;
                let mut shapes = checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .structural_types
                    .iter()
                    .filter(|shape| shape.identity == parameter.type_identity);
                if !matches!(
                    shapes.next().map(|shape| &shape.shape),
                    Some(checked_trees::CheckedUnitStructuralTypeShape::FusedServiceBinding)
                ) || shapes.next().is_some()
                {
                    return unsupported(
                        "Unit graph Binding lost its exact nonconstructible carrier",
                    );
                }
                continue;
            }
            if parameter.access == checked_trees::CheckedStructuralAccess::Owned {
                // State parameters receive established values. The complete
                // type below retains nested byte predicates; this storage
                // classifier grants neither construction nor copy permission.
                // Binding mutability permits writes to this received owner; it
                // does not change the edge's owned transfer into the state.
                // Each write or reborrow retains its checked place access.
                if source.is_const
                    || parameter.fused_service_erasure.is_some()
                    || validation::structural_result_qualifications(
                        &checked.typed,
                        source.type_reference,
                    )
                    .ok()
                    .as_ref()
                        != Some(&parameter.qualifications)
                    || checked.type_multiplicity(source.type_reference) != parameter.multiplicity
                    || checked
                        .normalized_type_identity(
                            crate::unit::attached_unit::parameters::structural_carrier_type(
                                checked,
                                source.type_reference,
                            )?,
                        )
                        .as_str()
                        != parameter.type_identity
                    || (parameter.multiplicity != Multiplicity::Linear
                        && !validation::has_contents(
                            &checked.typed,
                            source.type_reference, validation::ContentsRequirement::CleanupOwned)
                        && !crate::content_classes::plain_owned_with_represented_constraints(checked,
                            crate::unit::attached_unit::parameters::structural_carrier_type(
                                checked,
                                source.type_reference,
                            )?)
                        // A carrier of plain fields and shared views arrives
                        // whole — a move under the edge's own transfer for an
                        // affine descriptor, a copy for an unrestricted one —
                        // and its views keep their source loans' lineage.
                        && !validation::has_contents(
                            &checked.typed,
                            crate::unit::attached_unit::parameters::structural_carrier_type(
                                checked,
                                source.type_reference,
                            )?, validation::ContentsRequirement::OwnedOrSharedView)
                        && !validation::reference_result_custody::is_reference_record(
                            &checked.typed,
                            source.type_reference,
                        ))
                {
                    return unsupported(
                        "Unit graph owned parameter differs from exact source custody",
                    );
                }
                let mut shapes = checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .structural_types
                    .iter()
                    .filter(|shape| shape.identity == parameter.type_identity);
                if shapes.next().is_none() || shapes.next().is_some() {
                    return unsupported("Unit graph owned parameter type missing or duplicated");
                }
                continue;
            }
            borrowed_parameter(
                checked,
                plan,
                &live,
                state_index,
                parameter_index,
                qualified_invocation,
                source,
                parameter,
            )?;
        }
        let statements = checked.statement_table.statements(source.statement_nodes);
        scalars::validate(checked, state)?;
        let terminator_ordinal = state.terminator_statement_ordinal as usize;
        let tail =
            typed_trees::statement::reachable_transition_prefix(&statements[terminator_ordinal..]);
        match (&state.terminator, tail) {
            (CheckedComposedUnitControlTerminatorPlan::Guarded { .. }, _) => {}
            (CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }, _) => {
                super::cases::validate(checked, plan, source, state, tail, terminator_ordinal)?;
            }
            (CheckedComposedUnitControlTerminatorPlan::ReturnUnit, [])
                if plan.result == checked_trees::CheckedControlResultPlan::Unit =>
            {
                edges::return_discards(checked, owner.symbol, source, state)?;
            }
            // The unconditional terminal machine call stage 04 planned as the
            // state's last operation completes the machine with Unit.
            (
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
                [StatementNode::Transition(transition)],
            ) if plan.result == checked_trees::CheckedControlResultPlan::Unit
                && transition.guard == TransitionGuardNode::Always
                && state.operations.iter().any(|operation| {
                    operation.call().is_some_and(|call| {
                        call.coordinate.statement_index as usize == terminator_ordinal
                            && call.coordinate.call_ordinal == 0
                    })
                })
                && crate::call_sites::linear_terminal_invocation(
                    checked,
                    state.state,
                    u32::try_from(terminator_ordinal)
                        .map_err(|_| LoweringError::Unsupported("Unit graph ordinal overflow"))?,
                )? =>
            {
                edges::return_discards(checked, plan.machine, source, state)?;
            }
            (CheckedComposedUnitControlTerminatorPlan::ReturnScalar { completion: _ }, _) => {
                edges::return_discards(checked, owner.symbol, source, state)?;
            }
            (
                CheckedComposedUnitControlTerminatorPlan::Crash { statement_ordinal },
                [StatementNode::Transition(transition)],
            ) if matches!(transition.exit, TransitionExit::Crash(_))
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid()
                && u32::try_from(terminator_ordinal).ok() == Some(*statement_ordinal) =>
            {
                // The terminal target, authored cause and checked-site row are
                // revalidated by `lower_checked_crash_exit` at emission.
            }
            (
                CheckedComposedUnitControlTerminatorPlan::ReturnCase { .. },
                [StatementNode::Expression(_)],
            ) => {}
            (
                CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. },
                [StatementNode::Expression(_)],
            ) => {
                super::returns::validate_structural(
                    checked,
                    plan,
                    source,
                    state,
                    terminator_ordinal,
                )?;
            }
            (
                CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. },
                [StatementNode::Transition(transition)],
            ) if transition.guard == TransitionGuardNode::Always
                && transition.exit == TransitionExit::Ordinary
                && !transition.continuation.is_valid()
                && matches!(
                    checked.statement_table.transition_target(transition.target),
                    checked_trees::statement::TransitionTargetNode::Value(_)
                ) =>
            {
                // An unconditional `(expression)` arm returns its value on the
                // transition's own statement; the same structural checks.
                super::returns::validate_structural(
                    checked,
                    plan,
                    source,
                    state,
                    terminator_ordinal,
                )?;
            }
            (
                CheckedComposedUnitControlTerminatorPlan::Jump { successor },
                [StatementNode::Transition(transition)],
            ) if transition.guard == TransitionGuardNode::Always => {
                edges::validate(
                    checked,
                    plan,
                    source,
                    state,
                    transition,
                    successor,
                    terminator_ordinal,
                )?;
            }
            (
                CheckedComposedUnitControlTerminatorPlan::Conditional {
                    guard,
                    when_true,
                    when_false,
                },
                [
                    StatementNode::Transition(true_source),
                    StatementNode::Transition(false_source),
                ],
            ) if matches!(true_source.guard, TransitionGuardNode::When(_)) => {
                validate_guard(
                    checked,
                    owner.symbol,
                    state.state,
                    terminator_ordinal,
                    guard,
                )?;
                edges::validate(
                    checked,
                    plan,
                    source,
                    state,
                    true_source,
                    when_true,
                    terminator_ordinal,
                )?;
                edges::validate(
                    checked,
                    plan,
                    source,
                    state,
                    false_source,
                    when_false,
                    terminator_ordinal + 1,
                )?;
            }
            (
                CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                    guard,
                    jump,
                    return_arm,
                    return_when_true,
                },
                [
                    StatementNode::Transition(true_source),
                    StatementNode::Transition(false_source),
                ],
            ) if matches!(true_source.guard, TransitionGuardNode::When(_)) => {
                validate_guard(
                    checked,
                    owner.symbol,
                    state.state,
                    terminator_ordinal,
                    guard,
                )?;
                let (jump_source, return_source, jump_ordinal, return_ordinal) =
                    if *return_when_true {
                        (
                            false_source,
                            true_source,
                            terminator_ordinal + 1,
                            terminator_ordinal,
                        )
                    } else {
                        (
                            true_source,
                            false_source,
                            terminator_ordinal,
                            terminator_ordinal + 1,
                        )
                    };
                match return_arm {
                    checked_trees::CheckedConditionalReturnArm::Crash { .. } => {}
                    checked_trees::CheckedConditionalReturnArm::Unit { .. }
                        if plan.result == checked_trees::CheckedControlResultPlan::Unit
                            && return_source.exit == TransitionExit::Ordinary
                            && !return_source.continuation.is_valid()
                            && matches!(
                                checked
                                    .statement_table
                                    .transition_target(return_source.target),
                                checked_trees::statement::TransitionTargetNode::Terminal
                            ) =>
                    {
                        // Rejoin the exact normal completion before applying
                        // ordinary return disposal. A crash or named jump
                        // cannot acquire cleanup/return authority by relabeling.
                        edges::return_discards(checked, plan.machine, source, state)?;
                    }
                    checked_trees::CheckedConditionalReturnArm::Structural(operation) => {
                        validate_structural_arm(
                            checked,
                            plan,
                            state,
                            return_source,
                            return_ordinal,
                            operation,
                        )?;
                    }
                    checked_trees::CheckedConditionalReturnArm::Call {
                        operands,
                        operation,
                    } => validate_arm_call(
                        checked,
                        plan,
                        source,
                        state,
                        return_source,
                        return_ordinal,
                        operands,
                        operation,
                    )?,
                    // A scalar arm returns the value checking retained under its
                    // own `Return` role: the machine's result is that scalar and
                    // the authored arm names a value, not a state.
                    checked_trees::CheckedConditionalReturnArm::Scalar {
                        primitive_type, ..
                    } if plan.result
                        == checked_trees::CheckedControlResultPlan::Scalar {
                            primitive_type: *primitive_type,
                        }
                        && matches!(
                            checked
                                .statement_table
                                .transition_target(return_source.target),
                            checked_trees::statement::TransitionTargetNode::Value(_)
                        ) => {}
                    _ => {
                        return unsupported(
                            "Unit graph conditional return arm does not match its declared completion",
                        );
                    }
                }
                edges::validate(
                    checked,
                    plan,
                    source,
                    state,
                    jump_source,
                    jump,
                    jump_ordinal,
                )?;
            }
            // Each selected arm completes the activation, through ordinary
            // result custody or its exact abnormal crash site.
            (
                CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                    guard,
                    when_true,
                    when_false,
                },
                [
                    StatementNode::Transition(true_source),
                    StatementNode::Transition(false_source),
                ],
            ) if matches!(true_source.guard, TransitionGuardNode::When(_)) => {
                validate_guard(
                    checked,
                    plan.machine,
                    state.state,
                    terminator_ordinal,
                    guard,
                )?;
                for (arm, arm_source, arm_ordinal) in [
                    (when_true, true_source, terminator_ordinal),
                    (when_false, false_source, terminator_ordinal + 1),
                ] {
                    match arm {
                        checked_trees::CheckedConditionalReturnArm::Crash { .. } => {}
                        checked_trees::CheckedConditionalReturnArm::Call {
                            operands,
                            operation,
                        } => {
                            validate_arm_call(
                                checked,
                                plan,
                                source,
                                state,
                                arm_source,
                                arm_ordinal,
                                operands,
                                operation,
                            )?;
                        }
                        checked_trees::CheckedConditionalReturnArm::Structural(operation) => {
                            validate_structural_arm(
                                checked,
                                plan,
                                state,
                                arm_source,
                                arm_ordinal,
                                operation,
                            )?;
                        }
                        checked_trees::CheckedConditionalReturnArm::Unit { statement_ordinal }
                            if plan.result == checked_trees::CheckedControlResultPlan::Unit
                                && *statement_ordinal as usize == arm_ordinal
                                && arm_source.exit == TransitionExit::Ordinary
                                && !arm_source.continuation.is_valid()
                                && matches!(
                                    checked.statement_table.transition_target(arm_source.target),
                                    checked_trees::statement::TransitionTargetNode::Terminal
                                ) =>
                        {
                            edges::return_discards(checked, plan.machine, source, state)?;
                        }
                        // Beside a terminal scalar call, the other arm may
                        // return the value checking retained under its own
                        // `Return` role, exactly as a conditional return's
                        // scalar arm does.
                        checked_trees::CheckedConditionalReturnArm::Scalar {
                            statement_ordinal,
                            primitive_type,
                        } if plan.result
                            == checked_trees::CheckedControlResultPlan::Scalar {
                                primitive_type: *primitive_type,
                            }
                            && *statement_ordinal as usize == arm_ordinal
                            && arm_source.exit == TransitionExit::Ordinary
                            && !arm_source.continuation.is_valid()
                            && matches!(
                                checked.statement_table.transition_target(arm_source.target),
                                checked_trees::statement::TransitionTargetNode::Value(_)
                            ) =>
                        {
                            edges::return_discards(checked, plan.machine, source, state)?;
                        }
                        _ => {
                            return unsupported(
                                "Unit graph conditional completion arm does not complete the machine",
                            );
                        }
                    }
                }
            }
            (CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback }, tail)
                if tail.len() == arms.len() + 1
                    && tail[..arms.len()].iter().all(|statement| {
                        matches!(statement, StatementNode::Transition(transition)
                        if matches!(transition.guard, TransitionGuardNode::When(_)))
                    })
                    && matches!(tail.last(), Some(StatementNode::Transition(transition))
                    if matches!(
                        transition.guard,
                        TransitionGuardNode::Always | TransitionGuardNode::When(_)
                    )) =>
            {
                // The shared scalar tail roster owns this chain's guard order
                // and selected destinations; every arm must rejoin it exactly.
                let last_guarded = matches!(
                    tail.last(),
                    Some(StatementNode::Transition(transition))
                        if matches!(transition.guard, TransitionGuardNode::When(_))
                );
                let retained = checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_tails
                    .iter()
                    .filter(|tail| tail.state == state.state)
                    .collect::<Vec<_>>();
                let [retained] = retained.as_slice() else {
                    return unsupported("guarded jump successors lost their source roster");
                };
                let exits = checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span(retained.arms)
                    .ok_or(LoweringError::Unsupported(
                        "guarded jump successors have a stale roster",
                    ))?;
                // A guarded final arm is the roster's proven else: it is
                // retained as an exit and no authored fallback is recorded.
                for (index, (arm, exit)) in arms.iter().zip(exits.iter()).enumerate() {
                    let ordinal = terminator_ordinal + index;
                    let StatementNode::Transition(transition) = &tail[index] else {
                        return unsupported("guarded jump arm lost its authored transition");
                    };
                    match (arm, &exit.destination) {
                        (
                            checked_trees::CheckedGuardedJumpArmPlan::Jump(arm_plan),
                            checked_trees::CheckedScalarBranchDestination::Jump(_),
                        ) => {
                            validate_guard(
                                checked,
                                plan.machine,
                                state.state,
                                ordinal,
                                &arm_plan.guard,
                            )?;
                            edges::validate(
                                checked,
                                plan,
                                source,
                                state,
                                transition,
                                &arm_plan.successor,
                                ordinal,
                            )?;
                        }
                        (
                            checked_trees::CheckedGuardedJumpArmPlan::Return(return_plan),
                            destination,
                        ) => {
                            // The same validation the fallback's return arm gets.
                            validate_guarded_return(
                                checked,
                                plan,
                                source,
                                state,
                                transition,
                                &return_plan.return_arm,
                                destination,
                            )?;
                            validate_guard(
                                checked,
                                plan.machine,
                                state.state,
                                ordinal,
                                &return_plan.guard,
                            )?;
                        }
                        _ => {
                            return unsupported(
                                "guarded jump arm is not a named-state edge or scalar return",
                            );
                        }
                    }
                }
                let fallback_destination = if last_guarded {
                    exits.last().map(|exit| &exit.destination)
                } else {
                    retained.fallback.as_ref()
                };
                let ordinal = terminator_ordinal + arms.len();
                let Some(StatementNode::Transition(transition)) = tail.last() else {
                    return unsupported("guarded jump fallback lost its authored transition");
                };
                match (fallback, fallback_destination) {
                    (
                        checked_trees::CheckedGuardedJumpFallbackPlan::Successor(fallback),
                        Some(checked_trees::CheckedScalarBranchDestination::Jump(_)),
                    ) => {
                        edges::validate(
                            checked, plan, source, state, transition, fallback, ordinal,
                        )?;
                    }
                    (
                        checked_trees::CheckedGuardedJumpFallbackPlan::Return(return_arm),
                        Some(destination),
                    ) => {
                        validate_guarded_return(
                            checked,
                            plan,
                            source,
                            state,
                            transition,
                            return_arm,
                            destination,
                        )?;
                    }
                    _ => {
                        return unsupported(
                            "guarded jump fallback is not a named-state edge or scalar return",
                        );
                    }
                }
            }
            _ => return unsupported("Unit graph terminator disagrees with authored state"),
        }
    }
    // Dead states are authored and shape-checked above but never execute:
    // their parameter, claim, and call rows have no producing edge, so the
    // emitted graph prunes to the entry state's successor closure.
    // Claim-bearing successor parameters permanently bind the entry
    // parameter's place; resolve replays each edge's checked Transfer event
    // before emission trusts that alias.
    let mut claim_transport = claims::resolve(checked, plan, &live)?;
    // A whole loan proven to name the same entry storage on every arrival
    // keeps that invocation place instead of duplicating its authority.
    entry_aliases::alias_entry_parameter_loans(plan, &live, &mut claim_transport.aliased)?;
    // A shared parameter that only ever receives its source's whole
    // exclusive loan holds that loan; Terminal edges transfer mutable names.
    let exclusive_handoffs = exclusive_handoffs::promote_shared_handoffs(
        checked,
        plan,
        &live,
        &claim_transport.aliased,
    )?;
    let states = plan
        .states
        .iter()
        .zip(&live)
        .filter_map(|(state, live)| live.then_some(state))
        .collect::<Vec<_>>();
    // Every live state's calls take the one call admission; internal Unit
    // targets' signatures come from the closure that emits this graph.
    let boundaries = super::super::admission::admit_calls(checked, &states)?;
    if let Some(attachment) = attachment {
        let mut called = Vec::new();
        for state in &states {
            for operation in &state.operations {
                if let Some(call) = operation.call()
                    && crate::unit::attached_unit::provider_attachments::validate_call_source(
                        checked,
                        super::owner(checked, state.state)?,
                        state.state,
                        operation,
                        &state.structural_parameters,
                        &plan.provider_attachment_requirements,
                    )?
                {
                    called.push(call.target.machine);
                }
            }
        }
        // Both lexical self and ordinary contexts contribute their exact
        // attachment-field requirements after independent source replay.
        crate::unit::attached_unit::provider_attachments::validate_provider_attachment_requirements(
            checked,
            attachment,
            &plan.provider_attachment_requirements,
            &called,
        )?;
    } else if !plan.provider_attachment_requirements.is_empty() {
        return unsupported("free Unit graph cannot retain provider attachment requirements");
    }
    for (boundary, _) in &boundaries {
        if boundary.machine == boundary.state {
            // Static trait signatures have no runtime provider receiver.
            if boundary.attachment_type_identity.is_some() {
                return unsupported("Unit graph static trait boundary acquired an attachment");
            }
        } else {
            let (owner, _) = crate::declarations::state_declaration(checked, boundary.state)?;
            // A qualified top-level requirement such as `Float::add` names a
            // semantic namespace, not a data attachment, and stage 04's
            // `build_boundary_machine` retains no attachment identity for it.
            // Only an attached name that resolves to a data definition is an
            // authored nominal owner that the boundary must retain.
            let owner_has_data_attachment = owner.attached_data.as_ref().is_some_and(|name| {
                checked
                    .data_definitions()
                    .iter()
                    .any(|data| data.name == *name)
            });
            if owner.symbol != boundary.machine
                || owner_has_data_attachment != boundary.attachment_type_identity.is_some()
            {
                return unsupported("Unit graph boundary attachment lost its authored owner");
            }
            if let Some(retained) = &boundary.attachment_type_identity {
                let data = checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == owner.attached_data_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph boundary has no resolved nominal owner",
                    ))?;
                let identity = if owner.attached_data_application.is_valid() {
                    checked.normalized_type_identity(owner.attached_data_application)
                } else if let Some(application) = data.generic_instance {
                    checked.normalized_type_identity(application)
                } else {
                    checked
                        .normalized_nominal_type_identity(data.symbol)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph boundary has no normalized nominal owner",
                        ))?
                };
                if identity.as_str() != retained {
                    return unsupported(
                        "Unit graph boundary attachment disagrees with its authored nominal identity",
                    );
                }
            }
        }
        // Domain requirements on borrowed inputs lower into the emitted
        // boundary's `requires` rows; qualified affine results mint their
        // caller-side establishments at emission. Copyable structural results
        // retain their value without acquiring an affine disposal obligation.
        // A scalar result binds the state's next scalar value.
        if !(boundary.result.is_unit()
            || matches!(
                &boundary.result,
                CheckedBoundaryMachineResultPlan::Scalar(_)
                    | CheckedBoundaryMachineResultPlan::Structural {
                        multiplicity: Multiplicity::Affine | Multiplicity::Unrestricted,
                        ..
                    }
            ))
        {
            return unsupported(
                "Unit graph boundary requires additional provider or result custody",
            );
        }
    }
    Ok(AdmittedGraph {
        live,
        authored,
        claim_transport,
        exclusive_handoffs,
        boundaries,
    })
}

/// Ordered return arms and the fallback obey the same completion contract.
/// A Unit completion has no value producer, but still owes ordinary cleanup;
/// neither a named jump nor a crash acquires that authority by being relabeled.
fn validate_guarded_return(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    transition: &checked_trees::statement::TableTransition,
    arm: &checked_trees::CheckedConditionalReturnArm,
    destination: &checked_trees::CheckedScalarBranchDestination,
) -> Result<(), LoweringError> {
    use checked_trees::statement::TransitionTargetNode;
    if matches!(
        arm,
        checked_trees::CheckedConditionalReturnArm::Crash { .. }
    ) {
        return Ok(());
    }
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return unsupported("guarded return is not an ordinary completion");
    }
    let ordinal = match (arm, destination) {
        (
            checked_trees::CheckedConditionalReturnArm::Call {
                operands,
                operation,
            },
            checked_trees::CheckedScalarBranchDestination::Jump(selected),
        ) if !selected.is_continuation
            && operation
                .call()
                .is_some_and(|call| call.target.state == selected.target) =>
        {
            validate_arm_call(
                checked,
                plan,
                source,
                state,
                transition,
                selected.statement_ordinal as usize,
                operands,
                operation,
            )?;
            return Ok(());
        }
        (
            _,
            checked_trees::CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation: false,
            },
        ) => *statement_ordinal,
        _ => return unsupported("guarded completion changed its retained destination"),
    };
    let target = checked.statement_table.transition_target(transition.target);
    match (arm, target) {
        (
            checked_trees::CheckedConditionalReturnArm::Unit { .. },
            TransitionTargetNode::Terminal,
        ) if plan.result == checked_trees::CheckedControlResultPlan::Unit => {
            edges::return_discards(checked, plan.machine, source, state)?;
            Ok(())
        }
        (
            checked_trees::CheckedConditionalReturnArm::Structural(operation),
            TransitionTargetNode::Value(_),
        ) => validate_structural_arm(
            checked,
            plan,
            state,
            transition,
            ordinal as usize,
            operation,
        ),
        (
            checked_trees::CheckedConditionalReturnArm::Scalar { primitive_type, .. },
            TransitionTargetNode::Value(_),
        ) if plan.result
            == checked_trees::CheckedControlResultPlan::Scalar {
                primitive_type: *primitive_type,
            } =>
        {
            Ok(())
        }
        _ => unsupported("guarded return disagrees with its authored completion or result"),
    }
}

/// All selected value arms rejoin their exact source through the same ordinary
/// structural-return owner, independently of sibling completion forms.
fn validate_structural_arm(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    transition: &checked_trees::statement::TableTransition,
    ordinal: usize,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return unsupported("structural arm is not an ordinary completion");
    }
    let checked_trees::statement::TransitionTargetNode::Value(expression) =
        checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("structural arm has no authored returned value");
    };
    super::guarded::validate_structural_return(
        checked,
        plan,
        state,
        *expression,
        u32::try_from(ordinal)
            .map_err(|_| LoweringError::Unsupported("structural return coordinate overflow"))?,
        operation,
    )
}

/// A graph guard names exactly the value its `Guard` coordinate retains: the
/// pure Boolean expression with its source binding, or the unique Boolean
/// computation root this machine owns there. Emission evaluates that root once
/// through the shared computation expander, which rejoins its source custody.
/// A terminal machine call on one conditional arm: the arm's own authored
/// invocation of another machine (Terminal calls in transition arms,
/// state_contracts.md), run on that arm alone before its completion's ordinary
/// return disposal. Alternative arms may transfer the same inherited owner:
/// exact authored target custody and the selected-arm call namespace keep those
/// transfers distinct, while ordinary call admission and Terminal claim replay
/// require each executed transfer to consume its current owner exactly once. Its
/// normal result is the machine's: Unit, or the primitive scalar the call
/// binds at the arm's transition, which the arm returns. Emission rejoins
/// that binding's ordinal in the namespace the arm inherits. Checked bodies
/// and selected requirements share this completion contract; ordinary call
/// admission below retains their distinct target and argument custody.
pub(super) fn validate_arm_call(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    transition: &checked_trees::statement::TableTransition,
    ordinal: usize,
    operands: &checked_trees::CheckedOperandSequence,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let Some(call) = operation.call() else {
        return unsupported("Unit graph call arm lost its call");
    };
    let completes = match (&plan.result, call.result()) {
        (
            checked_trees::CheckedControlResultPlan::Unit,
            checked_trees::CheckedUnitCallResult::Unit,
        ) => true,
        (
            checked_trees::CheckedControlResultPlan::Scalar { primitive_type },
            checked_trees::CheckedUnitCallResult::Scalar(result),
        ) => result.primitive_type == *primitive_type && result.statement_index as usize == ordinal,
        (
            checked_trees::CheckedControlResultPlan::Structural(signature),
            checked_trees::CheckedUnitCallResult::Structural {
                binding,
                discard_result_on_return,
            },
        ) => {
            binding.type_identity == signature.type_identity
                && binding.multiplicity == signature.multiplicity
                && binding.statement_index as usize == ordinal
                && !discard_result_on_return
        }
        _ => false,
    };
    if !completes
        || call.coordinate.statement_index as usize != ordinal
        || transition.exit != TransitionExit::Ordinary
        || transition.continuation.is_valid()
        || !matches!(
            checked.statement_table.transition_target(transition.target),
            checked_trees::statement::TransitionTargetNode::Named { .. }
        )
    {
        return unsupported("Unit graph call arm is not its arm's terminal invocation");
    }
    crate::unit::attached_unit::argument_schedule::validate_operand_preparation(
        checked,
        plan.machine,
        state.state,
        operands,
        operation,
    )?;
    for operand in operands.iter() {
        match operand {
            checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {}
            checked_trees::CheckedUnitEffectOperationPlan::EstablishScalarArray {
                source,
                result,
                elements,
            } => {
                crate::unit::attached_unit::scalar_arrays::validate(
                    checked,
                    plan.machine,
                    state.state,
                    *source,
                    result,
                    elements,
                )?;
            }
            // The shared call admission handles calls in this sequence with
            // their complete preceding result namespace.
            _ => {}
        }
    }
    edges::return_discards(checked, plan.machine, source, state)?;
    Ok(())
}

fn validate_guard(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    ordinal: usize,
    guard: &checked_trees::CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    let statement = u32::try_from(ordinal)
        .map_err(|_| LoweringError::Unsupported("Unit graph ordinal overflow"))?;
    let role = CheckedScalarExpressionRole::Guard;
    let values = &checked.facts.values;
    let pure = values
        .scalar_expressions
        .expression_at(state, statement, role);
    match guard {
        checked_trees::CheckedCallScalarArgument::Pure(expression) => {
            if pure != Some(expression) {
                return unsupported("Unit graph guard disagrees with checked expression");
            }
            let (_, _) = values
                .scalar_expressions
                .bound_expression_at(state, statement, role)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph guard has no exact source binding",
                ))?;
            if expression.primitive_type() != Some(PrimitiveType::Bool) {
                return unsupported("Unit graph guard is not Boolean");
            }
        }
        checked_trees::CheckedCallScalarArgument::Computation(handle) => {
            if pure.is_some() {
                return unsupported("Unit graph guard computation competes with a pure guard");
            }
            let computations = &values.scalar_computations;
            let root =
                computations
                    .root_at(state, statement, role)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph guard has no unique computation root",
                    ))?;
            if root.machine != machine
                || root.root != *handle
                || !computations.nodes.is_valid(*handle)
            {
                return unsupported("Unit graph guard disagrees with its computation root");
            }
            if computations.nodes.get(*handle).primitive_type != PrimitiveType::Bool {
                return unsupported("Unit graph guard computation is not Boolean");
            }
        }
    }
    Ok(())
}

/// A borrowed formal's admission reads its plan row: qualifications must be
/// supplied by the invocation or forwarded unchanged, and the loan must be an
/// unrestricted, non-provider view. Its binding mutability must match the
/// loan's access: the graph has no route for rebinding a `mut` formal that
/// holds a shared loan.
#[allow(clippy::too_many_arguments)]
fn borrowed_parameter(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
    state_index: usize,
    parameter_index: usize,
    qualified_invocation: bool,
    source: &checked_trees::signature::StateParameter,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
) -> Result<(), LoweringError> {
    if !parameter.qualifications.is_empty()
        && !(state_index == 0 && qualified_invocation)
        && !forwards_qualified_parameter(checked, plan, live, state_index, parameter_index)
    {
        return unsupported(
            "Unit graph borrowed parameter qualifications differ from invocation custody",
        );
    }
    let exclusive = matches!(
        parameter.access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
            | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    if source.is_mutable != exclusive {
        return unsupported(
            "Unit graph borrowed parameter binding mutability is not its loan access",
        );
    }
    if parameter.multiplicity != Multiplicity::Unrestricted
        || parameter.fused_service_erasure.is_some()
    {
        return unsupported("Unit graph borrowed parameter carries owned or provider custody");
    }
    Ok(())
}

/// Qualified invocation inputs already have their caller-established domains.
/// Only their original entry uses that custody; every actual incoming edge,
/// including entry reentry, still needs separately supported domain transport.
/// A shared loan freezes its referent. Mutable bounded byte owners retain only
/// primitive byte predicates whose preservation every write independently
/// checks; arbitrary mutable domain transport remains unsupported. A successor's
/// qualified parameter is admitted only when every live edge into its
/// state forwards a source parameter with exactly the same carrier, access,
/// multiplicity and qualifications, or, for a shared byte view, establishes
/// an authored literal under those qualifications: each is then a
/// byte-predicate domain without establishment routes, which Terminal
/// verification decides from the literal's octets. The entry keeps its
/// invocation rule.
fn forwards_qualified_parameter(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
    target_index: usize,
    parameter_index: usize,
) -> bool {
    let Some(target) = plan.states.get(target_index) else {
        return false;
    };
    let Some(parameter) = target.structural_parameters.get(parameter_index) else {
        return false;
    };
    if !(parameter.access == checked_trees::CheckedStructuralAccess::SharedBorrow
        || (matches!(
            parameter.access,
            checked_trees::CheckedStructuralAccess::MutableBorrow
                | checked_trees::CheckedStructuralAccess::Owned
        ) && parameter.projected_qualifications.is_empty()
            && checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .any(|declaration| {
                    declaration.identity == parameter.type_identity
                        && matches!(
                            declaration.shape,
                            checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                })
            && parameter.qualifications.iter().all(|qualification| {
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .structural_domains
                    .iter()
                    .any(|domain| {
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
    let byte_predicate_domains = parameter.qualifications.iter().all(|qualification| {
        checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_domains
            .iter()
            .any(|domain| {
                domain.domain == *qualification
                    && domain.carrier_type_identity == parameter.type_identity
                    && domain.byte_predicate.is_some()
                    && domain.establishment_routes.is_empty()
            })
    });
    let literal_establishes_qualifications = parameter.access
        == checked_trees::CheckedStructuralAccess::SharedBorrow
        && parameter.projected_qualifications.is_empty()
        && byte_predicate_domains
        && checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.identity == parameter.type_identity
                    && matches!(
                        declaration.shape,
                        checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                            checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                        )
                    )
            });
    plan.states
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
                            && match &transfer.source {
                                checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral { .. } => {
                                    literal_establishes_qualifications
                                }
                                checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                                    source.structural_parameters.get(*index as usize).is_some_and(|forwarded| {
                                        forwarded.type_identity == parameter.type_identity
                                            && forwarded.access == parameter.access
                                            && forwarded.multiplicity == parameter.multiplicity
                                            && forwarded.qualifications == parameter.qualifications
                                            && forwarded.projected_qualifications
                                                == parameter.projected_qualifications
                                    })
                                }
                                // A projected loan or copy forwards custody of
                                // storage declared under the same base identity
                                // and access: the subject's admission already
                                // proved the declared type match, and byte-
                                // predicate qualifications re-verify on the
                                // storage at the edge either way.
                                checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. }
                                | checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue { subject } => {
                                    subject.type_identity == parameter.type_identity
                                        && subject.access == parameter.access
                                }
                                _ => false,
                            }
                    })
                })
        })
}

fn invocation_only_entry(plan: &CheckedComposedUnitControlMachinePlan, live: &[bool]) -> bool {
    plan.states.first().is_some_and(|entry| {
        !plan
            .states
            .iter()
            .zip(live)
            .filter(|(_, live)| **live)
            .flat_map(|(state, _)| state.successors())
            .any(|successor| successor.target_state == entry.state)
    })
}
