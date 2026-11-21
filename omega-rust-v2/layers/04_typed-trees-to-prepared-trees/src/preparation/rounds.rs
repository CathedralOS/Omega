//! A preparation round evaluates L3’s ready requests and supplies the answers to the next typing pass.
//! Begin at `PreparationRound::begin`. The package entry runs `finish_without_external_evidence`
//! until `apply` publishes the prepared trees or suspends for a target.

use super::typing_pass;

#[cfg(test)]
mod tests;

use crate::evaluation::refusal;
use crate::reasons::{PreparationReason, PreparationRefusal, RefusalCause};
use crate::{
    CheckedPreparationEvidence, OpenTargetDemand, PreparationAnswers, PreparationDependencies, PreparationResourceReceipt, PreparedPass, PreparedTrees,
    TargetObservation, TargetSemantics,
};
use omega_typed::TypeError;
use omega_typed::{SpecializedIndexValue, StaticValue, StaticValueDemand, StaticValueRequest};
use omega_typer::{PendingTyping, SourcePackage, TypingEnvironment, TypingProgress, type_source_package_for_preparation};
use std::collections::HashSet;
use std::sync::Arc;

/// A package can export its typed recipes while target-dependent identities
/// remain unsettled. Only the complete variant can be handed to lowering.
#[derive(Debug)]
#[allow(clippy::large_enum_variant, reason = "completed preparation already owns the typed forest")]
pub enum PreparationProgress<'text> {
    Complete(PreparedTrees<'text>),
    OpenTarget(OpenTargetPreparation<'text>),
}

/// The exact admitted sources and live static demands for a package whose
/// type identities still depend on target selection.
#[derive(Debug)]
pub struct OpenTargetPreparation<'text> {
    source: SourcePackage<'text>,
    boundary_slots: Vec<omega_typed::BoundarySlot>,
    pending: PendingTyping<'text>,
    demands: Vec<OpenTargetDemand>,
    state: PreparationState,
}

impl<'text> OpenTargetPreparation<'text> {
    pub fn source(&self) -> &SourcePackage<'text> {
        &self.source
    }

    pub fn demands(&self) -> &[OpenTargetDemand] {
        &self.demands
    }

    /// Only the listed ready demands and their dependencies have completed
    /// typing. Consumers can inspect those recipes before selecting a target.
    pub fn pending_typing(&self) -> &PendingTyping<'text> {
        &self.pending
    }

    pub fn evaluated_indices(&self) -> &[(StaticValueRequest, StaticValue)] {
        &self.state.evaluated_indices
    }

    pub fn specialized_indices(&self) -> &[SpecializedIndexValue] {
        &self.state.specialized_indices
    }

    /// Reads made while computing settled answers or reaching an open demand.
    pub fn dependencies(&self) -> &PreparationDependencies {
        &self.state.dependencies
    }

    /// Resume the original typing pass with the selected target. Previously
    /// settled answers remain attached to their checked source occurrences.
    pub fn close(
        self,
        target: &TargetSemantics,
    ) -> Result<PreparedTrees<'text>, PreparationFailure> {
        PreparationRound::from_progress(
            &self.source,
            &self.boundary_slots,
            Some(target),
            self.state,
            TypingProgress::Pending(self.pending),
        )
        .map_err(PreparationFailure::Preparation)?
        .finish_without_external_evidence()?
        .into_complete()
    }
}

impl<'text> PreparationProgress<'text> {
    /// The first open demand is the target input whose absence stopped preparation.
    pub(super) fn into_complete(self) -> Result<PreparedTrees<'text>, PreparationFailure> {
        match self {
            Self::Complete(prepared) => Ok(prepared),
            Self::OpenTarget(open) => Err(PreparationFailure::missing_target(open
                .demands
                .into_iter()
                .next()
                .expect("open package has a target demand"))),
        }
    }
}

#[derive(Debug, Default)]
struct PreparationState {
    resource_receipt: Option<PreparationResourceReceipt>,
    target_observations: Vec<TargetObservation>,
    dependencies: PreparationDependencies,
    completed: HashSet<StaticValueDemand>,
    evaluated_indices: Vec<(StaticValueRequest, StaticValue)>,
    specialized_indices: Vec<SpecializedIndexValue>,
}

impl PreparationState {
    fn type_package<'text>(
        &self,
        source: &SourcePackage<'text>,
        boundary_slots: &[omega_typed::BoundarySlot],
    ) -> Result<TypingProgress<'text>, TypeError> {
        let environment = TypingEnvironment {
            evaluated_indices: &self.evaluated_indices,
            specialized_indices: &self.specialized_indices,
            boundary_slots,
            ..TypingEnvironment::default()
        };
        type_source_package_for_preparation(source, &environment)
    }
}

