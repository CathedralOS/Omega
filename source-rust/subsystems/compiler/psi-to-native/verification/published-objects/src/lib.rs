//! Recheck the emitted object against original stage inputs on an explicit
//! audit request. Only a compile that requested an audit retains those
//! inputs; without them each audit refuses rather than passing.

#![forbid(unsafe_code)]

mod allocation;

use image_emission::{FragmentReplay, ObjectArtifact, RetainedTargetOperations};

use machine::StagedOptimizedPostAllocationMachinePlan;
use register_allocation::RetainedAllocation;
use selected_instruction_analyses::SelectedOptimizationRecord;
pub use selected_rewrites::SelectedOptimizationReplay;

/// The original stage outputs a validator request keeps for replay.
#[derive(Debug)]
pub struct RetainedPhysicalInputs {
    pub allocation: RetainedAllocation,
    pub machine: StagedOptimizedPostAllocationMachinePlan,
    pub encoding: machine_code::EncodedMachineProgram,
    pub baseline_layout: machine_code::ResolvedMachineProgram,
    /// The stage 07 rewrite run behind the allocated program, when a rewrite
    /// phase ran. Only its own validator reads it.
    pub selected_optimization: Option<SelectedOptimizationReplay>,
}

impl RetainedPhysicalInputs {
    /// Check that the retained encodings and physical roots describe this layout.
    pub fn belongs_to(&self, layout: &machine_code::OptimizedMachineLayout) -> bool {
        let program = &layout.program;
        self.machine.current().identity == program.context.identity
            && self.allocation.publication() == &program.context.publication
            && self.machine.machine().shared_plan() == program.context.plan
            && program.bindings == self.encoding.bindings
            && program.bindings == self.baseline_layout.bindings
            && program.protocol_identity == self.encoding.protocol_identity
            && program.protocol == self.encoding.protocol
            && program.frame == self.encoding.frame
            && program.context == self.encoding.context
            && program.layout_requirements == self.encoding.layout_requirements
            && program.exit_requirements == self.encoding.exit_requirements
            && program.fragment_origins == self.encoding.fragment_origins
            && program.frame_demands == self.encoding.frame_demands
            && program.encoding.as_ref() == self.encoding.encoding.as_ref()
            && layout.baseline == self.baseline_layout.layout.roots()
    }

    pub fn selected_optimization_evidence(
        &self,
    ) -> (
        Option<&SelectedOptimizationReplay>,
        &SelectedOptimizationRecord,
    ) {
        (
            self.selected_optimization.as_ref(),
            self.allocation.current().history(),
        )
    }

    fn check_object<'body>(
        &self,
        artifact: &'body ObjectArtifact,
    ) -> Result<&'body image_emission::PhysicalEncodingReadback, diagnostics::Diagnostic> {
        let original = artifact
            .fragment_replay()
            .and_then(FragmentReplay::original_encoding)
            .ok_or_else(missing_originals)?;
        if original.physical.identity != self.machine.current().identity
            || !std::sync::Arc::ptr_eq(&self.encoding.encoding, &original.encoding)
        {
            return Err(diagnostics::Diagnostic::error(
                "physical audit originals belong to another object",
            ));
        }
        Ok(original)
    }
}

fn missing_originals() -> diagnostics::Diagnostic {
    diagnostics::Diagnostic::error("physical audit requires retained originals (ReplayUnavailable)")
}

/// The compilation report owns the inputs requested for independent verification.
#[derive(Debug, Default)]
pub struct NativeStageReplay {
    root: Option<Box<RetainedPhysicalInputs>>,
    private: Vec<(
        function_identity::MachineFunctionIdentity,
        Box<RetainedPhysicalInputs>,
    )>,
    components: Vec<([u8; 32], Box<RetainedPhysicalInputs>)>,
}

