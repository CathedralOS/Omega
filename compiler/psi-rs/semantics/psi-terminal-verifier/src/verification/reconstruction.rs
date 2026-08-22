//! Exact executable-site obligations and all-path fact reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{Proposition, PropositionContext, ScalarTerm, ValueId};
use psi_proof_kernel::Obligation;
use psi_terminal::{OperationKind, TerminalMachine, TerminalModule, Terminator};
#[cfg(test)]
use psi_terminal_semantics::CanonicalScalarGoal;

use crate::{ModuleError, validate_module};

mod affine_custody;
mod affine_selection;
mod alias_transport;
mod cast_custody;
mod cast_selection;
mod certificate_entry;
mod integer_evidence;
mod integer_selection;
mod operation_facts;
mod path_facts;
mod terminator_facts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedOperationObligation {
    pub obligation: Obligation,
    pub semantic_axioms: Vec<Proposition>,
    /// The obligation is the operation schema's canonical kernel proposition,
    /// not a proposition selected by a trusted sufficient-form reducer.
    pub canonical_certificate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReconstructedMachineSemantics {
    pub(super) operation_obligations: Vec<ReconstructedOperationObligation>,
    pub(super) exit_axioms: Vec<Proposition>,
}

/// Reconstruct proof obligations owned by executable operation sites. This is
/// exposed so a producer can build a certificate against exactly the same
/// source-independent obligations and axiom ordering that the verifier will
/// later replay. The module is validated before any reconstruction occurs.
pub fn reconstruct_operation_obligations(
    module: &TerminalModule,
) -> Result<Vec<ReconstructedOperationObligation>, ModuleError> {
    validate_module(module)?;
    let mut obligations = Vec::new();
    for machine in &module.machines {
        obligations.extend(reconstruct_machine_semantics(module, machine)?.operation_obligations);
    }
    Ok(obligations)
}

/// Reconstruct facts at each executable obligation site and facts established
/// on every return path. A true conditional edge establishes the predicate
/// computed by its condition operation; edge bindings rewrite those facts to
/// successor parameters. Merge and return facts remain intersection-only.
pub(super) fn reconstruct_machine_semantics(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<ReconstructedMachineSemantics, ModuleError> {
    let reconstruct_path_facts = machine.blocks.iter().any(|block| {
        block.operations.iter().any(|operation| {
            matches!(
                &operation.kind,
                OperationKind::Call { .. }
                    | OperationKind::CallUnit { .. }
                    | OperationKind::IntegerExactCast { .. }
                    | OperationKind::ExactIntegerShiftLeft { .. }
                    | OperationKind::ExactIntegerShiftRight { .. }
                    | OperationKind::ExactIntegerAdd { .. }
                    | OperationKind::ExactIntegerSubtract { .. }
                    | OperationKind::ExactIntegerMultiply { .. }
                    | OperationKind::ExactIntegerDivide { .. }
                    | OperationKind::ExactIntegerRemainder { .. }
                    | OperationKind::WrappingIntegerDivide { .. }
                    | OperationKind::WrappingIntegerRemainder { .. }
                    | OperationKind::SaturatingIntegerDivide { .. }
                    | OperationKind::SaturatingIntegerRemainder { .. }
            )
        })
    });
    let value_types = machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|declaration| (declaration.id, declaration.scalar_type))
        .collect::<BTreeMap<_, _>>();
    let proposition_context = PropositionContext::from_value_types(
        value_types
            .iter()
            .map(|(&id, &scalar_type)| (id, scalar_type)),
    )
    .map_err(ModuleError::MalformedProposition)?;
    let machine_parameter_values = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let value_term = |id: ValueId| {
        ScalarTerm::value(
            id,
            *value_types
                .get(&id)
                .expect("validated module contains every referenced value"),
        )
    };

    // Result-content equalities become true only when an exact structural
    // return edge transfers the corresponding live claims.
    let base_axioms = Vec::new();
    let mut successors = BTreeMap::<_, Vec<_>>::new();
    let mut indegree = machine
        .blocks
        .iter()
        .map(|block| (block.id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for block in &machine.blocks {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for target in &targets {
            *indegree
                .get_mut(target)
                .expect("validated target has an indegree") += 1;
        }
        successors.insert(block.id, targets);
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(machine.blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in &successors[&block] {
            let count = indegree
                .get_mut(target)
                .expect("validated target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }

    let mut incoming = BTreeMap::<_, Vec<Vec<Proposition>>>::new();
    incoming.insert(machine.entry, vec![base_axioms]);
    let mut exits = Vec::<Vec<Proposition>>::new();
    let mut operation_obligations = Vec::new();
    for current in order {
        let block = blocks
            .get(&current)
            .expect("validated module contains every reached block");
        let paths = incoming
            .remove(&current)
            .expect("validated reachable block has incoming facts");
        let mut paths = paths.into_iter();
        let mut axioms = paths.next().expect("block has an incoming path");
        for path in paths {
            axioms.retain(|fact| path.contains(fact));
        }
        for operation in &block.operations {
            operation_facts::append_operation(
                module,
                machine,
                operation,
                &machines,
                &value_types,
                &proposition_context,
                &machine_parameter_values,
                &mut axioms,
                &mut operation_obligations,
            )?;
        }
        terminator_facts::append_terminator(
            &block.terminator,
            machine,
            &blocks,
            &machines,
            &value_term,
            reconstruct_path_facts,
            axioms,
            &mut incoming,
            &mut exits,
            &mut operation_obligations,
        );
    }
    let mut exits = exits.into_iter();
    let Some(mut guaranteed) = exits.next() else {
        return Ok(ReconstructedMachineSemantics {
            operation_obligations,
            exit_axioms: Vec::new(),
        });
    };
    for exit in exits {
        guaranteed.retain(|fact| exit.contains(fact));
    }
    Ok(ReconstructedMachineSemantics {
        operation_obligations,
        exit_axioms: guaranteed,
    })
}

/// The complete prior-fact exact divide/remainder families whose canonical
/// proofs need only exact citations, closed integer order, substitution, and
/// transitivity. No value-root custody or operation-definition authority
/// participates.
#[cfg(test)]
fn exact_division_has_closed_prior_certificate(
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    certificate_entry::retained(None, goal, semantic_axioms, requirements)
}

#[cfg(test)]
fn exact_division_has_prior_certificate(
    context: &PropositionContext,
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    certificate_entry::retained(Some(context), goal, semantic_axioms, requirements)
}

#[cfg(test)]
mod tests;
