/*
Sequence the native representation transformations for both application entries
and separately emitted callable bodies.

`lower_to_target` drives abstract optimization (`Omega 01`) and target lowering
(`Omega 02`), which ends by publishing the prepared execution plan. The caller
resolves the installations that plan lists (callback bodies, handler stacks)
and binds them before physical planning hands the plan and those bound
installations to instruction selection (`Omega 03`), selected-instruction optimization (`Omega 04`), register
allocation (`Omega 05`), post-allocation machine planning (`Omega 06`),
layout-independent encoding (`Omega 07`), resolved layout (`Omega 08`), layout
optimization (`Omega 09`), and fragment object emission.

Emission reads the current machine program plus `EmissionFacts`, the few facts
settled before instruction selection that it still needs, which physical planning
copies out of stage 02's output. Stage 02's output and the original
allocation, encoding and layout inputs reach emission only when a validator
request asked to retain them (`retain_native_stage_inputs`); otherwise they
are dropped as soon as their successor stage has run.
*/

use crate::observations::{NativePipelineObservation, NativePipelineStage};
use function_layout::FunctionRelativeOptimizationRealizationError;
use native_encoding::stage_optimized_layout_independent_selected_form_encoding;
use native_program_planning::NativeBodyPlan;
use physical_planning::{
    OptimizedPostAllocationMachinePipelineError, OptimizedSelectionPipelineError,
    PhysicalPlanningOriginals, SelectedInstructionOptimizationError,
};

#[derive(Debug)]
pub(crate) enum OptimizedVerifiedPhysicalPipelineError {
    SelectedOptimization(SelectedInstructionOptimizationError),
    RegisterEnvironment(register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    RegisterAllocation(physical_planning::RegisterAllocationError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    Encoding(native_encoding::OptimizedSelectedFormEncodingError),
    Layout(native_layout::OptimizedResolvedSelectedFormLayoutError),
    LayoutOptimization(native_layout::ResolvedLayoutOptimizationError),
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
            Self::LayoutOptimization(error) => write!(formatter, "LayoutOptimization({error:?})"),
            Self::FunctionRelativeRealization(error) => {
                write!(formatter, "FunctionRelativeRealization({error:?})")
            }
        }
    }
}

impl std::error::Error for OptimizedVerifiedPhysicalPipelineError {}

/// Bind measured stacks and the checked installed entry once. Selection reads
/// the receipt's plain facts; image publication retains the same receipt.
pub(crate) fn plan_installations(
    target: &NativeBodyPlan,
    execution: Option<provider_admission::AdmittedInstalledEntryExecution>,
    requirements: Vec<installation_evidence::InstalledInterruptStackRequirement>,
) -> Result<
    std::sync::Arc<physical_planning::installation_resources::PlannedInstallationResources>,
    &'static str,
> {
    physical_planning::installation_resources::plan_installation_resources(
        target.execution_plan(),
        target.facts.admission.installed_interrupts(),
        target.facts.admission.native_callbacks(),
        execution,
        requirements,
    )
    .map(std::sync::Arc::new)
}

pub(crate) type PreparedPhysicalBody =
    PhysicalBody<physical_planning::installation_resources::PlannedInstallationResources>;
pub(crate) type BoundPhysicalBody = PhysicalBody<installation_admission::BoundInstallationAuthority>;

pub(crate) struct PhysicalBody<Resources> {
    pub frame: physical_planning::PhysicalFrameProgram,
    pub private_references: Vec<function_identity::MachineFunctionIdentity>,
    pub facts: physical_program::EmissionFacts,
    pub installations: std::sync::Arc<Resources>,
    imports: physical_planning::imports::PhysicalImportProgram,
    windows_internet: Option<physical_planning::windows_internet::WindowsInternetProgram>,
    original_target: Option<image_emission::RetainedTargetOperations>,
    originals: Option<PhysicalPlanningOriginals>,
}

impl PreparedPhysicalBody {
    pub(crate) fn bind(
        self,
        handlers: &installation_binding::RetainedHandlerMeasurements,
        root: Option<installation_evidence::InstalledPhysicalStackRoot>,
    ) -> Result<BoundPhysicalBody, Vec<diagnostics::Diagnostic>> {
        let PhysicalBody {
            frame,
            private_references,
            facts,
            installations,
            imports,
            windows_internet,
            original_target,
            originals,
        } = self;
        let authority = installation_binding::bind(
            &frame,
            installations,
            handlers,
            root,
        )
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        Ok(PhysicalBody {
            frame,
            private_references,
            facts,
            installations: std::sync::Arc::new(authority),
            imports,
            windows_internet,
            original_target,
            originals,
        })
    }
}

