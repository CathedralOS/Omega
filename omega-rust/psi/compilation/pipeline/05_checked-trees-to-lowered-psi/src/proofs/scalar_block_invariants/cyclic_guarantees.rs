//! Propose contract-derived predicates at a cyclic header.
//!
//! Entry requirements, unchanged formals, conserved mathematical terms and
//! exit guarantees share the ordinary invariant roster's prove-or-drop owner.
//! A false exit-bound proposal must not discard an independently provable
//! relation needed by the loop's operations. Every retained predicate still
//! proves at every arrival, and finalization checks the actual guarantees.
//! Ranking evidence never supplies an invariant premise.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, MachineId, Proposition, PropositionContext, ScalarTerm, ValueId,
};
use terminal_psi::{ScalarBlockInvariant, TerminalMachine, TerminalModule, Terminator};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

use super::joins::{ArrivalEquations, scoped_goal, uses_header};
use crate::lowering_error::ProofSearchCapacity;
use crate::proofs::ProofSearchDeclines;
use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
mod conservation;

/// Shared finite budget for equation transport and candidate discovery.
const BUDGET: usize = ProofSearchCapacity::CyclicGuaranteeCandidateWork.limit() as usize;

pub(super) fn candidates(
    module: &TerminalModule,
    original: &ReconstructedTerminalObligationSet,
    declines: &mut ProofSearchDeclines,
) -> Vec<ScalarBlockInvariant> {
    let _scope = crate::proofs::proof_timing::Scope::enter("cyclic_guarantees::candidates");
    let Ok(validated) = terminal_verifier::validate_module(module) else {
        return Vec::new();
    };
    let demanding = unproved_cyclic_guarantees(module, validated, original, declines);
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
            declines,
        ));
    }
    if remaining == 0 {
        declines.record(ProofSearchCapacity::CyclicGuaranteeCandidateWork);
    }
    candidates
}

/// Machines with a cycle whose guarantee the retained roster cannot prove.
fn unproved_cyclic_guarantees(
    module: &TerminalModule,
    validated: terminal_verifier::ValidatedTerminalModule<'_>,
    questions: &ReconstructedTerminalObligationSet,
    declines: &mut ProofSearchDeclines,
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
            declines,
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
    declines: &mut ProofSearchDeclines,
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
                declines.record(ProofSearchCapacity::CyclicGuaranteeCandidateWork);
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

    // A mathematical expression built from invocation formals can remain
    // constant while its individual parameters change. Propose that relation
    // independently from the stronger exit guarantee; the common roster
    // discards either when one of its arrivals fails.
    for predicate in conservation::candidates(machine, &bound, remaining) {
        propose(predicate);
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
            declines.record(ProofSearchCapacity::CyclicGuaranteeCandidateWork);
            return candidates;
        };
        *remaining = next;
        // Normal-return reconstruction retains distinct alternatives when
        // exits bind the result differently. Each alternative suggests its
        // own header demand; none of its equations become invariant facts.
        // The common roster proves every proposed demand on every arrival.
        for equations in ArrivalEquations::alternatives(&site.semantic_axioms, remaining) {
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
