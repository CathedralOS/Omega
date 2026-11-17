/*
Sequence the native representation transformations for both application entries
and separately emitted callable bodies.

`lower_to_target` drives abstract optimization (`Omega 01`) and target lowering
(`Omega 02`), which ends by publishing the prepared execution plan. The caller
resolves the installations that plan lists (callback bodies, handler stacks)
and binds them before `emit_target` hands the plan and those bound
installations to instruction selection (`Omega 03`), selected-instruction optimization (`Omega 04`), register
allocation (`Omega 05`), post-allocation machine planning (`Omega 06`),
layout-independent encoding (`Omega 07`), resolved layout (`Omega 08`), layout
optimization (`Omega 09`), and fragment object emission.

Emission reads the current machine program plus `EmissionFacts`, the few facts
settled before instruction selection that it still needs, which `emit_target`
copies out of stage 02's output. Stage 02's output and the original
allocation, encoding and layout inputs reach emission only when a validator
request asked to retain them (`retain_native_stage_inputs`); otherwise they
are dropped as soon as their successor stage has run.
*/

use crate::observations::{NativePipelineObservation, NativePipelineStage};
use abstract_operations_to_target_operations::lower_optimized_to_target_operations;
use abstract_operations_to_target_operations::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
};
use machine_emission::{
    FunctionRelativeOptimizationRealizationError, StagedOptimizedFunctionFragmentEmissionSource,
};
use optimization_core::OptimizationExecutionPhase;
use post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding;
use register_environment::baseline_target_register_environment;
use register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError, stage_optimized_post_allocation_machine_plan,
};
use resolved_layout_to_resolved_layout::execute_resolved_layout_optimization;
use selected_form_encoding_to_resolved_layout::stage_optimized_resolved_selected_form_layout;
use selected_instruction_analyses::SelectedOptimizationRecord;
use selected_instructions_to_register_homes::stage_register_allocation;
use selected_instructions_to_selected_instructions::{
    SelectedInstructionOptimizationError, SelectedOptimizationInput, optimize_selected_instructions,
};
use target_operations_to_selected_instructions::{
    OptimizedSelectionPipelineError, SelectionInput, stage_optimized_instruction_selection,
};

pub(crate) enum TargetLoweringError {
    AbstractOptimization(abstract_operations_to_abstract_operations::AbstractOptimizationError),
    Target(abstract_operations_to_target_operations::LoweringError),
}

#[derive(Debug)]
pub(crate) enum OptimizedVerifiedPhysicalPipelineError {
    SelectedOptimization(SelectedInstructionOptimizationError),
    RegisterEnvironment(register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    RegisterAllocation(selected_instructions_to_register_homes::RegisterAllocationError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
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
            Self::FunctionRelativeRealization(error) => {
                write!(formatter, "FunctionRelativeRealization({error:?})")
            }
        }
    }
}

impl std::error::Error for OptimizedVerifiedPhysicalPipelineError {}

pub(crate) fn lower_to_target(
    input: terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    request: OptimizedTargetLoweringRequest<'_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    observation: &NativePipelineObservation,
) -> Result<ValidatedOptimizedTargetOperations, TargetLoweringError> {
    let optimized = optimize_abstract(input, selections, budget, observation)
        .map_err(TargetLoweringError::AbstractOptimization)?;
    observation.enter(NativePipelineStage::AbstractToTarget);
    let (program, lowering_facts) = optimized.into_lowering_parts();
    lower_optimized_to_target_operations(program, lowering_facts, request)
        .map_err(TargetLoweringError::Target)
}

pub(crate) fn optimize_abstract(
    input: terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    observation: &NativePipelineObservation,
) -> Result<
    abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan,
    abstract_operations_to_abstract_operations::AbstractOptimizationError,
> {
    observation.enter(NativePipelineStage::AbstractOptimization);
    let selections = selections.selections();
    abstract_operations_to_abstract_operations::optimize_abstract_operations(
        input.into_optimization_input(),
        selections,
        &selections.project_psi(),
        budget,
    )
}

/// Bind measured stacks and the checked installed entry once. Selection reads
/// the receipt's plain facts; image publication retains the same receipt.
pub(crate) fn bound_installations(
    target: &ValidatedOptimizedTargetOperations,
    execution: Option<terminal_psi_to_abstract_operations::AdmittedInstalledEntryExecution>,
    requirements: Vec<installation_evidence::InstalledInterruptStackRequirement>,
) -> Result<
    std::sync::Arc<
        terminal_psi_to_abstract_operations::provider_admission::BoundInstallationAuthority,
    >,
    &'static str,
> {
    let plan = target.execution_plan().plan();
    terminal_psi_to_abstract_operations::provider_admission::bind_installation_authority(
        bound_installations::BoundInstalledEntry {
            psi: plan.psi,
            target: plan.target,
            machine: plan.entry,
        },
        target.lowering_facts().installed_interrupts(),
        target.lowering_facts().native_callbacks(),
        execution,
        requirements,
    )
    .map(std::sync::Arc::new)
}

