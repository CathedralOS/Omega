//! Representation roots and pipeline ownership at migrated boundaries.
//!
//! Each entry here names a completed boundary, not an exemption for others.
//! Add a representation only when its program root and subordinate owners exist.

use std::path::{Path, PathBuf};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

fn rust_source(directory: &Path) -> String {
    let mut paths = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            if path.is_dir() {
                rust_source(&path)
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                std::fs::read_to_string(path).unwrap()
            } else {
                String::new()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn selected_program_has_one_representation_entrance() {
    let directory =
        repository().join("omega-rust/omega/representations/omega-selected-instructions/src");
    let mut roots = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    roots.sort();
    assert_eq!(roots, ["lib.rs", "selected_instructions.rs"]);
    let root = std::fs::read_to_string(directory.join("selected_instructions.rs")).unwrap();
    assert!(root.contains("pub struct SelectedInstructionPlan"));
    for area in [
        "control_flow",
        "values",
        "instructions",
        "calls",
        "effects",
        "provenance",
    ] {
        assert!(
            root.contains(&format!("pub mod {area};")),
            "missing owner: {area}"
        );
    }
    let source = rust_source(&directory);
    assert_eq!(
        source
            .matches("pub struct SelectedInstructionPlan {")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("pub struct PreAllocationMachineEffectPlan {")
            .count(),
        1
    );
    assert!(!source.contains("StagedOptimized"));
}

#[test]
fn program_representations_have_named_roots_and_concept_owners() {
    for (package, module, program, areas) in [
        (
            "omega-machine-code",
            "machine_code",
            "MachineCodePlan",
            &[
                "functions",
                "calls",
                "storage",
                "control_flow",
                "ownership",
                "boundary",
                "provenance",
                "instructions",
                "fragments",
                "encoding",
                "layout",
            ][..],
        ),
        (
            "omega-physical-instructions",
            "physical_instructions",
            "PostAllocationMachinePlan",
            &[
                "control_flow",
                "instructions",
                "operands",
                "identity",
                "codec",
                "evidence",
            ][..],
        ),
        (
            "omega-register-homes",
            "register_homes",
            "AllocatedProgram",
            &["storage", "evidence", "identity", "codec"][..],
        ),
        (
            "omega-abstract-operations",
            "abstract_operations",
            "AbstractOperationPlan",
            &["control_flow", "values", "calls", "ownership", "operations"][..],
        ),
        (
            "omega-target-operations",
            "target_operations",
            "TargetOperationPlan",
            &[
                "control_flow",
                "values",
                "calls",
                "storage",
                "boundary",
                "operations",
            ][..],
        ),
        (
            "omega-legalized-operations",
            "legalized_operations",
            "LegalizedOperationPlan",
            &["control_flow", "values", "calls", "legality", "identity"][..],
        ),
        (
            "omega-assigned-target-operations",
            "assigned_operations",
            "AssignedOperationPlan",
            &["control_flow", "values", "calls", "storage", "operations"][..],
        ),
    ] {
        let directory = repository()
            .join("omega-rust/omega/representations")
            .join(package)
            .join("src");
        let mut files = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        files.sort();
        let mut expected = vec!["lib.rs".to_owned(), format!("{module}.rs")];
        expected.sort();
        assert_eq!(files, expected, "ambiguous entrance in {package}");
        let root = std::fs::read_to_string(directory.join(format!("{module}.rs"))).unwrap();
        assert!(
            root.contains(&format!("pub struct {program} {{")),
            "root does not own {program}"
        );
        for area in areas {
            assert!(
                root.contains(&format!("pub mod {area};")),
                "{package} has no {area} owner"
            );
        }
        let source = rust_source(&directory);
        assert_eq!(
            source.matches(&format!("pub struct {program} {{")).count(),
            1
        );
        assert!(
            !source.contains("StagedOptimized"),
            "{package} contains stage ancestry"
        );
    }
}

#[test]
fn register_home_data_is_independent_of_allocation_authority() {
    let owner = repository().join("omega-rust/omega/representations/omega-register-homes");
    let representation = rust_source(&owner.join("src"));
    let allocator = rust_source(&repository().join("omega-rust/omega/pipeline/omega-regalloc/src"));
    for declaration in [
        "pub struct RegisterHomePlan {",
        "pub struct FunctionRegisterHomes {",
        "pub struct VirtualRegisterHome {",
        "pub struct RegisterHomeIdentity(",
        "pub struct AllocationLegalityIdentity(",
        "pub struct LiveRangeIdentity(",
        "pub struct AllocatorAvailabilityIdentity(",
    ] {
        assert_eq!(
            representation.matches(declaration).count(),
            1,
            "{declaration}"
        );
        assert!(
            !allocator.contains(declaration),
            "allocator owns {declaration}"
        );
    }
    assert!(!representation.contains("ValidatedRegisterHomes"));
    assert!(allocator.contains("pub struct ValidatedRegisterHomes {"));
    let manifest = std::fs::read_to_string(owner.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("omega-regalloc"));
    assert!(!manifest.contains("/pipeline/"));
}

#[test]
fn physical_instruction_data_is_independent_of_optimizer_authority() {
    let owner = repository().join("omega-rust/omega/representations/omega-physical-instructions");
    let representation = rust_source(&owner.join("src"));
    let optimizer =
        rust_source(&repository().join("omega-rust/omega/pipeline/omega-machine-optimizer/src"));
    for declaration in [
        "pub struct PostAllocationMachinePlan {",
        "pub struct PostAllocationMachineFunction {",
        "pub struct PostAllocationMachineBlock {",
        "pub struct PostAllocationMachineInstruction {",
        "pub struct PostAllocationStructuralUnitFunction {",
        "pub struct PhysicalOperandFootprint {",
        "pub struct PostAllocationMachineIdentity(",
        "pub enum MachineAlternativeChoiceRule {",
    ] {
        assert_eq!(
            representation.matches(declaration).count(),
            1,
            "{declaration}"
        );
        assert!(
            !optimizer.contains(declaration),
            "optimizer owns {declaration}"
        );
    }
    assert!(!representation.contains("pub struct ValidatedPostAllocationMachinePlan"));
    assert!(optimizer.contains("pub struct ValidatedPostAllocationMachinePlan {"));
    let manifest = std::fs::read_to_string(owner.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("omega-machine-optimizer"));
    assert!(!manifest.contains("omega-regalloc"));
    assert!(!manifest.contains("/pipeline/"));
}

#[test]
fn structural_call_encoding_data_does_not_require_the_isa_implementation() {
    let root = repository();
    let representation =
        rust_source(&root.join("omega-rust/omega/representations/omega-machine-code/src"));
    let isa = rust_source(
        &root.join("omega-rust/omega/backend/instruction_set_architectures/omega-isa-x86_64/src"),
    );
    for declaration in [
        "pub enum X86_64StructuralUnitInternalControlFixupKind {",
        "pub enum X86_64StructuralUnitInternalControlFixupState {",
        "pub struct X86_64StructuralUnitInternalControlFixup {",
        "pub enum X86_64StructuralUnitInternalControlResolutionState {",
        "pub struct X86_64ResolvedStructuralUnitInternalControlFixup {",
        "pub struct X86_64StructuralUnitRootRead {",
        "pub struct X86_64StructuralUnitCallerCopyWrite {",
        "pub struct X86_64StructuralUnitArgumentPointerWrite {",
        "pub struct X86_64SelectedStructuralUnitCallFootprint {",
    ] {
        assert_eq!(
            representation.matches(declaration).count(),
            1,
            "{declaration}"
        );
        assert!(!isa.contains(declaration), "ISA still owns {declaration}");
    }
    for admitted in [
        "pub struct ValidatedX86_64SelectedStructuralUnitCallTemplate {",
        "pub struct ValidatedX86_64ResolvedStructuralUnitCall {",
    ] {
        assert!(isa.contains(admitted), "target admission lost {admitted}");
        assert!(
            !representation.contains(admitted),
            "data owner grants target admission"
        );
    }
    for path in [
        "omega-rust/omega/pipeline/omega-post-allocation-machine-to-selected-form-encoding/src/model.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout/model.rs",
    ] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        assert!(source.contains("use omega_machine_code::{"));
        assert!(!source.contains("use omega_isa_x86_64::{"));
    }
    let manifest = std::fs::read_to_string(
        root.join("omega-rust/omega/representations/omega-machine-code/Cargo.toml"),
    )
    .unwrap();
    assert!(!manifest.contains("/backend/"));
    assert!(!manifest.contains("/pipeline/"));
}

#[test]
fn resolved_layout_data_and_identity_do_not_require_a_producing_stage() {
    let root = repository();
    let machine =
        rust_source(&root.join("omega-rust/omega/representations/omega-machine-code/src"));
    let physical =
        rust_source(&root.join("omega-rust/omega/representations/omega-physical-instructions/src"));
    let pipeline = rust_source(&root.join("omega-rust/omega/pipeline"));
    for declaration in [
        "pub struct ResolvedMachineLayout {",
        "pub struct ResolvedSelectedFunctionLayout {",
        "pub struct ResolvedStructuralUnitFunctionLayout {",
        "pub struct ResolvedSelectedFormLayoutIdentity(",
        "pub struct SelectedFormEncodingIdentity(",
        "pub struct SelectedFormInternalMachineFixup {",
    ] {
        assert_eq!(machine.matches(declaration).count(), 1, "{declaration}");
        assert!(
            !pipeline.contains(declaration),
            "pipeline owns {declaration}"
        );
    }
    let record = "pub struct PostAllocationMachineOptimizationCustody {";
    assert_eq!(physical.matches(record).count(), 1);
    assert!(!pipeline.contains(record));
    assert!(!machine.contains("omega_machine_optimizer::"));
    assert!(!machine.contains("pub struct StagedOptimizedResolvedSelectedFormLayout"));
    assert!(machine.contains("omega.terminal.resolved-selected-form-layout.v9"));
    assert!(!pipeline.contains("omega.terminal.resolved-selected-form-layout.v9"));

    let stage = root.join("omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout");
    let wrapper = std::fs::read_to_string(stage.join("model.rs")).unwrap();
    assert!(wrapper.contains("program: Arc<ResolvedMachineLayout>"));
    assert!(wrapper.contains("Arc::clone(&self.program)"));
    assert!(!wrapper.contains("pub(super) functions:"));
    assert!(!wrapper.contains("pub(super) structural_unit_functions:"));
    let admission = std::fs::read_to_string(stage.join("stage.rs")).unwrap();
    assert!(admission.contains("pub fn admit_resolved_machine_layout"));
    assert!(admission.contains("super::validation::validate("));
    for package in ["omega-machine-code", "omega-physical-instructions"] {
        let manifest = std::fs::read_to_string(
            root.join("omega-rust/omega/representations")
                .join(package)
                .join("Cargo.toml"),
        )
        .unwrap();
        assert!(!manifest.contains("/pipeline/"));
        assert!(!manifest.contains("/backend/"));
    }
}

#[test]
fn psi_program_roots_expose_concept_owners_without_flat_definition_dumps() {
    for (package, module, program, areas) in [
        (
            "psi-terminal",
            "terminal_module",
            "TerminalModule",
            &[
                "boundary",
                "control_flow",
                "identity",
                "observation",
                "ownership",
                "proof",
                "types",
                "values",
            ][..],
        ),
        (
            "psi-checked-trees",
            "checked_trees",
            "CheckedTrees",
            &[
                "admissibility",
                "borrow",
                "facts",
                "flow",
                "operators",
                "proof",
                "service_parameter",
                "statement",
                "values",
            ][..],
        ),
    ] {
        let directory = repository()
            .join("omega-rust/psi/representations")
            .join(package)
            .join("src");
        let mut files = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        files.sort();
        let mut expected = vec!["lib.rs".to_owned(), format!("{module}.rs")];
        expected.sort();
        assert_eq!(files, expected, "ambiguous program entrance in {package}");
        let entrance = std::fs::read_to_string(directory.join("lib.rs")).unwrap();
        assert!(entrance.contains(&format!("pub mod {module};")));
        assert!(!entrance.contains("pub struct ") && !entrance.contains("pub enum "));
        let root = std::fs::read_to_string(directory.join(format!("{module}.rs"))).unwrap();
        assert!(root.contains(&format!("pub struct {program} {{")));
        assert_eq!(
            root.matches("pub struct ").count(),
            1,
            "root must lead to subordinate vocabulary"
        );
        assert!(!root.contains("pub enum "));
        for area in areas {
            assert!(
                root.contains(&format!("pub mod {area};")),
                "{package} lost its {area} owner"
            );
        }
        assert_eq!(
            rust_source(&directory)
                .matches(&format!("pub struct {program} {{"))
                .count(),
            1
        );
    }
}

#[test]
fn effect_analysis_does_not_depend_on_optimizer_history() {
    let root = repository();
    let stage =
        root.join("omega-rust/omega/pipeline/omega-selected-instructions-to-machine-effects");
    let source = rust_source(&stage.join("src"));
    for forbidden in [
        "StagedOptimized",
        "_after_",
        "source_legality_stage",
        "pub struct PreAllocationMachineEffectPlan",
    ] {
        assert!(
            !source.contains(forbidden),
            "effect analysis leaked {forbidden}"
        );
    }
    let manifest = std::fs::read_to_string(stage.join("Cargo.toml")).unwrap();
    for forbidden in [
        "omega-allocation-legality-to-",
        "omega-selected-instructions-to-register-homes",
        "omega-target-operations-to-selected-instructions",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "effect stage depends on a producer: {forbidden}"
        );
    }
    let construction = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/omega-register-homes-to-post-allocation-machine/src/construction/mod.rs",
    )).unwrap();
    assert!(construction.contains("analyze_machine_effects(selected, environment)"));
    let validation = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/omega-register-homes-to-post-allocation-machine/src/validation.rs",
    )).unwrap();
    assert!(
        validation.contains("validate_machine_effects(selected, environment, staged.effects())")
    );
}