impl NativeStageReplay {
    pub fn from_bodies(
        root: Option<Box<RetainedPhysicalInputs>>,
        mut private: Vec<(
            function_identity::MachineFunctionIdentity,
            Box<RetainedPhysicalInputs>,
        )>,
    ) -> Result<Self, diagnostics::Diagnostic> {
        private.sort_unstable_by_key(|(key, _)| key.to_bytes());
        if private.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(diagnostics::Diagnostic::error(
                "duplicate private body in native stage replay",
            ));
        }
        Ok(Self {
            root,
            private,
            components: Vec::new(),
        })
    }

    pub fn root(&self) -> Option<&RetainedPhysicalInputs> {
        self.root.as_deref()
    }

    pub fn with_components(
        mut self,
        mut components: Vec<([u8; 32], Box<RetainedPhysicalInputs>)>,
    ) -> Result<Self, diagnostics::Diagnostic> {
        components.sort_unstable_by_key(|(slot, _)| *slot);
        for (index, (slot, original)) in components.iter().enumerate() {
            if components[..index]
                .iter()
                .rev()
                .take_while(|(other, _)| other == slot)
                .any(|(_, other)| {
                    std::sync::Arc::ptr_eq(&original.encoding.encoding, &other.encoding.encoding)
                })
            {
                return Err(diagnostics::Diagnostic::error(
                    "duplicate component object in native stage replay",
                ));
            }
        }
        self.components = components;
        Ok(self)
    }

    /// The same physical export can be reached through two receivers. An audit
    /// follows the retained encoding from this particular object construction.
    pub fn component(
        &self,
        slot: &[u8; 32],
        object: &ObjectArtifact,
    ) -> Option<&RetainedPhysicalInputs> {
        let start = self.components.partition_point(|(key, _)| key < slot);
        self.components[start..]
            .iter()
            .take_while(|(key, _)| key == slot)
            .find(|(_, inputs)| inputs.check_object(object).is_ok())
            .map(|(_, inputs)| inputs.as_ref())
    }

    fn check_bodies(
        &self,
        artifact: &ObjectArtifact,
        originals: &RetainedPhysicalInputs,
        check: fn(
            &ObjectArtifact,
            &RetainedPhysicalInputs,
        ) -> Result<bool, diagnostics::Diagnostic>,
    ) -> Result<bool, diagnostics::Diagnostic> {
        let mut checked = check(artifact, originals)?;
        for body in artifact.private_functions() {
            let index = self
                .private
                .binary_search_by_key(&body.identity.to_bytes(), |(key, _)| key.to_bytes())
                .map_err(|_| missing_originals())?;
            checked |= self.check_bodies(&body.body, &self.private[index].1, check)?;
        }
        Ok(checked)
    }
}

/// Compare the current fixed-frame output with retained original allocation,
/// encoding and layout inputs, and its allocation-recovery selection with the
/// one physical planning used. Missing originals refuse this explicit audit.
pub fn verify_retained_fixed_frame(
    artifact: &ObjectArtifact,
    archive: &NativeStageReplay,
) -> Result<(), diagnostics::Diagnostic> {
    archive
        .check_bodies(
            artifact,
            archive.root().ok_or_else(missing_originals)?,
            |object, originals| verify_body_fixed_frame(object, originals).map(|()| true),
        )
        .map(|_| ())
}

pub fn verify_body_fixed_frame(
    artifact: &ObjectArtifact,
    originals: &RetainedPhysicalInputs,
) -> Result<(), diagnostics::Diagnostic> {
    let encoded = originals.check_object(artifact)?;
    let replay = artifact.fragment_replay().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "fixed-frame audit requires retained originals (ReplayUnavailable)",
        )
    })?;
    verify_physical_origins(artifact, replay)?;
    let current = replay
        .fragment_source()
        .frame_application_for_audit()
        .source()
        .source();
    let layout = current.relaxed_layout();
    let requirements = physical::verify_originals(
        originals,
        &encoded.physical,
        layout.current(),
        layout.trace(),
    )
    .map_err(|error| {
        diagnostics::Diagnostic::error(format!("fixed-frame original replay failed: {error:?}"))
    })?;
    current
        .verify_frame_requirements(&requirements)
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    let original = retained_target(replay, "fixed-frame")?;
    let expected = original
        .target_for_correspondence()
        .optimized()
        .selections()
        .for_phase(optimization_core::OptimizationExecutionPhase::AllocationRecovery)
        .identity();
    if current
        .function_relative_manifest()
        .record()
        .allocation_recovery_selections
        != expected
    {
        return Err(diagnostics::Diagnostic::error(
            "fixed-frame allocation recovery differs from the selection physical planning used",
        ));
    }
    Ok(())
}

