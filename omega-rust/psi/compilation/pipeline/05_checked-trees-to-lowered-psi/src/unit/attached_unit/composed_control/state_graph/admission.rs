//! Admit a composed graph from its plan: the shapes its emitter lowers,
//! each state's parameter custody, claim transport across edges, and the
//! boundaries its calls name. Stage 04 resolved every terminator and edge
//! from the authored transitions, so admission does not rejoin them to
//! source; a closed-sum dispatch alone still reads its subject's declaration
//! and arms (`cases::validate`).
use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, Multiplicity, unsupported,
};
use super::super::{CheckedTrees, LoweringError};
use super::{
    CheckedComposedUnitControlMachinePlan, claims, edges, entry_aliases, exclusive_handoffs,
    topology,
};

/// Custody, not topology, selects this emitter. Once selected, failed source
/// rejoining must not retry an older matcher that ignores additional statements.
pub(in crate::unit::attached_unit::composed_control) fn has_shared_graph_custody(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
) -> bool {
    let Some(machine) = checked.machine(plan.machine) else {
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

/// Every claimed destination path must have an authenticated incoming move.
/// Its local binding name need not equal the source's claim identity.
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
    claims::resolve(checked, plan, live, &[]).is_ok()
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
    /// One map per plan state; a present entry binds that successor
    /// structural parameter to `states[0].structural_parameters[root]`: the
    /// parameters that only ever receive one whole claim-bearing entry
    /// parameter, the entry's whole receiver or a whole entry loan
    /// (`entry_aliases`).
    pub(super) entry_aliases: Vec<std::collections::BTreeMap<u32, u32>>,
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
    // The producer assigns scalar qualifications to scalar computations and
    // retains only structural requests here. Reclassifying every committed
    // domain as structural would reject those supported scalar operations.
    if plan.states.is_empty() || !plan.body_qualifications.is_empty() {
        return unsupported("Unit graph has unsupported qualification or provider custody");
    }
    let machine = checked
        .machine(plan.machine)
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
    super::ranking::require_witness_ranks(machine, plan)?;
    super::super::admission::reject_attachment(plan)?;
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
                    Some(checked_trees::CheckedUnitStructuralTypeShape::BindingCarrier)
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
                    || parameter.binding_carrier.is_some()
                    || validation::structural_result_qualifications(
                        &checked.typed,
                        source.type_reference,
                    )
                    .ok()
                    .as_ref()
                        != Some(&parameter.qualifications)
                    || checked.type_multiplicity(source.type_reference) != parameter.multiplicity
                    || validation::stored_structural_type_identity(
                        &checked.typed,
                        source.type_reference,
                        &[],
                        &[],
                    ) != parameter.type_identity
                    || (parameter.multiplicity != Multiplicity::Linear
                        && !validation::has_contents(
                            &checked.typed,
                            source.type_reference, validation::ContentsRequirement::LinearOwned)
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
                        && !crate::reference_custody::is_reference_record(
                            checked,
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
        if let CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. } = state.terminator {
            // A closed-sum dispatch checks its subject's sum custody and arms
            // against what the case emitter lowers (`cases::validate`).
            super::cases::validate(checked, plan, source, state)?;
        } else {
            for successor in state.successors() {
                let target =
                    plan.state(successor.target_state)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph successor target is missing",
                        ))?;
                edges::admit_transfers(state, successor, target)?;
            }
        }
    }
    // Dead states are authored and shape-checked above but never execute:
    // their parameter, claim, and call rows have no producing edge, so the
    // emitted graph prunes to the entry state's successor closure.
    // A whole entry parameter proven to arrive unchanged on every edge keeps
    // its invocation place: a claim-bearing owner instead of a block
    // parameter, a loan instead of duplicating its authority.
    let mut entry_aliases = vec![std::collections::BTreeMap::new(); plan.states.len()];
    entry_aliases::alias_forwarded_entry_claims(plan, &live, &mut entry_aliases)?;
    entry_aliases::alias_entry_parameter_loans(plan, &live, &mut entry_aliases)?;
    let claim_transport = claims::resolve(checked, plan, &live, &entry_aliases)?;
    // A shared parameter that only ever receives its source's whole
    // exclusive loan holds that loan; Terminal edges transfer mutable names.
    let exclusive_handoffs =
        exclusive_handoffs::promote_shared_handoffs(checked, plan, &live, &entry_aliases)?;
    let states = plan
        .states
        .iter()
        .zip(&live)
        .filter_map(|(state, live)| live.then_some(state))
        .collect::<Vec<_>>();
    // Every live state's calls take the one call admission; internal Unit
    // targets' signatures come from the closure that emits this graph.
    let boundaries = super::super::admission::admit_calls(checked, &states)?;
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
                    .data_definition_by_symbol(owner.attached_data_symbol)
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
        entry_aliases,
        exclusive_handoffs,
        boundaries,
    })
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
    if parameter.multiplicity != Multiplicity::Unrestricted || parameter.binding_carrier.is_some() {
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
    checked_trees::forwards_qualified_parameter(
        &plan.states,
        live,
        target_index,
        parameter_index,
        |identity| {
            checked_trees::checked_structural_type_by_identity(
                &checked.facts.flow.terminal_unit_effects.structural_types,
                identity,
            )
            .map(|declaration| &declaration.shape)
        },
        |qualification, identity| {
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_domains
                .iter()
                .any(|domain| {
                    domain.domain == qualification
                        && domain.carrier_type_identity == identity
                        && domain.byte_predicate.is_some()
                        && domain.establishment_routes.is_empty()
                })
        },
    )
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