#[test]
fn allocation_has_one_phase_owner_and_machine_consumers_ignore_history() {
    let pipeline = repository().join("omega-rust/omega/pipeline");
    for retired in [
        "omega-selected-instructions-to-liveness",
        "omega-liveness-to-live-ranges",
        "omega-live-ranges-to-allocation-legality",
        "omega-allocation-legality-to-literal-folds",
        "omega-allocation-legality-to-fixed-view-copies",
        "omega-fixed-view-copies-to-reanalyzed-legality",
        "omega-allocation-legality-to-active-resident-rematerialization",
        "omega-allocation-legality-to-register-homes",
        "omega-literal-folds-to-register-homes",
    ] {
        assert!(
            !pipeline.join(retired).join("Cargo.toml").exists(),
            "retired phase fragment: {retired}"
        );
    }
    let allocation = pipeline.join("omega-selected-instructions-to-register-homes/src");
    for area in ["analyses", "rewrites", "assignment", "output"] {
        assert!(
            allocation.join(area).join("mod.rs").is_file(),
            "missing allocation owner: {area}"
        );
    }
    let output = rust_source(&allocation.join("output"));
    assert!(output.contains("pub trait AllocationSource: sealed::Sealed"));
    assert!(output.contains("pub struct AllocationOutput<'program>"));
    assert!(output.contains("pub enum AllocationEvidence"));
    assert!(output.contains("pub struct RetainedAllocation"));
    assert!(output.contains("impl TryFrom<StagedOptimizedRegisterHomes>"));
    let retained = std::fs::read_to_string(allocation.join("output/retained.rs")).unwrap();
    let current_accessor = retained
        .split("pub fn current(&self)")
        .nth(1)
        .unwrap()
        .split("impl sealed::Sealed")
        .next()
        .unwrap();
    assert!(current_accessor.contains("self.current.view()"));
    assert!(!current_accessor.contains("match"));
    assert!(!current_accessor.contains("self.replay"));
    let current = std::fs::read_to_string(allocation.join("output/current.rs")).unwrap();
    assert!(current.contains("program: AllocatedProgram"));
    assert!(current.contains("program: self.program.as_ref()"));
    assert!(!current.contains("StagedOptimized"));
    assert!(!current.contains("History"));
    assert!(retained.contains("self.current.validate_against(&current)?"));
    for consumer in [
        "omega-register-homes-to-post-allocation-machine",
        "omega-post-allocation-machine-to-optimized-machine",
        "omega-register-homes-to-callee-saved-requirements",
    ] {
        let source = rust_source(&pipeline.join(consumer).join("src"));
        assert!(
            source.contains("AllocationSource"),
            "{consumer} has no common allocation input"
        );
        for forbidden in [
            "_after_",
            "StagedOptimizedRegisterHomes",
            "source_legality_stage",
            "selected_stage()",
            "steps().last()",
        ] {
            assert!(
                !source.contains(forbidden),
                "{consumer} depends on optimizer history: {forbidden}"
            );
        }
    }
}

