use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use omega_resolver::DeclarationId;
use omega_typer::{BoundarySlot, StaticValue, StaticValueDemand, TypeError, TypingProgress};

use crate::constant_evaluation::{ConstantEvaluation, StaticDemandError};
use crate::invocation::admission::{InvocationProofs, ProviderAdmission};
use crate::invocation::static_callbacks::{CallableRefinements, CheckedCallableRefinement, CheckedCallableUse};
use crate::invocation::termination::TerminationAdmission;
use crate::owned_values::NominalAdmission;
use crate::{PreparedValue, TargetObservation, TargetSemantics};

/// Judgments for this immutable typing pass. The evidence owner binds each
/// judgment to the selected occurrence, implementation and applicable premises.
/// Empty evidence preserves every ordinary refusal.
#[derive(Default)]
pub struct CheckedPreparationEvidence<'stage, 'text> {
    pub(crate) termination: TerminationAdmission<'stage, 'text>,
    pub(crate) invocation_proofs: InvocationProofs<'stage, 'text>,
    pub(crate) providers: ProviderAdmission<'stage, 'text>,
    pub(crate) nominal: NominalAdmission<'stage, 'text>,
    pub(crate) callable_refinements: CallableRefinements<'stage, 'text>,
}

impl<'stage, 'text> CheckedPreparationEvidence<'stage, 'text> {
    /// Supply a checked precondition, crash disproof, or result guarantee.
    pub fn with_invocation_fact(
        mut self,
        judgment: crate::CheckedInvocationFact<'stage, 'text>,
    ) -> Self {
        self.invocation_proofs.facts.push(judgment);
        self
    }

    /// Supply the checked realization and trust evidence for a selected provider.
    pub fn with_provider(
        mut self,
        judgment: crate::CheckedProvider<'stage, 'text>,
    ) -> Self {
        self.providers.checked.push(judgment);
        self
    }

    /// Supply establishment of the exact typed construction and field values.
    pub fn with_data_establishment(
        mut self,
        judgment: crate::CheckedDataEstablishment<'stage, 'text>,
    ) -> Self {
        self.nominal.establishments.push(judgment);
        self
    }

    /// Supply canonical identity at the exact closed index demand.
    pub fn with_canonical_identity(
        mut self,
        judgment: crate::CheckedCanonicalIdentity<'stage, 'text>,
    ) -> Self {
        self.nominal.canonical.push(judgment);
        self
    }
}

impl<'stage, 'text> CheckedPreparationEvidence<'stage, 'text> {
    pub fn with_callable_refinement(
        mut self,
        refinement: CheckedCallableRefinement<'stage, 'text>,
    ) -> Self {
        self.callable_refinements.checked.push(refinement);
        self
    }

    /// Supply checked callable-bound premises or guarantees at one indirect call.
    pub fn with_callable_use(
        mut self,
        judgment: CheckedCallableUse<'stage, 'text>,
    ) -> Self {
        self.callable_refinements.uses.push(judgment);
        self
    }
}

#[derive(Debug)]
pub struct PreparedPass {
    pub dependencies: crate::PreparationDependencies,
    pub answers: PreparationAnswers,
    pub target_observations: Vec<TargetObservation>,
}

#[derive(Debug)]
pub enum PreparationAnswers {
    /// Apply these answers with their exact occurrences and owner arguments
    /// before asking the typer for its next set of ready demands.
    Pending(Vec<(StaticValueDemand, StaticValue)>),
    /// These identities need target inputs. Independent requests in this pass
    /// may already have answers and can advance typing before target selection.
    OpenTarget {
        demands: Vec<OpenTargetDemand>,
        answers: Vec<(StaticValueDemand, StaticValue)>,
    },
    Frozen {
        constants: HashMap<DeclarationId, PreparedValue>,
        open_target_constants: HashMap<DeclarationId, Arc<TypeError>>,
    },
}

/// A live demand and the target-dependent operation that prevented its evaluation.
#[derive(Debug)]
pub struct OpenTargetDemand {
    pub demand: StaticValueDemand,
    pub diagnostic: TypeError,
}

/// Consume the typer's actual output. An intermediate forest can execute
/// only its ready demands. Once typing completes, every local constant is
/// either frozen or retains a target-dependent recipe.
pub fn prepare_typing_pass<'stage, 'text>(
    progress: &'stage TypingProgress<'text>,
    slots: &'stage [BoundarySlot],
    target: Option<&'stage TargetSemantics>,
    evidence: CheckedPreparationEvidence<'stage, 'text>,
) -> Result<PreparedPass, TypeError> {
    evaluate_pass(progress, slots, target, evidence, &HashSet::new())
}

pub(super) fn evaluate_pass<'stage, 'text>(
    progress: &'stage TypingProgress<'text>,
    slots: &'stage [BoundarySlot],
    target: Option<&'stage TargetSemantics>,
    evidence: CheckedPreparationEvidence<'stage, 'text>,
    completed: &HashSet<StaticValueDemand>,
) -> Result<PreparedPass, TypeError> {
    let trees = match progress {
        TypingProgress::Complete(trees) => trees,
        TypingProgress::Pending(pending) => pending.trees(),
    };
    let mut evaluator = ConstantEvaluation::new(trees, slots);
    evaluator.target = target;
    evaluator.termination = evidence.termination;
    evaluator.invocation_proofs = evidence.invocation_proofs;
    evaluator.providers = evidence.providers;
    evaluator.nominal = evidence.nominal;
    evaluator.callable_refinements = evidence.callable_refinements;
    let answers = match progress {
        TypingProgress::Complete(_) => PreparationAnswers::Frozen {
            constants: evaluator.finish()?,
            open_target_constants: evaluator.open_target_constants,
        },
        TypingProgress::Pending(pending) => {
            let mut answers = Vec::new();
            let mut open_demands = Vec::new();
            for demand in pending
                .demands()
                .iter()
                .filter(|demand| !completed.contains(demand))
            {
                match evaluator.evaluate_demand(demand) {
                    Ok(value) => answers.push((demand.clone(), value)),
                    Err(StaticDemandError::OpenTarget(diagnostic)) => open_demands.push(OpenTargetDemand {
                        demand: demand.clone(),
                        diagnostic,
                    }),
                    Err(StaticDemandError::Refused(error)) => return Err(error),
                }
            }
            if open_demands.is_empty() {
                PreparationAnswers::Pending(answers)
            } else {
                PreparationAnswers::OpenTarget {
                    demands: open_demands,
                    answers,
                }
            }
        }
    };
    Ok(PreparedPass {
        dependencies: evaluator.dependencies.into_inner(),
        answers,
        target_observations: evaluator.observations.into_inner(),
    })
}
