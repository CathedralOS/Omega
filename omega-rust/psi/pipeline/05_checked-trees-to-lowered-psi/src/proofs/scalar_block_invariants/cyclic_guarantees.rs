//! Strengthen a cyclic header so a published guarantee proves after the loop.
//!
//! A value-returning cycle returns a header-carried value, while the machine's
//! `ensures` relates the result to its invocation formals. The ordinary roster
//! proposes closed entry bounds, storage bounds and acyclic joins; the relation
//! between a loop-carried value and an immutable formal is a cyclic header
//! invariant no other pass proposes, so the guarantee obligation stays open and
//! the machine fails to lower. This pass runs only when a guarantee of a cyclic
//! machine cannot be proved from the retained roster, and proposes three
//! families at the header the entry edge binds from the formals: every entry
//! requirement conjunct rewritten over the bound header parameters, `header ==
//! formal` for every parameter each in-component arrival forwards unchanged,
//! and each guarantee transported through the exit's exact equations into
//! header scope. These are proposals only: every arrival is proved before
//! retention, and the whole strengthening is discarded when any header fails
//! or the guarantee still does not prove, so a module the pass cannot help
//! keeps its original roster unchanged. No ranking premise is used; the claim
//! is checked against the loop, never inherited from a termination certificate.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, MachineId, ObligationId, Proposition, PropositionContext, ScalarTerm, ValueId,
};
use terminal_psi::{
    ScalarBlockInvariant, ScalarBlockInvariantArrival, TerminalMachine, TerminalModule, Terminator,
};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

use super::joins::{ArrivalEquations, scoped_goal, uses_header};
use super::{header_arrival_edges, retained_evidence};
use crate::lowering_error::LoweringError;
use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
use lowered_psi::LoweredPsi;

/// Shared finite budget for equation transport and candidate discovery.
const BUDGET: usize = 4096;

pub(super) fn strengthen(
    lowered: &mut LoweredPsi,
    retained: Option<&ReconstructedTerminalObligationSet>,
) -> Result<(), LoweringError> {
    // The roster owner hands over its own reconstruction when it left the
    // module exactly as it reconstructed it; otherwise this pass takes one.
    let reconstructed;
    let original = match retained {
        Some(original) => original,
        None => {
            reconstructed =
                terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
                    .map_err(LoweringError::InvalidTerminalModule)?;
            &reconstructed
        }
    };
    let (demanding, candidates) = {
        let module = &lowered.semantic_module;
        // The roster owner validated the module before this pass; a shape the
        // verifier refuses is not turned into a new program restriction here.
        let Ok(validated) = terminal_verifier::validate_module(module) else {
            return Ok(());
        };
        let demanding = unproved_cyclic_guarantees(module, validated, original);
        let mut remaining = BUDGET;
        let mut candidates = Vec::new();
        for machine in module
            .machines
            .iter()
            .filter(|machine| demanding.contains(&machine.id))
        {
            let Ok(context) = validated.value_context(machine) else {
                continue;
            };
            candidates.extend(header_candidates(
                machine,
                &context,
                original,
                &mut remaining,
            ));
        }
        (demanding, candidates)
    };
    if candidates.is_empty() {
        return Ok(());
    }
    let baseline = lowered.semantic_module.scalar_block_invariants.clone();
    if merge(&mut lowered.semantic_module, candidates)? && proves(lowered, original, &demanding)? {
        return Ok(());
    }
    lowered.semantic_module.scalar_block_invariants = baseline;
    Ok(())
}

/// Machines with a cycle whose guarantee the retained roster cannot prove.
fn unproved_cyclic_guarantees(
    module: &TerminalModule,
    validated: terminal_verifier::ValidatedTerminalModule<'_>,
    questions: &ReconstructedTerminalObligationSet,
) -> BTreeSet<MachineId> {
    let mut demanding = BTreeSet::new();
    for site in questions.obligations() {
        let ReconstructedTerminalObligationOwner::ContractEnsures { machine, .. } = site.owner
        else {
            continue;
        };
        if demanding.contains(&machine) {
            continue;
        }
        let Some(owner) = module.machines.iter().find(|owner| owner.id == machine) else {
            continue;
        };
        let Ok(components) = terminal_verifier::control_cycle_members(owner) else {
            continue;
        };
        if components.is_empty() {
            continue;
        }
        let Ok(context) = validated.value_context(owner) else {
            continue;
        };
        let parameters = owner
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        if produce_checked_canonical_integer_proof(
            &context,
            &site.obligation.proposition,
            &site.requirements,
            &site.semantic_axioms,
            &parameters,
        )
        .is_none()
        {
            demanding.insert(machine);
        }
    }
    demanding
}