/// Reconstruct the physical instruction projections while the explicit audit owns its originals.
fn verify_physical_origins(
    artifact: &ObjectArtifact,
    replay: &FragmentReplay,
) -> Result<(), diagnostics::Diagnostic> {
    let source = replay.fragment_source();
    let current = source.frame_application_for_audit().source().source();
    let program = &replay
        .original_encoding()
        .ok_or_else(|| {
            diagnostics::Diagnostic::error(
                "original physical instructions were not retained (ReplayUnavailable)",
            )
        })?
        .physical;
    let facts = source.emission_facts();
    let physical = &facts.physical;
    let mismatch = diagnostics::Diagnostic::error;
    let calls = frame_planning::call_origins::capture(program);
    if &calls != physical.calls.as_ref() || &calls != artifact.call_origins().as_ref() {
        return Err(mismatch(
            "object call origins differ from retained physical instructions",
        ));
    }
    let foreign = frame_planning::foreign_call_plans::capture(program, current.frame_layout());
    if foreign.as_ref() != physical.foreign_calls.as_ref().map(std::sync::Arc::as_ref)
        || foreign.as_ref().ok() != Some(artifact.foreign_call_plans().as_ref())
    {
        return Err(mismatch(
            "object foreign call plans differ from retained physical instructions",
        ));
    }
    if frame_planning::image_origins::capture(program.functions.iter()) != physical.image_origins {
        return Err(mismatch(
            "image address origins differ from retained physical instructions",
        ));
    }
    if frame_planning::physical_facts::temporary_stack_bounds(program) != physical.temporary_stack {
        return Err(mismatch(
            "temporary stack demands differ from retained physical instructions",
        ));
    }
    if frame_planning::physical_facts::port_writes(program) != physical.port_writes {
        return Err(mismatch(
            "port event origins differ from retained physical instructions",
        ));
    }
    if frame_planning::boundary_settlements::capture(program) != physical.boundary_settlements {
        return Err(mismatch(
            "boundary settlements differ from retained physical instructions",
        ));
    }
    if frame_planning::internal_call_plans::capture(
        program,
        current.frame_layout(),
        &facts.semantic.program,
    ) != physical.internal_calls
    {
        return Err(mismatch(
            "internal call plans differ from retained physical instructions",
        ));
    }
    if installation_admission::selected::cpu_origins::capture(program) != physical.cpu {
        return Err(mismatch(
            "CPU origins differ from retained physical instructions",
        ));
    }
    if installation_admission::selected::fault_origins::capture(program) != physical.faults {
        return Err(mismatch(
            "fault origins differ from retained physical instructions",
        ));
    }
    Ok(())
}

/// Replay each body's retained selected-instruction rewrite run against the record its
/// allocated program carries. Returns whether any body had a run to check;
/// `false` means no selected-instruction rewrite phase produced this artifact.
pub fn verify_retained_selected_optimization(
    artifact: &ObjectArtifact,
    archive: &NativeStageReplay,
) -> Result<bool, diagnostics::Diagnostic> {
    archive.check_bodies(
        artifact,
        archive.root().ok_or_else(missing_originals)?,
        verify_body_selected_optimization,
    )
}

pub fn verify_body_selected_optimization(
    artifact: &ObjectArtifact,
    originals: &RetainedPhysicalInputs,
) -> Result<bool, diagnostics::Diagnostic> {
    originals.check_object(artifact)?;
    physical::verify_selected_optimization_replay(originals).map_err(|error| {
        diagnostics::Diagnostic::error(format!("selected-instruction replay failed: {error:?}"))
    })
}

