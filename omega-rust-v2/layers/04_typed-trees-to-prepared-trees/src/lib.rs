//! Preparation freezes local constants and computes the values needed for closed type identities.
#![forbid(unsafe_code)]

mod constant_evaluation;
mod dependencies;
pub use dependencies::{PreparationDependencies, PreparedValueOrigin, ValueInputs};
mod typing_pass;
pub use typing_pass::{CheckedPreparationEvidence, OpenTargetDemand, PreparationAnswers, PreparedPass, prepare_typing_pass};
mod materialization;
mod sponsorship;
pub use materialization::{ConstantMaterialization, ConstantMaterializationError};
pub use sponsorship::{
    CertifiedResources, CheckedPreparationBound, PreparationGrant, PreparationResourceReceipt, SponsoredPreparationError, prepare_source_package_under_grant,
};
mod target_semantics;
pub use target_semantics::{
    AddressSpaceIdentity, Endianness, EntryProfileIdentity, EntryStackGuarantee, LayoutExtent, LayoutGeometry, SelectedLayout, SelectedTargetProperty,
    TargetIdentity, TargetObservation, TargetProperty, TargetPropertyQuery, TargetSemantics, TargetSemanticsError,
};
mod floating_point;
mod integer_arithmetic;
pub use floating_point::{FloatFormat, FloatOperation, FloatRealization, FloatSemantics, PreparedFloat};
mod owned_values;
pub use owned_values::{CheckedCanonicalIdentity, CheckedDataEstablishment, MaterializationComponent, MaterializationError, PreparedValue};
mod invocation;
pub use invocation::admission::{CheckedInvocationFact, CheckedProvider, SelectedRealization};
pub use invocation::static_callbacks::{CheckedCallableRefinement, CheckedCallableUse};
pub use invocation::termination::CheckedRecursiveComponent;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use num_bigint::BigInt;
use omega_resolver::DeclarationId;
use omega_typer::{
    PendingTyping, SourcePackage, SpecializedIndexValue, StaticValue, StaticValueDemand, StaticValueRequest, TypeError, TypedTrees, TypingEnvironment,
    TypingProgress, type_source_package_for_preparation,
};

/// Typed trees whose demanded closed indices have been evaluated and materialized.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedTrees<'text> {
    typed: TypedTrees<'text>,
    constants: HashMap<DeclarationId, PreparedValue>,
    open_target_constants: HashMap<DeclarationId, Arc<TypeError>>,
    specialized_indices: Vec<SpecializedIndexValue>,
    target_observations: Vec<TargetObservation>,
    resource_receipt: Option<PreparationResourceReceipt>,
    dependencies: PreparationDependencies,
}

impl<'text> PreparedTrees<'text> {
    pub fn typed(&self) -> &TypedTrees<'text> {
        &self.typed
    }

    /// The frozen value of a local integer declaration.
    pub fn integer_constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&BigInt> {
        match self.constant(declaration)? {
            PreparedValue::Integer(value) => Some(value),
            _ => None,
        }
    }

    pub fn constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&PreparedValue> {
        self.constants.get(declaration)
    }

    pub fn dependencies(&self) -> &PreparationDependencies {
        &self.dependencies
    }

    pub fn resource_receipt(&self) -> Option<&PreparationResourceReceipt> {
        self.resource_receipt.as_ref()
    }

    /// Identify the live target dependency of an open constant. The original
    /// initializer remains a typed recipe in the typed stage.
    pub fn open_target_constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&TypeError> {
        self.open_target_constants.get(declaration).map(Arc::as_ref)
    }

    /// Target inputs used anywhere in this prepared package, including inputs
    /// that selected an overload during an earlier typing pass.
    pub fn target_observations(&self) -> &[TargetObservation] {
        &self.target_observations
    }

    /// Each answer identifies both the source operation and the owner arguments
    /// that closed it. One generic field can have a different value at each use.
    pub fn specialized_indices(&self) -> &[SpecializedIndexValue] {
        &self.specialized_indices
    }

    /// Extract the typed stage, dropping preparation results owned by this stage.
    pub fn into_typed(self) -> TypedTrees<'text> {
        self.typed
    }
}

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
    boundary_slots: Vec<omega_typer::BoundarySlot>,
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
    ) -> Result<PreparedTrees<'text>, TypeError> {
        prepare_progress(
            &self.source,
            &self.boundary_slots,
            Some(target),
            self.state,
            Some(TypingProgress::Pending(self.pending)),
        )?
        .into_complete()
    }
}

impl<'text> PreparationProgress<'text> {
    fn into_complete(self) -> Result<PreparedTrees<'text>, TypeError> {
        match self {
            Self::Complete(prepared) => Ok(prepared),
            Self::OpenTarget(open) => Err(open
                .demands
                .into_iter()
                .next()
                .expect("open package has a target demand")
                .diagnostic),
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
        boundary_slots: &[omega_typer::BoundarySlot],
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

/// Prepare until all requested identities are frozen or until the first pass
/// whose remaining ready requests need a target. Retain the open source
/// recipes rather than treating incomplete typing as a prepared package.
pub fn prepare_source_package_with_open_target<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typer::BoundarySlot],
) -> Result<PreparationProgress<'text>, TypeError> {
    prepare_progress(source, boundary_slots, None, PreparationState::default(), None)
}