/// Typing may refuse both before and after evaluation. Keep its diagnostic
/// attached to the producer even when the caller requested a prepared package.
/// `Typing` is the entry pass, before any static answer exists. `ResumedTyping`
/// is a retype that consumed answers this layer evaluated, so L4 was reached
/// even though L3 owns the refusal.
#[derive(Debug, Eq, PartialEq)]
pub enum PreparationFailure {
    Typing(TypeError),
    ResumedTyping(TypeError),
    Preparation(PreparationRefusal),
    MissingTarget(PreparationRefusal),
}

impl PreparationFailure {
    pub fn diagnostic(&self) -> &TypeError {
        match self {
            Self::Typing(error) | Self::ResumedTyping(error) => error,
            Self::Preparation(refusal) | Self::MissingTarget(refusal) => &refusal.diagnostic,
        }
    }

    /// L3 owns typing refusals. L4 carries the cause for evaluation and target failures.
    pub fn cause(&self) -> Option<RefusalCause> {
        match self {
            Self::Typing(_) | Self::ResumedTyping(_) => None,
            Self::Preparation(refusal) | Self::MissingTarget(refusal) => Some(refusal.cause),
        }
    }

    pub fn reason(&self) -> Option<PreparationReason> {
        self.cause().and_then(RefusalCause::reason)
    }

    fn missing_target(demand: OpenTargetDemand) -> Self {
        Self::MissingTarget(PreparationRefusal { diagnostic: demand.diagnostic, cause: demand.cause })
    }
}

impl From<PreparationFailure> for TypeError {
    fn from(failure: PreparationFailure) -> Self {
        match failure {
            PreparationFailure::Typing(error) | PreparationFailure::ResumedTyping(error) => error,
            PreparationFailure::Preparation(refusal) | PreparationFailure::MissingTarget(refusal) => refusal.diagnostic,
        }
    }
}

/// One typing pass and the inputs under which its closed demands will run.
/// The coordinator may inspect the immutable typing output and supply
/// checked judgments for exactly that output before continuing preparation.
pub struct PreparationRound<'input, 'text> {
    source: &'input SourcePackage<'text>,
    slots: &'input [omega_typed::BoundarySlot],
    target: Option<&'input TargetSemantics>,
    state: PreparationState,
    progress: TypingProgress<'text>,
    identity: Arc<()>,
}

pub enum PreparationStep<'input, 'text> {
    Evaluate(Box<PreparationRound<'input, 'text>>),
    Finished(PreparationProgress<'text>),
}

/// Only evaluation of an actual round can create these answers.
pub struct EvaluatedPreparationRound {
    identity: Arc<()>,
    pass: PreparedPass,
}

#[derive(Debug)]
pub enum PreparationAdvanceError {
    DifferentRound,
    Refused(PreparationFailure),
}

impl<'input, 'text> PreparationRound<'input, 'text> {
    pub fn begin(
        source: &'input SourcePackage<'text>,
        slots: &'input [omega_typed::BoundarySlot],
        target: Option<&'input TargetSemantics>,
    ) -> Result<Self, PreparationFailure> {
        let state = PreparationState::default();
        let progress = state.type_package(source, slots).map_err(PreparationFailure::Typing)?;
        Self::from_progress(source, slots, target, state, progress).map_err(PreparationFailure::Preparation)
    }

    pub(super) fn record_resource_receipt(&mut self, receipt: PreparationResourceReceipt) {
        self.state.resource_receipt = Some(receipt);
    }

    pub(super) fn finish_without_external_evidence(mut self) -> Result<PreparationProgress<'text>, PreparationFailure> {
        loop {
            let evaluated = self
                .evaluate_with_cause(CheckedPreparationEvidence::default())
                .map_err(PreparationFailure::Preparation)?;
            match self.apply(evaluated.pass)? {
                PreparationStep::Evaluate(next) => self = *next,
                PreparationStep::Finished(product) => return Ok(product),
            }
        }
    }
    fn from_progress(
        source: &'input SourcePackage<'text>,
        slots: &'input [omega_typed::BoundarySlot],
        target: Option<&'input TargetSemantics>,
        state: PreparationState,
        progress: TypingProgress<'text>,
    ) -> Result<Self, PreparationRefusal> {
        if let (Some(catalog), Some(target)) = (source.assembly(), target)
            && &catalog.target != target.identity()
        {
            let reason = PreparationReason::AssemblyTargetMismatch;
            return Err(PreparationRefusal::rendered(
                TypeError::new(String::new(), String::new(), omega_program_contracts::SourceSpan::new(0, 0), reason.text()),
                reason,
            ));
        }
        Ok(Self {
            source,
            slots,
            target,
            state,
            progress,
            identity: Arc::new(()),
        })
    }