pub fn verify_retained_target_operations(
    artifact: &ObjectArtifact,
) -> Result<(), diagnostics::Diagnostic> {
    for private in artifact.private_functions() {
        verify_retained_target_operations(&private.body)?;
    }
    let replay = artifact.fragment_replay().ok_or_else(|| {
        diagnostics::Diagnostic::error("target-operation audit requires retained target operations")
    })?;
    let original = retained_target(replay, "target-operation")?;
    let current = original.target_for_correspondence();
    let optimized = current.optimized();
    target_lowering::replay::require_lowering_context(optimized, current.lowering_facts(), current.target())
        .map_err(|error| diagnostics::Diagnostic::error(format!("target-operation source audit failed: {error:?}")))?;
    // Every access reference native program planning established must land on backing
    // the selected program actually allocates. Ordinary emission builds
    // from the current program, so this check runs here and in the object
    // replay, not on the default path.
    replay
        .verify_access_geometry()
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    // The validator judges the plan published for this object,
    // not a plan prepared again here.
    target_lowering::replay::verify_execution_plan(
        current.execution_plan(),
        current.working_program_for_validation(),
        optimized,
        current.lowering_facts(),
    )
    .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    let expected = current
        .lowering_facts()
        .replay_borrowed_storage_layouts(current.target())
        .map_err(|error| {
            diagnostics::Diagnostic::error(format!("borrowed storage geometry audit failed: {error}"))
        })?;
    if expected != current.execution_plan().plan().borrowed_storage_layouts {
        return Err(diagnostics::Diagnostic::error(
            "published borrowed storage geometry differs from admitted declarations",
        ));
    }
    Ok(())
}

/// The native planning output retained beside this object, joined to it: the current
/// allocation must name the unit, selections and budget that output was lowered under.
fn retained_target<'replay>(
    replay: &'replay FragmentReplay,
    audit: &str,
) -> Result<&'replay RetainedTargetOperations, diagnostics::Diagnostic> {
    let original = replay.target_source().ok_or_else(|| {
        diagnostics::Diagnostic::error(format!("{audit} audit requires retained target operations"))
    })?;
    let current = replay
        .fragment_source()
        .frame_application_for_audit()
        .source()
        .source();
    let optimized = original.target_for_correspondence().optimized();
    let allocation = current.post_allocation_manifest().record();
    if !original.belongs_to(&replay.fragment_source().emission_facts().semantic)
        || allocation.optimization_unit != optimized.unit().identity
        || allocation.selection != optimized.selections().identity()
        || allocation.budget_per_pass != optimized.budget_per_pass()
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "{audit} audit: retained target operations name another unit, selection or budget"
        )));
    }
    Ok(original)
}

/// Compare the retained stage outputs only when an independent audit asks for them.
pub mod physical {
    use super::RetainedPhysicalInputs;
    use crate::allocation::{AllocationOriginals, validate_allocated_callee_saved_requirements};
    use encoding_admission::layout::{
        ResolvedLayoutValidation, validate_optimized_resolved_selected_form_layout,
    };
    use encoding_admission::{
        SelectedFormEncodingValidation,
        validate_optimized_layout_independent_selected_form_encoding,
    };
    use optimization_core::OptimizationExecutionPhase;
    use placement::relaxation::{
        validate_current_layout_optimization, validate_resolved_layout_optimization,
    };
    use selected_instruction_analyses::SelectedProgramView;
    fn refused(stage: &'static str, error: impl std::fmt::Debug) -> VerificationError {
        VerificationError::Stage {
            stage,
            detail: format!("{error:?}"),
        }
    }