/// Proposals at the cyclic header the machine entry binds from its formals.
fn header_candidates(
    machine: &TerminalMachine,
    context: &PropositionContext,
    questions: &ReconstructedTerminalObligationSet,
    remaining: &mut usize,
) -> Vec<ScalarBlockInvariant> {
    let mut candidates = Vec::new();
    let Some(entry) = machine
        .blocks
        .iter()
        .find(|block| block.id == machine.entry)
    else {
        return candidates;
    };
    if !entry.operations.is_empty() {
        return candidates;
    }
    let Terminator::Jump {
        target, arguments, ..
    } = &entry.terminator
    else {
        return candidates;
    };
    let Ok(components) = terminal_verifier::control_cycle_members(machine) else {
        return candidates;
    };
    let Some(component) = components
        .iter()
        .find(|component| component.contains(target))
    else {
        return candidates;
    };
    let Some(header) = machine.blocks.iter().find(|block| block.id == *target) else {
        return candidates;
    };
    if header.parameters.len() != arguments.len() {
        return candidates;
    }
    let formals = machine
        .parameters
        .iter()
        .map(|formal| (formal.id, *formal))
        .collect::<BTreeMap<_, _>>();
    // Formal -> the header parameter the entry edge binds from it.
    let mut bound = BTreeMap::new();
    for (parameter, argument) in header.parameters.iter().zip(arguments) {
        if let Some(formal) = formals.get(argument)
            && formal.scalar_type == parameter.scalar_type
        {
            bound.entry(*argument).or_insert(*parameter);
        }
    }
    if bound.is_empty() {
        return candidates;
    }
    let Ok(scope) = PropositionContext::from_value_types(
        machine
            .parameters
            .iter()
            .chain(&header.parameters)
            .map(|value| (value.id, value.scalar_type)),
    ) else {
        return candidates;
    };
    let mut propose = |predicate: Proposition| {
        if let Ok(predicate) = super::canonical_predicate(predicate)
            && predicate != Proposition::Truth
        {
            candidates.push(ScalarBlockInvariant {
                machine: machine.id,
                header: header.id,
                predicate,
                arrivals: Vec::new(),
            });
        }
    };

    // (1) Entry requirements over the header parameters they bind: the
    // invocation's requires generalized to every iteration, proposed only.
    let binding_equations = bound
        .iter()
        .map(|(formal, parameter)| {
            Proposition::Equal(
                ScalarTerm::value(*formal, parameter.scalar_type),
                ScalarTerm::value(parameter.id, parameter.scalar_type),
            )
        })
        .collect::<Vec<_>>();
    let unbound_scope = header
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .chain(
            formals
                .keys()
                .copied()
                .filter(|formal| !bound.contains_key(formal)),
        )
        .collect::<BTreeSet<_>>();
    if let Some(equations) = ArrivalEquations::new(&binding_equations, remaining) {
        let mut pending = machine.contract.requires.iter().collect::<Vec<_>>();
        while let Some(requirement) = pending.pop() {
            let Some(next) = remaining.checked_sub(1) else {
                return candidates;
            };
            *remaining = next;
            if let Proposition::Conjunction(members) = requirement {
                pending.extend(members);
                continue;
            }
            if !requirement.any_value_id(|value| bound.contains_key(&value)) {
                continue;
            }
            if let Some(predicate) = scoped_goal(
                requirement,
                &equations,
                &unbound_scope,
                &scope,
                context,
                None,
                remaining,
            ) {
                propose(predicate);
            }
        }
    }

    // (2) A parameter every in-component arrival forwards unchanged still
    // equals the formal the entry bound it from.
    let in_component_arrivals = component
        .iter()
        .filter_map(|member| machine.blocks.iter().find(|block| block.id == *member))
        .flat_map(|block| successor_arguments(&block.terminator, header.id))
        .collect::<Vec<_>>();
    if !in_component_arrivals.is_empty() {
        for (position, parameter) in header.parameters.iter().enumerate() {
            let formal = arguments[position];
            if bound.get(&formal).map(|parameter| parameter.id) != Some(parameter.id) {
                continue;
            }
            if in_component_arrivals
                .iter()
                .all(|arrival| arrival.get(position) == Some(&parameter.id))
                && let Ok(equality) = crate::proofs::contract_predicates::canonical_equality(
                    ScalarTerm::value(parameter.id, parameter.scalar_type),
                    ScalarTerm::value(formal, parameter.scalar_type),
                )
            {
                propose(equality);
            }
        }
    }

    // (3) Each guarantee carried back through the exit's exact equations to
    // the values alive at this header.
    let header_scope = formals
        .keys()
        .copied()
        .chain(header.parameters.iter().map(|parameter| parameter.id))
        .collect::<BTreeSet<_>>();
    for site in questions.obligations().iter().filter(|site| {
        matches!(
            site.owner,
            ReconstructedTerminalObligationOwner::ContractEnsures { machine: owner, .. }
                if owner == machine.id
        )
    }) {
        let Some(next) = remaining.checked_sub(1) else {
            return candidates;
        };
        *remaining = next;
        let Some(equations) = ArrivalEquations::new(&site.semantic_axioms, remaining) else {
            continue;
        };
        if let Some(predicate) = scoped_goal(
            &site.obligation.proposition,
            &equations,
            &header_scope,
            &scope,
            context,
            None,
            remaining,
        ) && uses_header(&predicate, header)
        {
            propose(predicate);
        }
    }
    candidates
}

