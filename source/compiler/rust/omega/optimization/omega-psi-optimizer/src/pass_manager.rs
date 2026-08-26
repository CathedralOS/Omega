use std::collections::BTreeSet;

use omega_optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkBudget,
};
use omega_optimization_policy::{
    BaselineDecisionLog, BaselineDecisionOutcome, BaselinePolicy, ValidatedCandidateSummary,
};
use omega_optimization_unit::PsiOptimizationUnit;
use omega_optimization_validation::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, validate_integer_evaluation_candidate,
    validate_verified_psi_optimization_unit,
};
use omega_terminal_psi_to_abstract_operations::{
    VerifiedPsiOptimizationUnit, VerifiedTerminalOptimizationInput,
};

use crate::{
    AnalysisManager, AnalysisManagerError, OrderedRuleRegistry, RuleAnalysisView, RuleProposalError,
};

#[derive(Debug)]
pub struct VerifiedPsiOptimizationSession {
    input: VerifiedTerminalOptimizationInput,
    unit: PsiOptimizationUnit,
}

impl VerifiedPsiOptimizationSession {
    pub fn new(
        verified: VerifiedPsiOptimizationUnit,
    ) -> Result<Self, OptimizationUnitValidationError> {
        validate_verified_psi_optimization_unit(&verified)?;
        let (input, unit) = verified.into_parts();
        Ok(Self { input, unit })
    }