    pub fn typing(&self) -> &TypingProgress<'text> {
        &self.progress
    }

    pub fn evaluate<'stage>(
        &'stage self,
        evidence: CheckedPreparationEvidence<'stage, 'text>,
    ) -> Result<EvaluatedPreparationRound, TypeError> {
        self.evaluate_with_cause(evidence).map_err(TypeError::from)
    }

    /// Evaluate as `evaluate` does and keep the typed cause of a refusal.
    pub fn evaluate_with_cause<'stage>(
        &'stage self,
        evidence: CheckedPreparationEvidence<'stage, 'text>,
    ) -> Result<EvaluatedPreparationRound, PreparationRefusal> {
        let pass = typing_pass::evaluate_pass(&self.progress, self.slots, self.target, evidence, &self.state.completed)?;
        Ok(EvaluatedPreparationRound {
            identity: self.identity.clone(),
            pass,
        })
    }

    /// Resume identity-dependent typing or publish the frozen product. An
    /// answer from another forest cannot authorize this transition.
    pub fn advance(
        self,
        evaluated: EvaluatedPreparationRound,
    ) -> Result<PreparationStep<'input, 'text>, PreparationAdvanceError> {
        if !Arc::ptr_eq(&self.identity, &evaluated.identity) {
            return Err(PreparationAdvanceError::DifferentRound);
        }
        self.apply(evaluated.pass).map_err(PreparationAdvanceError::Refused)
    }

    fn apply(
        self,
        pass: PreparedPass,
    ) -> Result<PreparationStep<'input, 'text>, PreparationFailure> {
        let Self {
            source,
            slots: boundary_slots,
            target,
            mut state,
            progress,
            ..
        } = self;
        state.dependencies.extend(pass.dependencies);
        for observation in pass.target_observations {
            if !state.target_observations.contains(&observation) {
                state.target_observations.push(observation);
            }
        }
        let answers = match (progress, pass.answers) {
            (
                TypingProgress::Complete(typed),
                PreparationAnswers::Frozen {
                    constants,
                    open_target_constants,
                },
            ) => {
                // Float landing needs no target: it rounds once to the format.
                let landed_floats = crate::evaluation::floating_point::landed_floats(&typed);
                return Ok(PreparationStep::Finished(PreparationProgress::Complete(PreparedTrees::from_preparation(
                    typed,
                    constants,
                    open_target_constants,
                    state.specialized_indices,
                    state.target_observations,
                    state.resource_receipt,
                    state.dependencies,
                    landed_floats,
                ))));
            }
            (TypingProgress::Pending(pending), PreparationAnswers::Pending(answers)) => {
                if answers.is_empty() {
                    let request = pending
                        .demands()
                        .first()
                        .expect("pending typing has a demand")
                        .request();
                    return Err(PreparationFailure::Preparation(refusal(
                        &request.expression,
                        PreparationReason::TypingDidNotAdvance,
                    )));
                }
                answers
            }
            (TypingProgress::Pending(pending), PreparationAnswers::OpenTarget { demands, answers }) => {
                if answers.is_empty() {
                    // A new target changes the subject of the certified bound. A sponsored
                    // closure starts under the grant and certificate for that target.
                    if state.resource_receipt.is_some() {
                        return Err(PreparationFailure::missing_target(
                            demands
                                .into_iter()
                                .next()
                                .expect("open package has a target demand"),
                        ));
                    }
                    return Ok(PreparationStep::Finished(PreparationProgress::OpenTarget(OpenTargetPreparation {
                        source: source.clone(),
                        boundary_slots: boundary_slots.to_vec(),
                        pending,
                        demands,
                        state,
                    })));
                }
                answers
            }
            _ => unreachable!("preparation answers correspond to the input typing pass"),
        };
        for (demand, value) in answers {
            state.completed.insert(demand.clone());
            match demand {
                StaticValueDemand::Closed(request) => state.evaluated_indices.push((request, value)),
                StaticValueDemand::Specialized { request, arguments } => state
                    .specialized_indices
                    .push(SpecializedIndexValue { request, arguments, value }),
            }
        }
        // This pass consumes answers evaluated above: a refusal here is resumed typing.
        let progress = state
            .type_package(source, boundary_slots)
            .map_err(PreparationFailure::ResumedTyping)?;
        Ok(PreparationStep::Evaluate(Box::new(
            Self::from_progress(source, boundary_slots, target, state, progress).map_err(PreparationFailure::Preparation)?,
        )))
    }
}