#[test]
fn fixed_frame_consumes_current_allocation_and_retains_baseline_receipt_role() {
    let root = repository().join("omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/realization/function_relative_realization");
    let route = std::fs::read_to_string(root.join("routes/fixed_frame.rs")).unwrap();
    assert!(route.contains("allocation: RetainedAllocation"));
    let compact_route = route.split_whitespace().collect::<String>();
    assert!(compact_route.contains("staged.allocation.replay_allocation()"));
    let assembly = std::fs::read_to_string(root.join("assembly/fixed_frame.rs")).unwrap();
    assert!(assembly.contains("AllocationEvidence::RegisterHomes(source) => Ok(*source)"));
    for source in [&route, &assembly] {
        for forbidden in [
            "legality_stage()",
            "live_range_stage()",
            "liveness_stage()",
            "selected_stage()",
            "optimized_target()",
        ] {
            assert!(
                !source.contains(forbidden),
                "fixed-frame current input walks history: {forbidden}"
            );
        }
    }
}

#[test]
fn unit_realization_and_identity_routing_consume_current_allocation() {
    let root = repository().join("omega-rust/omega/pipeline/omega-optimization-pipeline/src");
    for family in [
        "unit_function_relative_realization",
        "structural_unit_function_relative_realization",
    ] {
        let source = rust_source(&root.join("stages/realization").join(family));
        assert!(source.contains("allocation: RetainedAllocation"));
        assert!(source.contains("replay_allocation()"));
        assert!(source.contains("AllocationEvidence::RegisterHomes(source)"));
        for forbidden in [
            "StagedOptimizedRegisterHomes",
            "legality_stage()",
            "selected_stage(",
            "optimized_target()",
        ] {
            assert!(
                !source.contains(forbidden),
                "{family} recovers current data from history: {forbidden}"
            );
        }
    }
    let route =
        std::fs::read_to_string(root.join("coordination/physical_pipeline/routes/identity.rs"))
            .unwrap();
    assert!(route.contains("RetainedAllocation::try_from(homes)"));
    assert!(route.contains("current.selected_plan()"));
    assert!(route.contains("current.budget_per_pass()"));
    for forbidden in ["legality_stage()", "selected_stage()", "optimized_target()"] {
        assert!(
            !route.contains(forbidden),
            "identity route walks history: {forbidden}"
        );
    }
}

