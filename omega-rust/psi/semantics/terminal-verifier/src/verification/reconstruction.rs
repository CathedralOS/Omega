//! Exact executable-site obligations and all-path fact reconstruction.

use std::collections::BTreeMap;

use proof_admission::Obligation;
use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, OperationId, Proposition};
use terminal_psi::{OutcomeSpecificGuard, TerminalMachine, TerminalModule, Terminator};

use crate::validation::exact_payloadless_case_return_exits;
use crate::{ModuleError, ValidatedInterpretableTerminalModule, validate_module};

mod crash_field_origins;
mod crash_paths;
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
    pub(super) crash_sites: Vec<ReconstructedCrashSiteFacts>,
}

/// Private source-independent facts at an exact crash terminator. Asserted
/// site guards are proof goals, never premises in this collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReconstructedCrashSiteFacts {
    pub(crate) machine: MachineId,
    pub(crate) block: BlockId,
    pub(crate) edge: EdgeId,
    pub(crate) semantic_axioms: Vec<Proposition>,
}

/// The caller must have completed structural, control-flow and policy
/// validation. This bridge deliberately does not call `validate_module`:
/// validation consumes these facts before granting its result carrier.
pub(crate) fn reconstruct_validated_crash_site_facts(
    module: &TerminalModule,
) -> Result<Vec<ReconstructedCrashSiteFacts>, ModuleError> {
    let mut sites = Vec::new();
    for machine in module.machines.iter().filter(|machine| {
        machine.blocks.iter().any(|block| {
            matches!(&block.terminator, Terminator::Crash { site_guard, .. } if !site_guard.is_empty())
        })
    }) {
        if machine.ranked_scc.is_some() {
            sites.extend(reconstruct_machine_semantics_with_crash_facts(module, machine, true)?.crash_sites);
        } else {
            sites.extend(crash_paths::reconstruct(module, machine)?);
        }
    }
    Ok(sites)
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

/// Preserve the complete proof question, including ordered assumptions, across
/// an optimization. This consumes structural validation, not execution authority.
pub fn reconstruct_optimizable_terminal_obligations(
    validated: crate::ValidatedOptimizableTerminalModule<'_>,
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
                        class: proof_admission::ObligationClass::Derivable,
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
                                    class: proof_admission::ObligationClass::Derivable,
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
    reconstruct_machine_semantics_with_crash_facts(module, machine, false)
}

fn reconstruct_machine_semantics_with_crash_facts(
    module: &TerminalModule,
    machine: &TerminalMachine,
    crash_facts: bool,
) -> Result<ReconstructedMachineSemantics, ModuleError> {
    let context = machine_context::MachineReconstructionContext::new(module, machine, crash_facts);
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
    let mut crash_sites = Vec::new();
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
        if crash_facts {
            axioms.retain(|proposition| {
                crash_field_origins::retains_entry_meaning(proposition, machine)
            });
        }
        for operation in &block.operations {
            operation_facts::append_operation(
                module,
                machine,
                operation,
                &context.machines,
                &context.value_types,
                &mut axioms,
                &mut operation_obligations,
            )?;
            if crash_facts {
                // Do not let a current mutable-field observation become an
                // entry fact or feed a later derived crash-path predicate.
                axioms.retain(|proposition| {
                    crash_field_origins::retains_entry_meaning(proposition, machine)
                });
            }
        }
        terminator_facts::append_terminator(
            &block.terminator,
            current,
            machine,
            &context.blocks,
            &context.machines,
            &|id| context.value_term(id),
            context.reconstruct_path_facts,
            crash_facts,
            axioms,
            &mut incoming,
            &mut exits,
            outcome_exit_guards.get(&current).copied(),
            &mut outcome_exits,
            &mut operation_obligations,
            &mut crash_sites,
            &ranked_backedges,
        );
    }
    Ok(ReconstructedMachineSemantics {
        operation_obligations,
        crash_sites,
        exit_axioms: machine_flow::guaranteed_exit_facts(exits),
        outcome_exit_axioms: outcome_exits
            .into_iter()
            .map(|(guard, exits)| (guard, machine_flow::guaranteed_exit_facts(exits)))
            .collect(),
    })
}