fn successor_arguments(terminator: &Terminator, header: BlockId) -> Vec<&[ValueId]> {
    match terminator {
        Terminator::Jump {
            target, arguments, ..
        } if *target == header => vec![arguments.as_slice()],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .filter(|successor| successor.target == header)
            .map(|successor| successor.arguments.as_slice())
            .collect(),
        _ => Vec::new(),
    }
}

/// Join the proposals into the roster: one conjunction per header, existing
/// rows extended in place, new rows given one arrival per actual edge. Returns
/// false when no obligation identity remains for a new arrival.
fn merge(
    module: &mut TerminalModule,
    mut candidates: Vec<ScalarBlockInvariant>,
) -> Result<bool, LoweringError> {
    candidates.sort_by_key(|candidate| (candidate.machine, candidate.header));
    let mut grouped: Vec<ScalarBlockInvariant> = Vec::new();
    for candidate in candidates {
        if let Some(previous) = grouped.last_mut()
            && (previous.machine, previous.header) == (candidate.machine, candidate.header)
        {
            previous.predicate = crate::proofs::contract_predicates::connective(
                previous.predicate.clone(),
                candidate.predicate,
                true,
            )?;
        } else {
            grouped.push(candidate);
        }
    }
    let mut next = terminal_verifier::maximum_registered_obligation_id(module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    for mut candidate in grouped {
        if let Some(existing) = module.scalar_block_invariants.iter_mut().find(|existing| {
            (existing.machine, existing.header) == (candidate.machine, candidate.header)
        }) {
            existing.predicate = crate::proofs::contract_predicates::connective(
                existing.predicate.clone(),
                candidate.predicate,
                true,
            )?;
            continue;
        }
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == candidate.machine)
            .ok_or(LoweringError::Unsupported(
                "guarantee candidate lost its machine",
            ))?;
        for edge in header_arrival_edges(machine, candidate.header) {
            let Some(fresh) = next.checked_add(1) else {
                return Ok(false);
            };
            next = fresh;
            candidate.arrivals.push(ScalarBlockInvariantArrival {
                edge,
                obligation: ObligationId::new(next).ok_or(LoweringError::Unsupported(
                    "scalar invariant obligation identity is zero",
                ))?,
            });
        }
        module.scalar_block_invariants.push(candidate);
    }
    module
        .scalar_block_invariants
        .sort_by_key(|candidate| (candidate.machine, candidate.header));
    Ok(true)
}

/// Every arrival of the strengthened roster and every demanding guarantee
/// proves, and previously supplied certificates still replay.
fn proves(
    lowered: &LoweredPsi,
    original: &ReconstructedTerminalObligationSet,
    demanding: &BTreeSet<MachineId>,
) -> Result<bool, LoweringError> {
    let module = &lowered.semantic_module;
    let Ok(validated) = terminal_verifier::validate_module(module) else {
        return Ok(false);
    };
    let questions = terminal_verifier::reconstruct_terminal_obligations(module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut contexts = BTreeMap::new();
    for site in questions.obligations() {
        let machine = match site.owner {
            ReconstructedTerminalObligationOwner::ScalarBlockInvariant { machine, .. } => machine,
            ReconstructedTerminalObligationOwner::ContractEnsures { machine, .. }
                if demanding.contains(&machine) =>
            {
                machine
            }
            _ => continue,
        };
        let (context, parameters) = match contexts.entry(machine) {
            std::collections::btree_map::Entry::Occupied(prepared) => prepared.into_mut(),
            std::collections::btree_map::Entry::Vacant(slot) => {
                let owner = validated
                    .machine(machine)
                    .ok_or(LoweringError::Unsupported(
                        "guarantee proof owner disappeared",
                    ))?;
                let context = validated
                    .value_context(owner)
                    .map_err(LoweringError::InvalidTerminalModule)?;
                let parameters = owner
                    .parameters
                    .iter()
                    .map(|parameter| parameter.id)
                    .collect::<BTreeSet<_>>();
                slot.insert((context, parameters))
            }
        };
        if produce_checked_canonical_integer_proof(
            context,
            &site.obligation.proposition,
            &site.requirements,
            &site.semantic_axioms,
            parameters,
        )
        .is_none()
        {
            return Ok(false);
        }
    }
    retained_evidence::replays(module, original, &questions, &lowered.proof_bundle)
}
