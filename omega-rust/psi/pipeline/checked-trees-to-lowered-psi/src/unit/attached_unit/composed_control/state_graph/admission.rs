//! Rejoin every retained state, call, and edge before admitting the graph.
use super::super::super::super::{CheckedComposedUnitControlTerminatorPlan, PrimitiveType};
use super::super::super::{
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, Multiplicity, ScalarType, unsupported,
};
use super::super::{CheckedTrees, LoweringError};
use super::{CheckedComposedUnitControlMachinePlan, claims, edges, scalars, topology};
use checked_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};
use checked_trees::types::TypeReferenceNode;

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
    checked.machine_type_parameters(machine).is_empty()
        // A persistent receiver remains part of every state's invocation
        // custody, even when its body only calls a compile-selected service.
        && checked.machine_states(machine).iter().all(|source| {
            !checked.state_parameters(source).iter().any(|parameter| parameter.is_self)
                || plan.states.iter().find(|state| state.state == source.symbol)
                    .is_some_and(|state| state.structural_parameters.iter().any(|parameter| parameter.is_self))
        })
        && plan.body_qualifications.is_empty()
        && {
            // Unreachable states never execute: their custody rows cannot
            // disqualify the route. Reachable positions still enforce every
            // parameter's custody shape. A missing edge target is topology
            // drift, not custody — defer to `admit`'s authored-roster checks
            // for the precise drift diagnostic.
            let Ok(live) = topology::live(plan) else {
                return true;
            };
            plan.states
                .iter()
                .zip(&live)
                .filter_map(|(state, live)| live.then_some(state))
                .all(|state| {
                    state.structural_parameters.iter().all(|parameter| {
                        ((parameter.multiplicity == Multiplicity::Unrestricted
                            && matches!(parameter.access, checked_trees::CheckedStructuralAccess::SharedBorrow | checked_trees::CheckedStructuralAccess::MutableBorrow))
                            || parameter.access == checked_trees::CheckedStructuralAccess::Owned)
                            && (parameter.qualifications.is_empty() || parameter.multiplicity == Multiplicity::Linear)
                    })
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
    pub(super) source_states: &'a [checked_trees::state::State],
    pub(super) claim_transport: claims::ClaimTransport,
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
    let source_states = checked.machine_states(machine);
    if source_states.len() != plan.states.len() {
        return unsupported("Unit graph state roster drifted");
    }
    if source_states
        .iter()
        .zip(&plan.states)
        .any(|(source, state)| source.symbol != state.state)
    {
        return unsupported("Unit graph state identity, contract, or custody drifted");
    }
    // Reachability is pure topology on the checked plan. States outside the
    // entry successor closure never execute: the drift checks below still
    // pair their authored rows, but their custody rows cannot disqualify the
    // route and emission prunes them.
    let live = topology::live(plan)?;
    for (state_index, (source, state)) in source_states.iter().zip(&plan.states).enumerate() {
        crate::unit::attached_unit::claims::validate_entry_claims(
            checked,
            plan.machine,
            source,
            &state.structural_parameters,
            &state.entry_claims,
        )?;
        if source.symbol != state.state {
            return unsupported("Unit graph state identity, contract, or custody drifted");
        }
        // `requires` rows that lower into closed scalar predicates stay on
        // the plan as retained state requires and emit as header invariants;
        // every other authored contract row is still rejected outright.
        match validation::structural_state_contract_scalar_predicates(&checked.typed, source) {
            Some(predicates)
                if predicates.len() == state.requires.len()
                    && state.requires.iter().all(Option::is_some) => {}
            _ => {
                return unsupported("structural graph state has unrepresented authored contracts");
            }
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
        // `[erased]` bindings own no plan entry; count the retained ones by
        // their typed relevance.
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
        for parameter in &state.structural_parameters {
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
            if parameter.access == checked_trees::CheckedStructuralAccess::Owned {
                if source.is_self
                    || source.is_const
                    || source.is_mutable
                    || parameter.is_self
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
                        && !validation::has_plain_owned_contents_with_numeric_constraints(
                            &checked.typed,
                            crate::unit::attached_unit::parameters::structural_carrier_type(
                                checked,
                                source.type_reference,
                            )?,
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
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = checked
                .type_reference_table
                .type_reference(source.type_reference)
            else {
                return unsupported("Unit graph structural parameter is not a borrowed view");
            };
            if parameter.is_self {
                super::parameters::validate_receiver(checked, plan, source, parameter, *access)?;
                continue;
            }
            let expected_shape = if let Some(primitive) = checked.primitive_type_reference(*referee)
            {
                if !matches!(
                    checked.type_reference_table.type_reference(*referee),
                    TypeReferenceNode::Named { .. }
                ) {
                    return unsupported("Unit graph primitive borrow has qualified storage");
                }
                checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive)
            } else {
                match checked.type_reference_table.type_reference(*referee) {
                    TypeReferenceNode::Slice { element_type } => {
                        if checked.primitive_type_reference(*element_type)
                            == Some(PrimitiveType::U8)
                        {
                            checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                                checked_trees::CheckedByteSequenceCarrier::BorrowedView,
                            )
                        } else {
                            checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView {
                                element_type_identity: checked
                                    .normalized_type_identity(*element_type)
                                    .into_string(),
                            }
                        }
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: checked_trees::types::FixedArrayLength::Literal(length),
                    } => checked_trees::CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity: checked
                            .normalized_type_identity(*element_type)
                            .into_string(),
                        length: *length as u64,
                    },
                    _ => {
                        let referent_identity =
                            checked.normalized_type_identity(*referee).into_string();
                        match checked
                            .facts
                            .flow
                            .terminal_unit_effects
                            .structural_types
                            .iter()
                            .find(|plan| plan.identity == referent_identity)
                        {
                            Some(plan) => plan.shape.clone(),
                            None => {
                                return unsupported(
                                    "Unit graph borrowed referent has no published shape",
                                );
                            }
                        }
                    }
                }
            };
            if !matches!(
                *access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) || source.is_self
                || source.is_const
                || source.is_mutable != (*access == language_core::ReferenceAccess::Mutable)
                || parameter.is_self
                || parameter.multiplicity != Multiplicity::Unrestricted
                || parameter.access
                    != match *access {
                        language_core::ReferenceAccess::Mutable => {
                            checked_trees::CheckedStructuralAccess::MutableBorrow
                        }
                        _ => checked_trees::CheckedStructuralAccess::SharedBorrow,
                    }
                || !parameter.qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
                || checked.normalized_type_identity(*referee).as_str() != parameter.type_identity
            {
                return unsupported("Unit graph view type or authority disagrees with source");
            }
            let mut shapes = checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .filter(|shape| shape.identity == parameter.type_identity);
            if shapes.next().map(|shape| &shape.shape) != Some(&expected_shape)
                || shapes.next().is_some()
            {
                return unsupported("Unit graph borrow has no exact referent shape");
            }
        }
        let statements = checked.statement_table.statements(source.statement_nodes);
        scalars::validate(checked, state)?;
        let terminator_ordinal = super::body::validate(checked, plan.machine, source, state)?;
        let tail = &statements[terminator_ordinal..];
        match (&state.terminator, tail) {
            (CheckedComposedUnitControlTerminatorPlan::Guarded { .. }, _) => {
                super::guarded::validate(checked, plan, source, state, terminator_ordinal)?;
            }
            (CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }, _) => {
                super::cases::validate(checked, plan, source, state, tail, terminator_ordinal)?;
            }
            (CheckedComposedUnitControlTerminatorPlan::ReturnUnit, [])
                if plan.result == checked_trees::CheckedControlResultPlan::Unit =>
            {
                edges::return_discards(checked, plan.machine, source, state)?;
            }
            (CheckedComposedUnitControlTerminatorPlan::ReturnScalar { completion }, _) => {
                super::returns::validate_scalar(
                    checked,
                    plan,
                    source,
                    state,
                    completion,
                    terminator_ordinal,
                )?;
                edges::return_discards(checked, plan.machine, source, state)?;
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
            ) => {
                super::returns::validate(checked, plan, source, state, terminator_ordinal)?;
            }
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
                edges::validate_fallback(checked, state.state, terminator_ordinal, false_source)?;
                if when_true.statement_ordinal as usize != terminator_ordinal {
                    return unsupported("Unit graph guard drifted from its authored ordinal");
                }
                validate_guard(
                    checked,
                    plan.machine,
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
                edges::validate_fallback(checked, state.state, terminator_ordinal, false_source)?;
                validate_guard(
                    checked,
                    plan.machine,
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
                let arm_ordinal = match return_arm {
                    checked_trees::CheckedConditionalReturnArm::Structural(
                        checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            result,
                            ..
                        },
                    ) => result.statement_index as usize,
                    // A scalar arm returns the value checking retained under its
                    // own `Return` role: the machine's result is that scalar and
                    // the authored arm names a value, not a state.
                    checked_trees::CheckedConditionalReturnArm::Scalar {
                        statement_ordinal,
                        primitive_type,
                    } if plan.result
                        == checked_trees::CheckedControlResultPlan::Scalar {
                            primitive_type: *primitive_type,
                        }
                        && matches!(
                            checked
                                .statement_table
                                .transition_target(return_source.target),
                            checked_trees::statement::TransitionTargetNode::Value(_)
                        ) =>
                    {
                        *statement_ordinal as usize
                    }
                    _ => {
                        return unsupported(
                            "Unit graph conditional return arm is neither a structural value producer nor the scalar result",
                        );
                    }
                };
                if jump.statement_ordinal as usize != jump_ordinal || arm_ordinal != return_ordinal
                {
                    return unsupported(
                        "Unit graph conditional return drifted from its authored ordinals",
                    );
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
            (CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback }, tail)
                if tail.len() == arms.len() + 1
                    && tail[..arms.len()].iter().all(|statement| {
                        matches!(statement, StatementNode::Transition(transition)
                        if matches!(transition.guard, TransitionGuardNode::When(_)))
                    })
                    && matches!(tail.last(), Some(StatementNode::Transition(transition))
                    if transition.guard == TransitionGuardNode::Always) =>
            {
                // The shared scalar tail roster owns this chain's guard order
                // and selected destinations; every arm must rejoin it exactly.
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
                if exits.len() != arms.len() {
                    return unsupported("guarded jump successors drifted from their roster");
                }
                for (index, (arm, exit)) in arms.iter().zip(exits.iter()).enumerate() {
                    let ordinal = terminator_ordinal + index;
                    let statement_ordinal = u32::try_from(ordinal)
                        .map_err(|_| LoweringError::Unsupported("Unit graph ordinal overflow"))?;
                    let StatementNode::Transition(transition) = &tail[index] else {
                        return unsupported("guarded jump arm lost its authored transition");
                    };
                    let checked_trees::CheckedScalarBranchDestination::Jump(selected) =
                        &exit.destination
                    else {
                        return unsupported("guarded jump arm is not a named-state edge");
                    };
                    if exit.guard_statement_ordinal != statement_ordinal
                        || selected.statement_ordinal != statement_ordinal
                        || selected.is_continuation
                        || selected.target != arm.successor.target_state
                    {
                        return unsupported("guarded jump arm drifted from the shared roster");
                    }
                    if arm.successor.statement_ordinal != statement_ordinal {
                        return unsupported("guarded jump guard drifted from its authored ordinal");
                    }
                    validate_guard(checked, plan.machine, state.state, ordinal, &arm.guard)?;
                    edges::validate(
                        checked,
                        plan,
                        source,
                        state,
                        transition,
                        &arm.successor,
                        ordinal,
                    )?;
                }
                let Some(checked_trees::CheckedScalarBranchDestination::Jump(selected)) =
                    &retained.fallback
                else {
                    return unsupported("guarded jump fallback is not a named-state edge");
                };
                let ordinal = terminator_ordinal + arms.len();
                let statement_ordinal = u32::try_from(ordinal)
                    .map_err(|_| LoweringError::Unsupported("Unit graph ordinal overflow"))?;
                let Some(StatementNode::Transition(transition)) = tail.last() else {
                    return unsupported("guarded jump fallback lost its authored transition");
                };
                if selected.statement_ordinal != statement_ordinal
                    || selected.is_continuation
                    || selected.target != fallback.target_state
                {
                    return unsupported("guarded jump fallback drifted from the shared roster");
                }
                edges::validate(checked, plan, source, state, transition, fallback, ordinal)?;
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
    let claim_transport = claims::resolve(checked, plan, &live)?;
    let states = plan
        .states
        .iter()
        .zip(&live)
        .filter_map(|(state, live)| live.then_some(state))
        .collect::<Vec<_>>();
    // Internal Unit targets are admitted for their call custody here; their
    // signatures come from the closure that emits this graph.
    let (boundaries, _) =
        super::super::admission::retain_call_targets(checked, plan.machine, &states)?;
    if let Some(attachment) = attachment {
        for state in &states {
            for operation in &state.operations {
                crate::unit::attached_unit::provider_attachments::validate_call_source(
                    checked,
                    plan.machine,
                    state.state,
                    operation,
                    &plan.provider_attachment_requirements,
                )?;
            }
        }
        // Only signature-directed boundary plans (machine == state) are
        // provider obligations; a boundary-declaration plan's machine-directed
        // calls settle through that machine's own retained boundary seam.
        let called = boundaries
            .iter()
            .filter(|(boundary, _)| boundary.machine == boundary.state)
            .map(|(boundary, _)| boundary.machine)
            .collect::<Vec<_>>();
        crate::unit::attached_unit::provider_attachments::validate_provider_attachment_requirements(
            attachment,
            &plan.provider_attachment_requirements,
            &called,
        )?;
    } else if !plan.provider_attachment_requirements.is_empty() {
        return unsupported("free Unit graph cannot retain provider attachment requirements");
    }
    for (boundary, _) in &boundaries {
        if boundary
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self)
            != boundary.attachment_type_identity.is_some()
        {
            return unsupported("Unit graph boundary attachment disagrees with its receiver");
        }
        if let Some(attachment) = &boundary.attachment_type_identity
            && (!matches!(boundary.structural_parameters.as_slice(), [parameter]
                if parameter.is_self
                    && parameter.type_identity == *attachment
                    && parameter.multiplicity == Multiplicity::Linear
                    && parameter.access == checked_trees::CheckedStructuralAccess::Owned
                    && parameter.qualifications.is_empty())
                || !boundary.result.is_unit())
        {
            return unsupported(
                "Unit graph attached boundary requires exact linear receiver custody",
            );
        }
        // Domain requirements on borrowed inputs lower into the emitted
        // boundary's `requires` rows; qualified affine results mint their
        // caller-side establishments at emission.
        if !(boundary.result.is_unit()
            || matches!(
                &boundary.result,
                CheckedBoundaryMachineResultPlan::Structural {
                    multiplicity: Multiplicity::Affine,
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
        source_states,
        claim_transport,
        boundaries,
    })
}

/// A graph guard names exactly the value its `Guard` coordinate retains: the
/// pure Boolean expression with its source binding, or the unique Boolean
/// computation root this machine owns there. Emission evaluates that root once
/// through the shared computation expander, which rejoins its source custody.
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
            let (binding, _) = values
                .scalar_expressions
                .bound_expression_at(state, statement, role)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph guard has no exact source binding",
                ))?;
            crate::expression_preparation::source_custody::validate_pure(
                checked,
                binding,
                ScalarType::Boolean,
            )?;
            if !matches!(expression, CheckedScalarExpression::Boolean(_)) {
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