    pub const fn input(&self) -> &VerifiedTerminalOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(self) -> (VerifiedTerminalOptimizationInput, PsiOptimizationUnit) {
        (self.input, self.unit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsiOptimizationCommit {
    pub rule: OptimizationRuleIdentity,
    pub candidate: OptimizationCandidateIdentity,
    pub validator: OptimizationValidatorIdentity,
    pub input: OptimizationUnitIdentity,
    pub output: OptimizationUnitIdentity,
    pub predicted_cost_delta: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizationRunUsage {
    pub rule_evaluations: u64,
    pub candidates: u64,
    pub validation_steps: u64,
    pub commits: u64,
    pub iterations: u64,
}

#[derive(Debug)]
pub struct OptimizationRun {
    pub session: VerifiedPsiOptimizationSession,
    pub commits: Vec<PsiOptimizationCommit>,
    pub usage: OptimizationRunUsage,
    pub decisions: BaselineDecisionLog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationRunError {
    InitialValidation(OptimizationUnitValidationError),
    Analysis(AnalysisManagerError),
    Proposal {
        rule: OptimizationRuleIdentity,
        error: RuleProposalError,
    },
    CandidateValidation(OptimizationUnitValidationError),
    WorkBudgetExhausted(&'static str),
    NonDecreasingConvergenceMeasure {
        previous: u64,
        current: u64,
    },
    RegistryCoverageMismatch,
    PolicySelectionMissing(OptimizationCandidateIdentity),
}

impl std::fmt::Display for OptimizationRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Psi optimization run failed: {self:?}")
    }
}

impl std::error::Error for OptimizationRunError {}

pub fn run_psi_registry(
    verified: VerifiedPsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    let session = VerifiedPsiOptimizationSession::new(verified)
        .map_err(OptimizationRunError::InitialValidation)?;
    let (unit, commits, usage, decisions) = run_unit(session.unit, registry, budget)?;
    Ok(OptimizationRun {
        session: VerifiedPsiOptimizationSession {
            input: session.input,
            unit,
        },
        commits,
        usage,
        decisions,
    })
}

fn run_unit(
    mut unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
) -> Result<
    (
        PsiOptimizationUnit,
        Vec<PsiOptimizationCommit>,
        OptimizationRunUsage,
        BaselineDecisionLog,
    ),
    OptimizationRunError,
> {
    let mut analyses = AnalysisManager::new(&unit);
    let mut usage = OptimizationRunUsage::default();
    let mut commits = Vec::new();
    let mut dispatched = BTreeSet::new();
    let mut policy = BaselinePolicy::default();
    loop {
        charge(&mut usage.iterations, budget.iterations(), "iterations")?;
        let previous_measure = exact_integer_operation_count(&unit);
        let mut chosen: Option<(
            omega_optimization_unit::PsiRewriteCandidate,
            ValidatedPsiRewrite,
        )> = None;
        for rule in registry.iter() {
            let contract = rule.contract();
            dispatched.insert(contract.identity());
            charge(
                &mut usage.rule_evaluations,
                budget.rule_evaluations(),
                "rule evaluations",
            )?;
            let products = analyses
                .require_all(&unit, contract.required_analyses())
                .map_err(OptimizationRunError::Analysis)?
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&products))
                .map_err(|error| OptimizationRunError::Proposal {
                    rule: contract.identity(),
                    error,
                })?;
            for _ in &candidates {
                charge(&mut usage.candidates, budget.candidates(), "candidates")?;
            }
            let mut validated = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                charge(
                    &mut usage.validation_steps,
                    budget.validation_steps(),
                    "validation steps",
                )?;
                let output = validate_integer_evaluation_candidate(&unit, &candidate)
                    .map_err(OptimizationRunError::CandidateValidation)?;
                validated.push((candidate, output));
            }
            if validated.is_empty() {
                continue;
            }
            let outcome = policy.choose(
                unit.identity,
                validated
                    .iter()
                    .map(|(candidate, _)| ValidatedCandidateSummary {
                        candidate: candidate.identity(),
                        predicted_cost_delta: candidate.predicted_cost_delta(),
                    }),
            );
            match outcome {
                BaselineDecisionOutcome::Choose(identity) => {
                    let index = validated
                        .iter()
                        .position(|(candidate, _)| candidate.identity() == identity)
                        .ok_or(OptimizationRunError::PolicySelectionMissing(identity))?;
                    chosen = Some(validated.swap_remove(index));
                    break;
                }
                BaselineDecisionOutcome::Skip(_) => continue,
            }
        }
        let Some((candidate, validated)) = chosen else {
            if dispatched.len() != registry.len() {
                return Err(OptimizationRunError::RegistryCoverageMismatch);
            }
            return Ok((unit, commits, usage, policy.finish()));
        };
        let input_identity = unit.identity;
        let validator = validated.validator();
        let candidate_identity = validated.candidate();
        let next = validated.into_unit();
        let current_measure = exact_integer_operation_count(&next);
        if current_measure >= previous_measure {
            return Err(OptimizationRunError::NonDecreasingConvergenceMeasure {
                previous: previous_measure,
                current: current_measure,
            });
        }
        analyses
            .commit_revision(&next, candidate.invalidated_analyses(), true)
            .map_err(OptimizationRunError::Analysis)?;
        charge(&mut usage.commits, budget.commits(), "commits")?;
        commits.push(PsiOptimizationCommit {
            rule: candidate.rule(),
            candidate: candidate_identity,
            validator,
            input: input_identity,
            output: next.identity,
            predicted_cost_delta: candidate.predicted_cost_delta(),
        });
        unit = next;
    }
}

fn charge(counter: &mut u64, limit: u64, axis: &'static str) -> Result<(), OptimizationRunError> {
    if *counter == limit {
        return Err(OptimizationRunError::WorkBudgetExhausted(axis));
    }
    *counter += 1;
    Ok(())
}

fn exact_integer_operation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerAdd {
                    ..
                } | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerSubtract { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerMultiply { .. }
            )
        })
        .count()
        .try_into()
        .expect("operation count fits u64")
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{Optimization, OptimizationSelections};
    use omega_terminal_abstract_operations::TerminalAbstractOperation;

    use super::*;
    use crate::{built_in_psi_registry, rules::tests::exact_add_unit};

    fn verified_empty_unit() -> VerifiedPsiOptimizationUnit {
        use psi_core::{BlockId, ContractId, EdgeId, MachineId};
        use psi_terminal::{
            Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule,
            Terminator, VocabularyMarker,
        };

        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(401).unwrap(),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: MachineId::new(401).unwrap(),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(402).unwrap(),
                blocks: vec![Block {
                    id: BlockId::new(402).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(403).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(404).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            }],
        };
        let semantic = psi_terminal_codec::encode_module(&module).unwrap();
        let proof =
            psi_terminal_codec::encode_proof_bundle(&psi_terminal_verifier::ProofBundle::default())
                .unwrap();
        let input =
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                &semantic,
                &proof,
                &psi_proof_admission::AdmissionProfile::default(),
            )
            .unwrap();
        omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap()
    }

    fn budget(iterations: u64) -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(16, 16, 16, 16, iterations).unwrap()
    }

    #[test]
    fn fixed_point_dispatch_validates_then_commits_with_stable_usage() {
        let unit = exact_add_unit();
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, decisions) =
            run_unit(unit.clone(), &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].input, unit.identity);
        assert_eq!(commits[0].output, output.identity);
        assert_eq!(usage.commits, 1);
        assert_eq!(usage.validation_steps, 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(usage.rule_evaluations, 2);
        assert_eq!(decisions.records.len(), 1);
        assert_eq!(
            decisions.records[0].outcome,
            BaselineDecisionOutcome::Choose(commits[0].candidate)
        );
        assert!(matches!(
            output.functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant { .. }
        ));
    }

    #[test]
    fn exhausted_iteration_budget_fails_deterministically_without_output() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let first = run_unit(exact_add_unit(), &registry, budget(1)).unwrap_err();
        let second = run_unit(exact_add_unit(), &registry, budget(1)).unwrap_err();
        assert_eq!(first, second);
        assert_eq!(
            first,
            OptimizationRunError::WorkBudgetExhausted("iterations")
        );
    }

    #[test]
    fn public_run_requires_and_retains_verified_optimizer_context() {
        let registry = OrderedRuleRegistry::new(Vec::new()).unwrap();
        let run = run_psi_registry(verified_empty_unit(), &registry, budget(2)).unwrap();
        assert!(run.commits.is_empty());
        assert_eq!(run.usage.iterations, 1);
        assert_eq!(
            run.session.unit().terminal_psi,
            run.session.input().plan().terminal_psi
        );
    }
}
