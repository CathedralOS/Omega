//! Exact executable-site obligations and all-path fact reconstruction.

use std::collections::BTreeMap;

use psi_core::{ContractId, EdgeId, MachineId, OperationId, Proposition};
#[cfg(test)]
use psi_core::{PropositionContext, ScalarTerm, ValueId};
use psi_proof_admission::Obligation;
use psi_terminal::{
    OperationKind, OutcomeSpecificGuard, TerminalMachine, TerminalModule, Terminator,
};
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
        let guarded_return = if let Some(clause) = machine.contract.outcome_specific_ensures.first()
        {
            Some(exact_payloadless_case_return_guard(machine).ok_or(
                ModuleError::OutcomeSpecificGuaranteeReplayUnavailable {
                    machine: machine.id,
                    obligation: clause.obligation,
                },
            )?)
        } else {
            None
        };
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
        if let Some(guarded_return) = guarded_return {
            let guarded_position_offset = machine.contract.ensures.len();
            obligations.extend(
                machine
                    .contract
                    .outcome_specific_ensures
                    .iter()
                    .enumerate()
                    .filter(|(_, clause)| {
                        clause.guard == guarded_return && clause.evidence.is_none()
                    })
                    .map(
                        |(guarded_position, clause)| ReconstructedTerminalObligation {
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
                            semantic_axioms: semantics.exit_axioms.clone(),
                            canonical_certificate: false,
                        },
                    ),
            );
        }
    }
    Ok(ReconstructedTerminalObligationSet { obligations })
}

/// Recognize only the first bounded executable guarded-result carrier. Wider
/// structural control, calls, payloads, and multiple ordinary exits remain
/// fail closed until their case-conditioned replay is implemented.
pub(super) fn exact_payloadless_case_return_guard(
    machine: &TerminalMachine,
) -> Option<OutcomeSpecificGuard> {
    let [block] = machine.blocks.as_slice() else {
        return None;
    };
    let [operation] = block.operations.as_slice() else {
        return None;
    };
    let Terminator::ReturnStructural { source, .. } = block.terminator else {
        return None;
    };
    let result = machine.result.structural()?;
    let operation_result = operation.result.structural()?;
    let OperationKind::EstablishPayloadlessCase { result_case } = operation.kind else {
        return None;
    };
    if operation_result.place != source
        || operation_result.structural_type != result.structural_type
        || !operation_result.claims.is_empty()
        || !operation_result.qualifications.is_empty()
    {
        return None;
    }
    Some(OutcomeSpecificGuard {
        result_type: result.structural_type,
        result_case,
    })
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

    // Result-content equalities become true only when an exact structural
    // return edge transfers the corresponding live claims.
    let base_axioms = Vec::new();
    let mut incoming = BTreeMap::<_, Vec<Vec<Proposition>>>::new();
    incoming.insert(machine.entry, vec![base_axioms]);
    let mut exits = Vec::<Vec<Proposition>>::new();
    let mut operation_obligations = Vec::new();
    for current in machine_flow::deterministic_block_order(machine) {
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
            &mut operation_obligations,
        );
    }
    Ok(ReconstructedMachineSemantics {
        operation_obligations,
        exit_axioms: machine_flow::guaranteed_exit_facts(exits),
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