#[test]
fn post_allocation_realization_and_emission_do_not_select_allocation_history() {
    let root = repository().join("omega-rust/omega/pipeline/omega-optimization-pipeline/src");
    let realization = root.join("stages/realization/function_relative_realization");
    assert!(
        !realization
            .join("routes/post_allocation_machine/allocation_recovery.rs")
            .exists()
    );
    let route =
        std::fs::read_to_string(realization.join("routes/post_allocation_machine.rs")).unwrap();
    assert!(route.contains("RetainedAllocation"));
    assert!(route.contains("replay_allocation"));
    for obsolete in [
        "_after_",
        "steps().last()",
        "selected_stage()",
        "source_legality_stage",
    ] {
        assert!(
            !route.contains(obsolete),
            "realization depends on allocation history: {obsolete}"
        );
    }
    let consumers = rust_source(&root.join("stages/artifacts/function_fragment_emission"));
    let carriers = std::fs::read_to_string(realization.join("carriers.rs")).unwrap();
    for obsolete in [
        "StagedPostAllocationMachineFunctionRelativeSource",
        "PostAllocationMachineFunctionRelativeSourceCustody",
    ] {
        assert!(!consumers.contains(obsolete));
        assert!(!carriers.contains(obsolete));
    }
}

#[test]
fn rematerialization_uses_the_common_encoding_and_layout_stages() {
    let pipeline = repository().join("omega-rust/omega/pipeline");
    assert!(
        !pipeline
            .join("omega-active-resident-rematerialization-to-selected-form-encoding/Cargo.toml")
            .exists()
    );
    let stages = pipeline.join("omega-optimization-pipeline/src/stages");
    for retired in [
        "layout/active_resident_resolved_selected_form_layout/mod.rs",
        "realization/active_resident_function_relative_realization/mod.rs",
    ] {
        assert!(
            !stages.join(retired).exists(),
            "retired history-specific stage: {retired}"
        );
    }
    let source = rust_source(&stages);
    assert!(!source.contains("StagedOptimizedActiveResidentRematerializationSelectedFormEncoding"));
    assert!(
        !source
            .contains("StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout")
    );
}

