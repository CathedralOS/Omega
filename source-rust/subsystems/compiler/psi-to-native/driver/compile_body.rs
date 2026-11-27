//! Run stages 03 through 15 for application entries and private callables.
//! Callers bind measured handlers between frame planning and instruction encoding.
//! Validators can request the original stage inputs for independent replay.

use crate::driver::observations::{NativePipelineObservation, NativePipelineStage};
use artifact_lowering::VerifiedNativeArtifactInput;
use function_layout::FunctionRelativeOptimizationRealizationError;
use instruction_selection::OptimizedSelectionPipelineError;
use machine::OptimizedPostAllocationMachinePipelineError;
use optimization_core::{OptimizationWorkBudget, PostTerminalOptimizationSelections};
use selected_rewrites::SelectedInstructionOptimizationError;
use target_lowering::{NativeBodyPlan, OptimizedTargetLoweringRequest};

#[derive(Debug)]
pub enum NativeBodyPlanningError {
    AbstractOptimization(abstract_optimization::AbstractOptimizationError),
    Target(target_lowering::LoweringError),
}

pub fn plan_native_body(
    input: VerifiedNativeArtifactInput,
    target: OptimizedTargetLoweringRequest<'_>,
    selections: &PostTerminalOptimizationSelections,
    budget: OptimizationWorkBudget,
    observation: &NativePipelineObservation,
    retain_original: bool,
) -> Result<NativeBodyPlan, NativeBodyPlanningError> {
    observation.enter(NativePipelineStage::AbstractOptimization);
    let optimized = abstract_optimization::optimize_abstract_operations(
        input.into_optimization_input(),
        selections.selections(),
        budget,
    )
    .map_err(NativeBodyPlanningError::AbstractOptimization)?;
    observation.enter(NativePipelineStage::AbstractToTarget);
    target_lowering::lower_native_body(optimized.into_lowering_input(), target, retain_original)
        .map_err(NativeBodyPlanningError::Target)
}

struct PhysicalPlanningOriginals {
    selected_optimization: Option<selected_rewrites::SelectedOptimizationReplay>,
    allocation: allocation::RetainedAllocation,
    machine: machine::StagedOptimizedPostAllocationMachinePlan,
}