/// Prepare the static values demanded by one admitted source package.
pub fn prepare_source_package<'text>(source: &SourcePackage<'text>) -> Result<PreparedTrees<'text>, TypeError> {
    prepare_source_package_with_boundary_slots(source, &[])
}

/// Use the same Build-selected boundary slots in every typing pass, including
/// the strict pass that checks the computed identities.
pub fn prepare_source_package_with_boundary_slots<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typer::BoundarySlot],
) -> Result<PreparedTrees<'text>, TypeError> {
    prepare(source, boundary_slots, None)
}

/// Prepare under the selected target, using the same layout and realization inputs across typing retries.
pub fn prepare_source_package_for_target<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typer::BoundarySlot],
    target: &TargetSemantics,
) -> Result<PreparedTrees<'text>, TypeError> {
    prepare(source, boundary_slots, Some(target))
}

fn prepare<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typer::BoundarySlot],
    target: Option<&TargetSemantics>,
) -> Result<PreparedTrees<'text>, TypeError> {
    prepare_progress(source, boundary_slots, target, PreparationState::default(), None)?.into_complete()
}

fn prepare_progress<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typer::BoundarySlot],
    target: Option<&TargetSemantics>,
    state: PreparationState,
    initial: Option<TypingProgress<'text>>,
) -> Result<PreparationProgress<'text>, TypeError> {
    let progress = match initial {
        Some(progress) => progress,
        None => state.type_package(source, boundary_slots)?,
    };
    PreparationRound::from_progress(source, boundary_slots, target, state, progress).finish_without_external_evidence()
}

/// One typing pass and the inputs under which its closed demands will run.
/// The coordinator may inspect the immutable typing output and supply
/// checked judgments for exactly that output before continuing preparation.
pub struct PreparationRound<'input, 'text> {
    source: &'input SourcePackage<'text>,
    slots: &'input [omega_typer::BoundarySlot],
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
    Typing(TypeError),
}

impl<'input, 'text> PreparationRound<'input, 'text> {
    pub fn begin(
        source: &'input SourcePackage<'text>,
        slots: &'input [omega_typer::BoundarySlot],
        target: Option<&'input TargetSemantics>,
    ) -> Result<Self, TypeError> {
        let state = PreparationState::default();
        let progress = state.type_package(source, slots)?;
        Ok(Self::from_progress(source, slots, target, state, progress))
    }

    fn finish_without_external_evidence(mut self) -> Result<PreparationProgress<'text>, TypeError> {
        loop {
            let evaluated = self.evaluate(CheckedPreparationEvidence::default())?;
            match self.apply(evaluated.pass)? {
                PreparationStep::Evaluate(next) => self = *next,
                PreparationStep::Finished(product) => return Ok(product),
            }
        }
    }
    fn from_progress(
        source: &'input SourcePackage<'text>,
        slots: &'input [omega_typer::BoundarySlot],
        target: Option<&'input TargetSemantics>,
        state: PreparationState,
        progress: TypingProgress<'text>,
    ) -> Self {
        Self {
            source,
            slots,
            target,
            state,
            progress,
            identity: Arc::new(()),
        }
    }

    pub fn typing(&self) -> &TypingProgress<'text> {
        &self.progress
    }

    pub fn evaluate<'stage>(
        &'stage self,
        evidence: CheckedPreparationEvidence<'stage, 'text>,
    ) -> Result<EvaluatedPreparationRound, TypeError> {
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
        self.apply(evaluated.pass)
            .map_err(PreparationAdvanceError::Typing)
    }

    fn apply(
        self,
        pass: PreparedPass,
    ) -> Result<PreparationStep<'input, 'text>, TypeError> {
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
                return Ok(PreparationStep::Finished(PreparationProgress::Complete(PreparedTrees {
                    typed,
                    constants,
                    open_target_constants,
                    specialized_indices: state.specialized_indices,
                    target_observations: state.target_observations,
                    resource_receipt: state.resource_receipt,
                    dependencies: state.dependencies,
                })));
            }
            (TypingProgress::Pending(pending), PreparationAnswers::Pending(answers)) => {
                if answers.is_empty() {
                    let request = pending
                        .demands()
                        .first()
                        .expect("pending typing has a demand")
                        .request();
                    return Err(refusal(&request.expression, "typing did not advance after receiving the evaluation result"));
                }
                answers
            }
            (TypingProgress::Pending(pending), PreparationAnswers::OpenTarget { demands, answers }) => {
                if answers.is_empty() {
                    // A new target changes the subject of the certified bound. A sponsored
                    // closure starts under the grant and certificate for that target.
                    if state.resource_receipt.is_some() {
                        return Err(demands
                            .into_iter()
                            .next()
                            .expect("open package has a target demand")
                            .diagnostic);
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
        let progress = state.type_package(source, boundary_slots)?;
        Ok(PreparationStep::Evaluate(Box::new(Self::from_progress(
            source,
            boundary_slots,
            target,
            state,
            progress,
        ))))
    }
}

fn refusal(
    site: &omega_typer::Site,
    reason: &str,
) -> TypeError {
    TypeError::new(
        &site.package,
        &site.file,
        site.span,
        format!("static evaluation cannot admit this invocation: {reason}"),
    )
}