#[test]
fn physical_coordination_shares_selection_and_does_not_fork_machine_rules_by_history() {
    let root = repository().join(
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/coordination/physical_pipeline",
    );
    let composition = std::fs::read_to_string(root.join("routes/composition/model.rs")).unwrap();
    assert!(!composition.contains("after_selected_lowering"));
    assert!(!composition.contains("SelectedLoweringWithFunctionRelativeLayout"));
    let entrance = std::fs::read_to_string(root.join("mod.rs")).unwrap();
    assert_eq!(
        entrance
            .matches("::stage_optimized_instruction_selection(")
            .count(),
        1
    );
    for route in [
        "routes/selected_phases.rs",
        "routes/allocation_recovery/mod.rs",
    ] {
        let source = std::fs::read_to_string(root.join(route)).unwrap();
        assert!(!source.contains("::stage_optimized_instruction_selection("));
        assert!(!source.contains("stage_optimized_liveness("));
    }
    let machine_route = std::fs::read_to_string(root.join("routes/selected_phases.rs")).unwrap();
    assert_eq!(
        machine_route
            .matches("stage_post_allocation_machine_function_relative_realization(")
            .count(),
        1
    );
    assert!(machine_route.contains("stage_register_allocation(ranges)"));
    let recovery = rust_source(&root.join("routes/allocation_recovery"));
    for owned_by_allocation in [
        "SpillChoicePolicy",
        "RecoveryClassificationPolicy",
        "FixedViewCopyPolicy",
        "stage_optimized_selected_reanalysis(",
        "budget_per_pass()",
    ] {
        assert!(
            !recovery.contains(owned_by_allocation),
            "coordinator owns allocation details: {owned_by_allocation}"
        );
    }
}

