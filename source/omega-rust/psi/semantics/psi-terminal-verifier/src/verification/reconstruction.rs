//! Exact executable-site obligations and all-path fact reconstruction.

use std::collections::BTreeMap;

use psi_core::{ContractId, EdgeId, MachineId, OperationId, Proposition};
#[cfg(test)]
use psi_core::{PropositionContext, ScalarTerm, ValueId};
use psi_proof_admission::Obligation;
use psi_terminal::{OutcomeSpecificGuard, TerminalMachine, TerminalModule};
#[cfg(test)]
use psi_terminal_semantics::CanonicalScalarGoal;

use crate::validation::exact_payloadless_case_return_exits;
use crate::{ModuleError, ValidatedInterpretableTerminalModule, validate_module};

#[cfg(test)]
mod affine_custody;
#[cfg(test)]
mod affine_selection;
#[cfg(test)]
mod alias_transport;
#[cfg(test)]
mod cast_custody;
#[cfg(test)]
mod cast_selection;
#[cfg(test)]
mod certificate_entry;
#[cfg(test)]
mod integer_evidence;
#[cfg(test)]
mod integer_selection;
mod machine_context;
mod machine_flow;
mod operation_facts;
mod path_facts;
mod terminator_facts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedOperationObligation {
    pub owner: ReconstructedTerminalObligationOwner,
    pub obligation: Obligation,
    pub semantic_axioms: Vec<Proposition>,
    /// The obligation is the operation schema's canonical kernel proposition,
    /// not a proposition selected by a trusted sufficient-form reducer.
    pub canonical_certificate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReconstructedTerminalObligationOwner {
    Operation {
        machine: MachineId,
        operation: OperationId,
    },
    CallRequires {
        machine: MachineId,
        operation: OperationId,
        requirement_position: u32,
    },
    NominalCleanupRequires {
        machine: MachineId,
        edge: EdgeId,
        cleanup_position: u32,
        requirement_position: u32,
    },
    ContractEnsures {
        machine: MachineId,
        contract: ContractId,
        clause_position: u32,
    },
}

impl ReconstructedTerminalObligationOwner {
    pub const fn machine(self) -> MachineId {
        match self {
            Self::Operation { machine, .. }
            | Self::CallRequires { machine, .. }
            | Self::NominalCleanupRequires { machine, .. }
            | Self::ContractEnsures { machine, .. } => machine,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedTerminalObligation {
    pub owner: ReconstructedTerminalObligationOwner,
    pub obligation: Obligation,
    pub requirements: Vec<Proposition>,
    pub semantic_axioms: Vec<Proposition>,
    /// The obligation is the operation schema's canonical kernel proposition,
    /// not a proposition selected by a trusted sufficient-form reducer.
    pub canonical_certificate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedTerminalObligationSet {
    obligations: Vec<ReconstructedTerminalObligation>,
}

impl ReconstructedTerminalObligationSet {
    pub fn obligations(&self) -> &[ReconstructedTerminalObligation] {
        &self.obligations
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReconstructedMachineSemantics {
    pub(super) operation_obligations: Vec<ReconstructedOperationObligation>,
    pub(super) exit_axioms: Vec<Proposition>,
    pub(super) outcome_exit_axioms: BTreeMap<OutcomeSpecificGuard, Vec<Proposition>>,
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

/// Reconstruct operation proof questions for the exact interpreter-admitted
/// subset. This does not grant the execution-grade carrier consumed by fixed
/// fuel or native lowering.
pub fn reconstruct_interpretable_operation_obligations(
    validated: ValidatedInterpretableTerminalModule<'_>,
) -> Result<Vec<ReconstructedOperationObligation>, ModuleError> {
    let module = validated.module();
    let mut obligations = Vec::new();
    for machine in &module.machines {
        obligations.extend(reconstruct_machine_semantics(module, machine)?.operation_obligations);
    }
    Ok(obligations)
}

/// Reconstruct the complete proof question for a module admitted by the
/// interpreter profile. Ranked modules use this to bind canonical artifact
/// identity without pretending that ordinary acyclic execution validation
/// accepted them.
pub fn reconstruct_interpretable_terminal_obligations(
    validated: ValidatedInterpretableTerminalModule<'_>,
) -> Result<ReconstructedTerminalObligationSet, ModuleError> {
    reconstruct_validated_terminal_obligations(validated.module())
}

/// Reconstruct the complete proof question replayed by [`crate::verify_module`].
/// Rows retain exact semantic owners, assumptions, and axiom ordering. The set
/// contains both executable-site obligations and every published contract
/// `ensures` clause; a proof bundle cannot add, remove, or retarget either.
pub fn reconstruct_terminal_obligations(
    module: &TerminalModule,
) -> Result<ReconstructedTerminalObligationSet, ModuleError> {
    validate_module(module)?;
    reconstruct_validated_terminal_obligations(module)
}

pub(super) fn reconstruct_validated_terminal_obligations(
    module: &TerminalModule,
) -> Result<ReconstructedTerminalObligationSet, ModuleError> {
    let mut obligations = Vec::new();
    for machine in &module.machines {
        let semantics = reconstruct_machine_semantics(module, machine)?;
        obligations.extend(semantics.operation_obligations.into_iter().map(|site| {
            ReconstructedTerminalObligation {
                owner: site.owner,
                obligation: site.obligation,
                requirements: machine.contract.requires.clone(),
                semantic_axioms: site.semantic_axioms,
                canonical_certificate: site.canonical_certificate,
            }
        }));
        obligations.extend(machine.contract.ensures.iter().enumerate().map(
            |(clause_position, clause)| {
                ReconstructedTerminalObligation {
                    owner: ReconstructedTerminalObligationOwner::ContractEnsures {
                        machine: machine.id,
                        contract: machine.contract.id,
                        clause_position: u32::try_from(clause_position)
                            .expect("validated contract clause position fits u32"),
                    },
                    obligation: Obligation {
                        id: clause.obligation,
                        proposition: clause.proposition.clone(),
                        class: psi_proof_admission::ObligationClass::Derivable,
                    },
                    requirements: machine.contract.requires.clone(),
                    semantic_axioms: semantics.exit_axioms.clone(),
                    canonical_certificate: false,
                }
            },
        ));
        if !machine.contract.outcome_specific_ensures.is_empty() {
            let guarded_position_offset = machine.contract.ensures.len();
            obligations.extend(
                machine
                    .contract
                    .outcome_specific_ensures
                    .iter()
                    .enumerate()
                    .filter_map(|(guarded_position, clause)| {
                        let exit_axioms = semantics.outcome_exit_axioms.get(&clause.guard)?;
                        clause
                            .evidence
                            .is_none()
                            .then(|| ReconstructedTerminalObligation {
                                owner: ReconstructedTerminalObligationOwner::ContractEnsures {
                                    machine: machine.id,
                                    contract: machine.contract.id,
                                    clause_position: u32::try_from(
                                        guarded_position_offset + guarded_position,
                                    )
                                    .expect("validated guarded contract clause position fits u32"),
                                },
                                obligation: Obligation {
                                    id: clause.obligation,
                                    proposition: clause.proposition.clone(),
                                    class: psi_proof_admission::ObligationClass::Derivable,
                                },
                                requirements: machine.contract.requires.clone(),
                                semantic_axioms: exit_axioms.clone(),
                                canonical_certificate: false,
                            })
                    }),
            );
        }
    }
    Ok(ReconstructedTerminalObligationSet { obligations })
}

/// Reconstruct facts at each executable obligation site and facts established
/// on every return path. A true conditional edge establishes the predicate
/// computed by its condition operation; edge bindings rewrite those facts to
/// successor parameters. Merge and return facts remain intersection-only.
pub(super) fn reconstruct_machine_semantics(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<ReconstructedMachineSemantics, ModuleError> {
    let context = machine_context::MachineReconstructionContext::new(module, machine)?;
    let outcome_exit_guards =
        if let Some(clause) = machine.contract.outcome_specific_ensures.first() {
            exact_payloadless_case_return_exits(machine).ok_or(
                ModuleError::OutcomeSpecificGuaranteeReplayUnavailable {
                    machine: machine.id,
                    obligation: clause.obligation,
                },
            )?
        } else {
            BTreeMap::new()
        };

    // Result-content equalities become true only when an exact structural
    // return edge transfers the corresponding live claims.
    let base_axioms = Vec::new();
    let mut incoming = BTreeMap::<_, Vec<Vec<Proposition>>>::new();
    incoming.insert(machine.entry, vec![base_axioms]);
    let mut exits = Vec::<Vec<Proposition>>::new();
    let mut outcome_exits = BTreeMap::<OutcomeSpecificGuard, Vec<Vec<Proposition>>>::new();
    let mut operation_obligations = Vec::new();
    let ranked_backedges = machine
        .ranked_scc
        .iter()
        .flat_map(|component| component.covered_cyclic_edges.iter().map(|row| row.edge))
        .collect();
    for current in machine_flow::deterministic_block_order(machine, &ranked_backedges) {
        let block = context
            .blocks
            .get(&current)
            .expect("validated module contains every reached block");
        let mut axioms = machine_flow::take_guaranteed_incoming(&mut incoming, current);
        for operation in &block.operations {
            operation_facts::append_operation(
                module,
                machine,
                operation,
                &context.machines,
                &context.value_types,
                &context.proposition_context,
                &context.machine_parameter_values,
                &mut axioms,
                &mut operation_obligations,
            )?;
        }
        terminator_facts::append_terminator(
            &block.terminator,
            machine,
            &context.blocks,
            &context.machines,
            &|id| context.value_term(id),
            context.reconstruct_path_facts,
            axioms,
            &mut incoming,
            &mut exits,
            outcome_exit_guards.get(&current).copied(),
            &mut outcome_exits,
            &mut operation_obligations,
            &ranked_backedges,
        );
    }
    Ok(ReconstructedMachineSemantics {
        operation_obligations,
        exit_axioms: machine_flow::guaranteed_exit_facts(exits),
        outcome_exit_axioms: outcome_exits
            .into_iter()
            .map(|(guard, exits)| (guard, machine_flow::guaranteed_exit_facts(exits)))
            .collect(),
    })
}

/// Test entry for canonical goals whose proof closes without value-root
/// custody or operation-definition authority.
#[cfg(test)]
fn canonical_goal_has_closed_prior_certificate(
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    certificate_entry::retained(None, goal, semantic_axioms, requirements)
}

/// The complete prior-fact exact divide/remainder families whose canonical
/// proofs need only exact citations, closed integer order, substitution, and
/// transitivity.
#[cfg(test)]
fn exact_division_has_closed_prior_certificate(
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    canonical_goal_has_closed_prior_certificate(goal, semantic_axioms, requirements)
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