    pub fn verify_originals(
        originals: &RetainedPhysicalInputs,
        instructions: &post_allocation_machine::PostAllocationMachineProgram,
        layout: &machine_code::OptimizedMachineLayout,
        trace: Option<&machine_code::layout::X86BranchRelaxationRecord>,
    ) -> Result<register_homes::AllocatedCalleeSavedRequirements, VerificationError> {
        originals
            .allocation
            .validate_publication()
            .map_err(|error| refused("allocation publication", error))?;
        let program = &layout.program;
        let current = &AllocationOriginals::retain(&originals.allocation);
        current
            .validate_roots()
            .map_err(|error| refused("allocation roots", error))?;
        let selected = &current.selected;
        let environment = &current.environment;
        let physical = environment.physical();
        let replay = post_allocation_machine::program_validation::PhysicalProgramReplay {
            selected: selected.current_program(),
            machine: originals.machine.machine().plan(),
            environment,
            preservation: &current.preservation,
            publication: &current.publication,
            phase: post_allocation_machine::PostAllocationMachinePhaseReceipt::from_parts(
                originals.machine.optimization().complete_selection(),
                originals.machine.optimization().phase_selections(),
            ),
        };
        post_allocation_machine::program_validation::validate_current_program(instructions, replay)
            .map_err(VerificationError::CurrentProgram)?;
        if !originals.belongs_to(layout) {
            return Err(VerificationError::RootMismatch);
        }
        validate_optimized_layout_independent_selected_form_encoding(
            SelectedFormEncodingValidation {
                physical: instructions,
                replay,
                frame: Some(&program.frame),
                artifact: &originals.encoding,
            },
        )
        .map_err(|error| refused("selected-form encoding", error))?;
        validate_optimized_resolved_selected_form_layout(ResolvedLayoutValidation {
            physical: instructions,
            replay,
            pre_layout: originals.encoding.encoding.as_ref(),
            artifact: &originals.baseline_layout,
        })
        .map_err(|error| refused("resolved layout", error))?;
        validate_resolved_layout_optimization(
            physical,
            &originals.baseline_layout.layout,
            &current
                .publication
                .selections()
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
            layout,
            trace,
        )
        .map_err(|error| refused("layout optimization", error))?;
        encoding_admission::layout::validate_current_resolved_layout(program)
            .map_err(|error| refused("resolved layout bytes", error))?;
        validate_current_layout_optimization(layout, trace)
            .map_err(|error| refused("relaxation trace", error))?;
        let requirements = validate_allocated_callee_saved_requirements(
            current,
            program.context.preservation.plan().clone(),
        )
        .map_err(|error| refused("callee-saved requirements", error))?;
        if encoding_admission::fragment_origins::prepare_fragment_origins(instructions)
            != *program.fragment_origins
        {
            return Err(VerificationError::RootMismatch);
        }
        let exits = encoding_admission::exit_requirements::prepare_exit_requirements(
            instructions,
            &program.encoding,
        )
        .map_err(|error| refused("whole-function exits", error))?;
        let demands = frame_admission::frame_layout::prepare_frame_demands(
            &instructions.machine,
            &instructions.environment,
        )
        .map_err(|error| refused("frame demands", error))?;
        if exits != *program.exit_requirements || demands != *program.frame_demands {
            return Err(VerificationError::RootMismatch);
        }
        Ok(requirements)
    }

    pub fn verify_selected_optimization_replay(
        originals: &RetainedPhysicalInputs,
    ) -> Result<bool, VerificationError> {
        match originals.selected_optimization_evidence() {
            (Some(run), record) => run
                .validate(record)
                .map(|()| true)
                .map_err(|error| refused("selected-instruction optimization replay", error)),
            (None, record) if record.is_rewritten() => Err(VerificationError::ReplayUnavailable),
            (None, _) => Ok(false),
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum VerificationError {
        /// The compile did not retain the originals this audit compares with.
        ReplayUnavailable,
        /// The current program differs from the original allocation and machine plan.
        CurrentProgram(post_allocation_machine::program_validation::PhysicalProgramValidationError),
        /// The current form names different original roots.
        RootMismatch,
        /// A validator refused this stage output or its retained original.
        Stage { stage: &'static str, detail: String },
    }
}