#[derive(Debug)]
pub(crate) enum OptimizedVerifiedPhysicalPipelineError {
    SelectedOptimization(SelectedInstructionOptimizationError),
    RegisterEnvironment(register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    RegisterAllocation(allocation::RegisterAllocationError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    Encoding(native_encoding::OptimizedSelectedFormEncodingError),
    Layout(placement::LayoutError),
    FunctionRelativeRealization(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedVerifiedPhysicalPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("common physical staging failed: ")?;
        match self {
            Self::SelectedOptimization(error) => {
                write!(formatter, "SelectedOptimization({error:?})")
            }
            Self::RegisterEnvironment(error) => write!(formatter, "RegisterEnvironment({error:?})"),
            Self::Selection(error) => write!(formatter, "Selection({error:?})"),
            Self::RegisterAllocation(error) => write!(formatter, "RegisterAllocation({error:?})"),
            Self::PostAllocationMachine(error) => {
                write!(formatter, "PostAllocationMachine({error:?})")
            }
            Self::Encoding(error) => write!(formatter, "Encoding({error:?})"),
            Self::Layout(error) => write!(formatter, "Layout({error:?})"),
            Self::FunctionRelativeRealization(error) => {
                write!(formatter, "FunctionRelativeRealization({error:?})")
            }
        }
    }
}

impl std::error::Error for OptimizedVerifiedPhysicalPipelineError {}

pub(crate) type PreparedPhysicalBody = PhysicalBody<entry_planning::imports::PreparedFunctionBody>;
pub(crate) type BoundPhysicalBody = PhysicalBody<installation_binding::BoundFunctionBody>;

pub(crate) struct PhysicalBody<Stage> {
    pub stage: Stage,
    originals: Option<PhysicalPlanningOriginals>,
}

impl PreparedPhysicalBody {
    pub(crate) fn bind(
        self,
        handlers: &installation_binding::RetainedHandlerMeasurements,
        root: Option<installation_conditions::InstalledPhysicalStackRoot>,
    ) -> Result<BoundPhysicalBody, Vec<diagnostics::Diagnostic>> {
        let stage = installation_binding::bind_function_body(self.stage, handlers, root)
            .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        Ok(PhysicalBody {
            stage,
            originals: self.originals,
        })
    }
}

pub(crate) fn prepare_physical_body(
    target: NativeBodyPlan,
    entry_execution: Option<provider_admission::AdmittedInstalledEntryExecution>,
    requirements: Vec<installation_conditions::InstalledInterruptStackRequirement>,
    observation: &NativePipelineObservation,
) -> Result<PreparedPhysicalBody, BodyEmissionError> {
    let body_error =
        |reason| BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)]);
    observation.enter(NativePipelineStage::InstructionSelection);
    let selected = instruction_selection::select_instructions(
        instruction_selection::SelectionInput {
            body: target,
            entry_execution,
            stack: requirements,
        },
    )
    .map_err(|error| match error {
        OptimizedSelectionPipelineError::RegisterEnvironment(reason) => {
            OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment(reason).into()
        }
        OptimizedSelectionPipelineError::BodyBindings(reason) => body_error(reason),
        OptimizedSelectionPipelineError::Installation(reason) => {
            BodyEmissionError::Installation(reason)
        }
        other => OptimizedVerifiedPhysicalPipelineError::Selection(other).into(),
    })?;
    observation.enter(NativePipelineStage::SelectedOptimization);
    let optimized = selected_rewrites::optimize_selected_instructions(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedOptimization)?;
    observation.enter(NativePipelineStage::RegisterAllocation);
    let (allocation, selected_optimization) = allocation::stage_register_allocation(optimized)
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)?;
    observation.enter(NativePipelineStage::PostAllocationMachine);
    let machine = machine::stage_optimized_post_allocation_machine_plan(&allocation)
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    let (frame, original_machine) =
        frame_planning::plan_function_frame(machine).map_err(|error| match error {
            frame_planning::PhysicalFrameError::CalleeSaveStorage(error) => {
                FunctionRelativeOptimizationRealizationError::CalleeSaveStorage(error)
            }
            frame_planning::PhysicalFrameError::FrameLayout(error) => {
                FunctionRelativeOptimizationRealizationError::FrameLayout(error)
            }
            frame_planning::PhysicalFrameError::FrameProtocol(error) => {
                FunctionRelativeOptimizationRealizationError::FrameProtocol(error)
            }
        })?;
    let originals = original_machine.map(|machine| PhysicalPlanningOriginals {
        selected_optimization,
        allocation,
        machine,
    });
    let stage = entry_planning::imports::plan_function_body(frame).map_err(body_error)?;
    Ok(PreparedPhysicalBody { stage, originals })
}

pub(crate) struct EncodedBody {
    program: native_encoding::StagedOptimizedSelectedFormEncoding<
        machine_code::layout::EncodedBodyPublication<installation_binding::BoundBodyPublication>,
    >,
    originals: Option<PhysicalPlanningOriginals>,
}

#[cfg(test)]
impl EncodedBody {
    pub(crate) fn original_encoding_weak(
        &self,
    ) -> std::sync::Weak<machine_code::SelectedFormEncoding> {
        std::sync::Arc::downgrade(&self.program.current().encoding)
    }
}

pub(crate) struct LaidOutBody {
    originals: Option<Box<published_object_verification::RetainedPhysicalInputs>>,
    placed: function_layout::FunctionPlacement<
        machine_code::layout::EncodedBodyPublication<installation_binding::BoundBodyPublication>,
    >,
}