pub(crate) fn prepare_physical_body(
    optimized_target: NativeBodyPlan,
    installations: std::sync::Arc<
        physical_planning::installation_resources::PlannedInstallationResources,
    >,
    observation: &NativePipelineObservation,
    retain_native_stage_inputs: bool,
) -> Result<PreparedPhysicalBody, BodyEmissionError> {
    let native_program_planning::NativeBodyPlan {
        selection,
        facts,
        private_references: native_references,
        original_for_audit,
    } = optimized_target;
    let input = physical_planning::PhysicalBodyInput {
        program: selection,
        installations: std::sync::Arc::clone(installations.facts()),
    };
    let original_target = original_for_audit.map(image_emission::RetainedTargetOperations::new);
    let physical_planning::PlannedPhysicalProgram { frame, originals } =
        physical_planning::plan_physical_program(
            input,
            physical_planning::PhysicalPlanningObservation {
                instruction_selection: observation
                    .counter(NativePipelineStage::InstructionSelection),
                selected_optimization: observation
                    .counter(NativePipelineStage::SelectedOptimization),
                register_allocation: observation.counter(NativePipelineStage::RegisterAllocation),
                post_allocation_machine: observation
                    .counter(NativePipelineStage::PostAllocationMachine),
            },
            retain_native_stage_inputs,
        )
        .map_err(|error| match error {
            physical_planning::PhysicalPlanningError::RegisterEnvironment(error) => {
                BodyEmissionError::from(
                    OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment(error),
                )
            }
            physical_planning::PhysicalPlanningError::Selection(error) => {
                BodyEmissionError::from(OptimizedVerifiedPhysicalPipelineError::Selection(error))
            }
            physical_planning::PhysicalPlanningError::SelectedOptimization(error) => {
                BodyEmissionError::from(
                    OptimizedVerifiedPhysicalPipelineError::SelectedOptimization(error),
                )
            }
            physical_planning::PhysicalPlanningError::Allocation(error) => BodyEmissionError::from(
                OptimizedVerifiedPhysicalPipelineError::RegisterAllocation(error),
            ),
            physical_planning::PhysicalPlanningError::Machine(error) => BodyEmissionError::from(
                OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine(error),
            ),
            physical_planning::PhysicalPlanningError::Frame(error) => {
                BodyEmissionError::from(match error {
                    physical_planning::PhysicalFrameError::CalleeSaveStorage(error) => {
                        FunctionRelativeOptimizationRealizationError::CalleeSaveStorage(error)
                    }
                    physical_planning::PhysicalFrameError::FrameLayout(error) => {
                        FunctionRelativeOptimizationRealizationError::FrameLayout(error)
                    }
                    physical_planning::PhysicalFrameError::FrameProtocol(error) => {
                        FunctionRelativeOptimizationRealizationError::FrameProtocol(error)
                    }
                })
            }
        })?;
    let stack =
        physical_planning::stack_demand::plan_physical_stack_program(&frame).map_err(|reason| {
            BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)])
        })?;
    let calls = physical_planning::call_origins::capture(&frame.instructions);
    let foreign_calls =
        physical_planning::foreign_call_plans::capture(&frame.instructions, frame.frame.plan());
    let image_origins =
        physical_planning::image_origins::capture(frame.instructions.functions.iter());
    let contracts = std::sync::Arc::clone(&frame.instructions.contracts);
    let temporary_stack =
        physical_planning::physical_facts::temporary_stack_bounds(&frame.instructions);
    let port_writes = physical_planning::physical_facts::port_writes(&frame.instructions);
    let boundary_settlements =
        physical_planning::boundary_settlements::capture(&frame.instructions);
    let internal_calls = physical_planning::internal_call_plans::capture(
        &frame.instructions,
        frame.frame.plan(),
        &facts.program,
    );
    let origins = physical_planning::installed_handlers::capture_execution_origins(&frame.instructions, &installations);
    let facts = physical_program::EmissionFacts {
        termination: origins.termination,
        faults: origins.faults,
        cpu: origins.cpu,
        internal_calls,
        boundary_settlements,
        semantic: facts,
        stack,
        calls,
        foreign_calls,
        image_origins,
        contracts,
        temporary_stack,
        port_writes,
    };
    let mut private_references = frame.instructions.functions.iter().flat_map(|function| {
        function.normalized_foreign_calls.iter()
            .flat_map(|call| &call.call.callbacks)
            .map(|callback| callback.callback_function)
            .chain(function.instructions_and_terminators().filter_map(|instruction| match instruction.kind {
                selected_instructions::SelectedInstructionKind::ImageAddress {
                    reference: selected_instructions::SelectedImageAddressReference::PrivateFunction { identity },
                } => Some(identity),
                _ => None,
            }))
    }).collect::<Vec<_>>();
    private_references.sort_unstable_by_key(|identity| identity.to_bytes());
    private_references.dedup();
    if private_references != native_references {
        return Err(BodyEmissionError::Object(vec![
            diagnostics::Diagnostic::error(
                "physical selection changed the closed native private-body references",
            ),
        ]));
    }
    if private_references
        .iter()
        .any(|identity| !identity.is_valid() || identity.caller() != frame.instructions.psi)
    {
        return Err(BodyEmissionError::Object(vec![
            diagnostics::Diagnostic::error(
                "private function reference escaped its exact physical program namespace",
            ),
        ]));
    }
    let windows_internet = (frame.instructions.target == target::NativeTarget::windows_x64()
        && frame.instructions.functions.iter().any(|function| {
            function.normalized_foreign_calls.iter().any(|call| {
                function
                    .instructions_and_terminators()
                    .any(|instruction| instruction.id == call.instruction)
                    && call
                        .call
                        .binding
                        .foreign_import_locator()
                        .is_some_and(|locator| {
                            locator.target() == target::TargetProfile::WindowsX64
                                && matches!(
                                    locator.locator(),
                                    target::ForeignLocatorCandidate::PeByName { library, .. }
                                    | target::ForeignLocatorCandidate::PeByOrdinal { library, .. }
                                        if library == runtime_abi::WINDOWS_INTERNET_PROVIDER_LIBRARY
                                )
                        })
            })
        }))
    .then(physical_planning::windows_internet::plan_windows_internet_provider);
    let imports = physical_planning::imports::plan_import_program(
        frame.instructions.target,
        frame.instructions.functions.iter().flat_map(|function| {
            function
                .normalized_foreign_calls
                .iter()
                .filter(|call| {
                    function
                        .instructions_and_terminators()
                        .any(|instruction| instruction.id == call.instruction)
                })
                .filter_map(|call| call.call.binding.import_locator())
                .map(|locator| {
                    machine_code::import_code::ImportCodeIdentity::Normalized(locator.to_owned())
                })
        }),
    )
    .map_err(|reason| BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)]))?;
    Ok(PreparedPhysicalBody {
        frame,
        private_references,
        facts,
        installations,
        imports,
        windows_internet,
        original_target,
        originals,
    })
}