#[test]
fn completed_physical_results_and_emission_do_not_fork_by_history() {
    let root = repository().join("omega-rust/omega/pipeline/omega-optimization-pipeline/src");
    let model =
        std::fs::read_to_string(root.join("coordination/physical_pipeline/model.rs")).unwrap();
    assert!(model.contains("pub struct StagedOptimizedVerifiedPhysicalPipeline"));
    assert!(!model.contains("pub enum StagedOptimizedVerifiedPhysicalPipeline"));
    for ancestry in [
        "selected_stage()",
        "legality_stage()",
        "selected_lowering_run()",
    ] {
        assert!(!model.contains(ancestry));
    }
    let compute = root.join("stages/artifacts/function_fragment_emission/compute");
    for retired in [
        "allocation_recovery",
        "post_allocation_machine",
        "selected_lowering",
        "x86_rel8",
        "unit_baseline",
        "fixed_frame",
    ] {
        assert!(!compute.join(format!("{retired}.rs")).exists());
    }
    for file in ["mod.rs", "ordinary.rs", "structural_unit.rs"] {
        let source = std::fs::read_to_string(compute.join(file)).unwrap();
        assert!(!source.contains("StagedOptimizedFunctionFragmentEmissionSource::"));
        assert!(!source.contains("FunctionFragmentReplayInputs::"));
        assert!(!source.contains("selected_stage()"));
        assert!(!source.contains("steps().last()"));
    }
    let recovery =
        root.join("stages/realization/allocation_recovery_function_relative_realization");
    assert!(!recovery.join("source/mod.rs").exists());
    let source = rust_source(&recovery);
    assert!(source.contains("RetainedAllocation"));
    assert!(source.contains("replay_allocation()"));
    assert!(!source.contains("StagedAllocationRecoveryFunctionRelativeSource"));
    for ancestry in [
        "reanalysis_stage()",
        "source_legality_stage()",
        "selected_stage()",
    ] {
        assert!(!source.contains(ancestry));
    }
}