pub(crate) fn encode_physical_body(
    prepared: BoundPhysicalBody,
    observation: &NativePipelineObservation,
) -> Result<EncodedBody, BodyEmissionError> {
    let BoundPhysicalBody { stage, originals } = prepared;
    observation.enter(NativePipelineStage::SelectedFormEncoding);
    let program =
        native_encoding::encode_bound_function_body(stage).map_err(|error| match error {
            native_encoding::FunctionBodyEncodingError::Encoding(error) => {
                OptimizedVerifiedPhysicalPipelineError::Encoding(error).into()
            }
            native_encoding::FunctionBodyEncodingError::BodyContracts(reason) => {
                BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)])
            }
            native_encoding::FunctionBodyEncodingError::FrameProtocol(error) => {
                BodyEmissionError::from(
                    FunctionRelativeOptimizationRealizationError::FrameProtocol(error),
                )
            }
            native_encoding::FunctionBodyEncodingError::WindowsInternet(reason) => {
                BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)])
            }
        })?;
    Ok(EncodedBody { program, originals })
}

pub(crate) fn layout_encoded_body(
    encoded: EncodedBody,
    observation: &NativePipelineObservation,
) -> Result<LaidOutBody, BodyEmissionError> {
    let EncodedBody { program, originals } = encoded;
    observation.enter(NativePipelineStage::ResolvedLayout);
    let originals = originals.map(|earlier| (earlier, program.current().clone()));
    let placement::LayoutResolution { layout: output, baseline_for_audit } = placement::resolve_program(program)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Layout)?;
    let retained = match (originals, baseline_for_audit) {
        (Some((earlier, encoding)), Some(baseline_layout)) => Some(Box::new(
            published_object_verification::RetainedPhysicalInputs {
                allocation: earlier.allocation,
                machine: earlier.machine,
                encoding,
                baseline_layout,
                selected_optimization: earlier.selected_optimization,
            },
        )),
        (None, None) => None,
        _ => return Err(BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(
            "layout and encoding disagree on independent replay custody",
        )])),
    };
    if retained
        .as_ref()
        .is_some_and(|originals| !originals.belongs_to(output.layout_for_audit()))
    {
        return Err(BodyEmissionError::Object(vec![
            diagnostics::Diagnostic::error(
                "retained physical inputs belong to another relaxed layout",
            ),
        ]));
    }
    let placed = function_layout::place_function_text(
        output,
        observation.counter(NativePipelineStage::FragmentEmission),
    )
    .map_err(|error| match error {
        function_layout::FunctionTextLayoutError::Realization(error) => {
            BodyEmissionError::from(error)
        }
        error => BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(error.to_string())]),
    })?;
    Ok(LaidOutBody {
        originals: retained,
        placed,
    })
}

pub(crate) struct BodyObject {
    pub projection: native_artifact::FragmentObjectProjection,
    pub originals: Option<Box<published_object_verification::RetainedPhysicalInputs>>,
}

pub(crate) fn construct_body_object(body: LaidOutBody) -> Result<BodyObject, BodyEmissionError> {
    let issued = object_file::publish_function_object(body.placed)
        .map_err(object_publication_error)?;
    let projection =
        native_artifact::emit_fragment_object(issued).map_err(BodyEmissionError::Object)?;
    Ok(BodyObject {
        projection,
        originals: body.originals,
    })
}

fn object_publication_error(
    error: object_file::RelocationFreeObjectContainerError,
) -> BodyEmissionError {
    BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(format!(
        "native artifact fragment object placement failed: {error}"
    ))])
}

pub(crate) enum BodyEmissionError {
    Installation(&'static str),
    Physical(OptimizedVerifiedPhysicalPipelineError),
    Object(Vec<diagnostics::Diagnostic>),
}

impl From<OptimizedVerifiedPhysicalPipelineError> for BodyEmissionError {
    fn from(error: OptimizedVerifiedPhysicalPipelineError) -> Self {
        Self::Physical(error)
    }
}

impl From<FunctionRelativeOptimizationRealizationError> for BodyEmissionError {
    fn from(error: FunctionRelativeOptimizationRealizationError) -> Self {
        Self::Physical(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization(error))
    }
}