pub(crate) fn emit_target(
    optimized_target: ValidatedOptimizedTargetOperations,
    installations: std::sync::Arc<
        terminal_psi_to_abstract_operations::provider_admission::BoundInstallationAuthority,
    >,
    observation: &NativePipelineObservation,
    retain_native_stage_inputs: bool,
) -> Result<native_artifact::FragmentObjectProjection, BodyEmissionError> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment)?;
    let target_input = std::sync::Arc::new(optimized_target);
    let selected = {
        observation.enter(NativePipelineStage::InstructionSelection);
        let optimized = target_input.optimized();
        stage_optimized_instruction_selection(
            SelectionInput {
                plan: target_input.execution_plan(),
                installations: installations.facts(),
                optimization: optimized.identity_bundle().identity(),
                projection: optimized.projection(),
                manifest: optimized.pre_physical_manifest().identity,
                selections: optimized.selections().clone(),
                budget_per_pass: optimized.budget_per_pass(),
            },
            register_environment,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)
    }?;
    let facts = emission_facts(&target_input);
    let original_target = retain_native_stage_inputs
        .then(|| image_emission::RetainedTargetOperations::new(target_input));
    let selected = {
        observation.enter(NativePipelineStage::SelectedOptimization);
        optimize_selected_instructions(SelectedOptimizationInput {
            program: selected.selected_program(),
            custody: selected.custody().core(),
            register_environment: selected.register_environment().clone(),
            selections: selected.selections().clone(),
            budget_per_pass: selected.budget_per_pass(),
            record: SelectedOptimizationRecord::default(),
        })
        .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedOptimization)
    }?;
    // A validator request keeps Omega 04's completed run as replay evidence;
    // a default compile drops it here and allocation never sees it.
    let (selected, selected_optimization) = if retain_native_stage_inputs {
        selected.retain_replay_for_validation()
    } else {
        (selected.into_allocation_input(), None)
    };
    let allocation = {
        observation.enter(NativePipelineStage::RegisterAllocation);
        stage_register_allocation(selected)
            .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)
    }?;
    let machine = {
        observation.enter(NativePipelineStage::PostAllocationMachine);
        stage_optimized_post_allocation_machine_plan(allocation.facts())
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)
    }?;
    let framed = machine_emission::stage_function_frame(
        machine.current().clone(),
        machine_emission::frame_layout::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
    )?;
    let encoding = {
        observation.enter(NativePipelineStage::SelectedFormEncoding);
        stage_optimized_layout_independent_selected_form_encoding(framed)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)
    }?;
    let layout = {
        observation.enter(NativePipelineStage::ResolvedLayout);
        stage_optimized_resolved_selected_form_layout(encoding.current())
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)
    }?;
    let optimized_layout = {
        observation.enter(NativePipelineStage::LayoutOptimization);
        execute_resolved_layout_optimization(
            layout.current(),
            &layout
                .current()
                .instructions
                .publication
                .selections()
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
            layout.current().instructions.publication.budget(),
        )
        .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)
    }?;
    let realization = machine_emission::assemble_fixed_frame_realization(optimized_layout)?;
    let realization = if retain_native_stage_inputs {
        realization.retain_replay_for_validation(machine_emission::FunctionRealizationReplay {
            allocation,
            machine,
            encoding,
            baseline_layout: layout,
            selected_optimization: selected_optimization.map(|run| {
                Box::new(run)
                    as Box<dyn selected_instruction_analyses::RetainedSelectedOptimizationRun>
            }),
        })
    } else {
        drop((allocation, machine, encoding, layout, selected_optimization));
        realization
    };
    let physical =
        StagedOptimizedFunctionFragmentEmissionSource::new(realization, facts, installations)
            .map_err(|error| {
                BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(format!(
                    "native artifact function-fragment emission failed: {error}"
                ))])
            })?;
    observation.enter(NativePipelineStage::FragmentEmission);
    native_artifact::emit_fragment_object(physical, original_target)
        .map_err(BodyEmissionError::Object)
}

/// Copy what emission needs from stage 02's output and cannot read from the
/// current machine program: the admitted lowering facts and the optimized
/// abstract program with its projection identities. The program is shared,
/// not copied.
fn emission_facts(target: &ValidatedOptimizedTargetOperations) -> machine_emission::EmissionFacts {
    let optimized = target.optimized();
    machine_emission::EmissionFacts {
        lowering: target.lowering_facts().clone(),
        abstract_program: optimized.shared_program(),
        projection: machine_emission::OptimizedProgramProjection {
            identity: optimized.projection(),
            terminal: optimized.projection_psi(),
            final_unit: optimized.final_unit(),
        },
    }
}

pub(crate) enum BodyEmissionError {
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