// Placement and publication consume these facts for the same closed body.
struct BodyObjectInputs {
    private_references: Vec<function_identity::MachineFunctionIdentity>,
    imports: machine_code::import_code::EncodedImportProgram,
    windows_internet: Option<machine_code::WindowsInternetProviderCode>,
    original_target: Option<image_emission::RetainedTargetOperations>,
}

pub(crate) struct EncodedBody {
    facts: physical_program::EmissionFacts,
    installations: std::sync::Arc<installation_admission::BoundInstallationAuthority>,
    program: machine_code::EncodedMachineProgram,
    object: BodyObjectInputs,
    originals: Option<(
        PhysicalPlanningOriginals,
        post_allocation_machine::PostAllocationMachineProgram,
    )>,
}

pub(crate) struct LaidOutBody {
    text: function_layout::StagedOptimizedFixedFrameTextSection,
    object: BodyObjectInputs,
}

pub(crate) fn encode_physical_body(
    prepared: BoundPhysicalBody,
    observation: &NativePipelineObservation,
) -> Result<EncodedBody, BodyEmissionError> {
    let BoundPhysicalBody {
        frame,
        private_references,
        facts,
        installations,
        imports,
        windows_internet,
        original_target,
        originals,
    } = prepared;
    let imports = native_encoding::imports::encode_import_program(imports);
    let windows_internet = windows_internet
        .map(native_encoding::windows_internet::encode_windows_internet_provider)
        .transpose()
        .map_err(|reason| {
            BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)])
        })?;
    observation.enter(NativePipelineStage::SelectedFormEncoding);
    let framed = native_encoding::encode_function_frame(frame)
        .map_err(FunctionRelativeOptimizationRealizationError::FrameProtocol)?;
    let originals = originals.map(|originals| (originals, framed.instructions.clone()));
    let program = stage_optimized_layout_independent_selected_form_encoding(framed)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Encoding)?
        .into_current();
    Ok(EncodedBody {
        facts,
        installations,
        program,
        originals,
        object: BodyObjectInputs {
            private_references,
            imports,
            windows_internet,
            original_target,
        },
    })
}