#[test]
fn fragment_consumers_read_current_data_and_only_replay_walks_history() {
    let root = repository();
    let data = rust_source(&root.join("omega-rust/omega/representations/omega-machine-code/src"));
    let pipeline = root.join("omega-rust/omega/pipeline/omega-optimization-pipeline/src");
    assert_eq!(
        data.matches("pub struct ResolvedMachineProgram {").count(),
        1
    );
    assert!(!rust_source(&pipeline).contains("pub struct ResolvedMachineProgram {"));
    let emission = pipeline.join("stages/artifacts/function_fragment_emission");
    let source = std::fs::read_to_string(emission.join("source.rs")).unwrap();
    assert!(source.contains("pub struct StagedOptimizedFunctionFragmentEmissionSource"));
    assert!(!source.contains("pub enum StagedOptimizedFunctionFragmentEmissionSource"));
    for ancestry in [
        "legality_stage()",
        "selected_stage()",
        "selected_lowering_run()",
        "steps().last()",
    ] {
        assert!(
            !source.contains(ancestry),
            "current accessor walks {ancestry}"
        );
    }
    let custody = std::fs::read_to_string(emission.join("custody.rs")).unwrap();
    assert!(custody.contains("match source.replay()"));
    assert!(custody.contains("source.validate_current()?"));
    for consumer in [
        "stages/artifacts/function_fragment_frame_application",
        "stages/artifacts/function_fragment_text_section/placement",
        "stages/artifacts/function_fragment_emission/compute",
    ] {
        let text = rust_source(&pipeline.join(consumer));
        for history in [
            "FunctionFragmentReplayInputs",
            ".replay()",
            ".fixed_frame_realization()",
            ".selected_stage()",
        ] {
            assert!(
                !text.contains(history),
                "{consumer} reads history through {history}"
            );
        }
    }
}