pub(crate) fn layout_encoded_body(
    encoded: EncodedBody,
    observation: &NativePipelineObservation,
) -> Result<LaidOutBody, BodyEmissionError> {
    let EncodedBody {
        facts,
        installations,
        program,
        originals,
        object,
    } = encoded;
    let output = native_layout::layout_program(
        program,
        originals.is_some(),
        native_layout::LayoutObservation {
            placement: observation.counter(NativePipelineStage::ResolvedLayout),
            relaxation: observation.counter(NativePipelineStage::LayoutOptimization),
        },
    )
    .map_err(|error| match error {
        native_layout::LayoutError::Placement(error) => {
            OptimizedVerifiedPhysicalPipelineError::Layout(error)
        }
        native_layout::LayoutError::Optimization(error) => {
            OptimizedVerifiedPhysicalPipelineError::LayoutOptimization(error)
        }
    })?;
    let retained = match (originals, output.originals) {
        (Some((originals, physical)), Some(layout)) => {
            Some(native_verification::RetainedPhysicalInputs {
                physical,
                allocation: originals.allocation,
                machine: originals.machine,
                encoding: layout.encoding,
                baseline_layout: layout.baseline,
                selected_optimization: originals.selected_optimization,
            })
        }
        (None, None) => None,
        _ => {
            return Err(BodyEmissionError::Object(vec![
                diagnostics::Diagnostic::error(
                    "layout did not preserve requested verification inputs",
                ),
            ]));
        }
    };
    let verification =
        native_verification::PhysicalVerificationEvidence::new(output.relaxation, retained);
    let text = function_layout::place_function_text(
        output.current,
        verification,
        facts,
        installations,
        observation.counter(NativePipelineStage::FragmentEmission),
    )
    .map_err(|error| match error {
        function_layout::FunctionTextLayoutError::Realization(error) => {
            BodyEmissionError::from(error)
        }
        error => BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(error.to_string())]),
    })?;
    Ok(LaidOutBody { text, object })
}

pub(crate) fn construct_body_object(
    body: LaidOutBody,
) -> Result<native_artifact::FragmentObjectProjection, BodyEmissionError> {
    let BodyObjectInputs {
        private_references,
        imports,
        windows_internet,
        original_target,
    } = body.object;
    let container = object_file::stage_optimized_relocation_free_object_container(body.text)
        .map_err(|error| BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(format!(
            "native artifact fragment object placement failed: {error}"
        ))]))?;
    let publication = container.publication();
    let retain_originals = original_target.is_some() || container.retains_originals();
    let replay = retain_originals.then(|| std::sync::Arc::new(container));
    let mut object = native_artifact::emit_fragment_object(publication, replay, original_target)
        .map_err(BodyEmissionError::Object)?;
    object = object.with_import_program(imports).map_err(|reason| {
        BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)])
    })?;
    if let Some(code) = windows_internet {
        object = object
            .with_windows_internet_provider(code)
            .map_err(|reason| {
                BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)])
            })?;
    }
    let mut emitted_references = object
        .object()
        .object()
        .layout
        .function_symbols
        .iter()
        .map(|(_, reference)| reference.identity)
        .collect::<Vec<_>>();
    emitted_references.sort_unstable_by_key(|identity| identity.to_bytes());
    if emitted_references != private_references {
        return Err(BodyEmissionError::Object(vec![
            diagnostics::Diagnostic::error(
                "object publication changed the closed physical program's private references",
            ),
        ]));
    }
    Ok(object)
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
