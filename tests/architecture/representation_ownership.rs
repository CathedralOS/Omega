//! Representation roots and pipeline ownership at migrated boundaries.
//!
//! Each entry here names a completed boundary, not an exemption for others.
//! Add a representation only when its program root and subordinate owners exist.

use std::path::{Path, PathBuf};

#[path = "representation_ownership/allocation_analysis.rs"]
mod allocation_analysis;
#[path = "representation_ownership/selected_analysis.rs"]
mod selected_analysis;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

#[test]
fn retired_assignment_program_and_emission_owners_are_absent() {
    let root = repository();
    for retired in [
        "omega-rust/omega/representations/assigned-target-operations",
        "omega-rust/omega/pipeline/target-operations-to-assigned-target-operations",
        "omega-rust/omega/backend/machine-emission/src/unit",
    ] {
        assert!(
            !root.join(retired).join("Cargo.toml").is_file()
                && (!root.join(retired).exists()
                    || rust_source(&root.join(retired)).trim().is_empty()),
            "retired owner remains: {retired}"
        );
    }
}

#[test]
fn selected_form_encoding_data_outlives_its_producer() {
    let root = repository();
    let representation = root.join("omega-rust/omega/representations/machine-code/src");
    let data = rust_source(&representation);
    let stage = root
        .join("omega-rust/omega/pipeline/post-allocation-machine-to-selected-form-encoding/src");
    let producer = rust_source(&stage);
    for declaration in [
        "pub struct SelectedFormEncoding {",
        "pub struct SelectedFormEncodingRow {",
        "pub enum SelectedFormEncodingState {",
        "pub struct SelectedFormDecodedFootprint {",
        "pub struct ResolvedPhysicalAddress {",
    ] {
        assert_eq!(data.matches(declaration).count(), 1, "{declaration}");
        assert!(
            !producer.contains(declaration),
            "producer owns {declaration}"
        );
    }
    assert!(!data.contains("post_allocation_machine_to_post_allocation_machine::"));
    assert!(!data.contains("pub struct StagedOptimizedSelectedFormEncoding {"));
    let admission = std::fs::read_to_string(stage.join("model.rs")).unwrap();
    assert!(admission.contains("Arc<SelectedFormEncoding>"));
    assert!(admission.contains("pub fn shared_program("));
    assert!(!admission.contains("pub fn from_program("));
    let current = std::fs::read_to_string(
        root.join("omega-rust/omega/backend/machine-emission/src/fragment_emission/current.rs"),
    )
    .unwrap();
    assert!(current.contains("encoding: replay.encoding().shared_program()"));
    assert!(!current.contains("StagedOptimizedSelectedFormEncoding"));
}

#[test]
fn structural_calls_share_current_graph_storage_and_publication_owners() {
    let root = repository();
    for package in [
        "legalized-operations",
        "selected-instructions",
        "physical-instructions",
        "machine-code",
    ] {
        let source = rust_source(
            &root
                .join("omega-rust/omega/representations")
                .join(package)
                .join("src"),
        );
        for retired in [
            "pub structural_unit_functions:",
            "pub struct LegalizedStructuralUnitFunction {",
            "pub struct SelectedStructuralUnitFunction {",
            "pub struct PostAllocationStructuralUnitFunction {",
            "pub struct SelectedStructuralUnitFunctionEncoding {",
            "pub struct ResolvedStructuralUnitFunctionLayout {",
            "pub struct StructuralUnitFunctionFragment {",
            "pub struct WholeFunctionStructuralUnitExitEvidence {",
            "StructuralUnitV1",
        ] {
            assert!(
                !source.contains(retired),
                "{package} retains parallel owner {retired}"
            );
        }
    }
    let legalized =
        rust_source(&root.join("omega-rust/omega/representations/legalized-operations/src"));
    assert!(legalized.contains("pub struct LegalizedStructuralContract {"));
    assert!(legalized.contains("pub enum LegalizedScalarArgument {"));
    assert!(legalized.contains("pub result: Option<LegalizedValueDefinition>"));
    let machine = rust_source(&root.join("omega-rust/omega/representations/machine-code/src"));
    assert!(machine.contains("pub frame: Option<crate::TargetFrameLayoutPlan>"));
    assert!(machine.contains("pub symbolic: physical_instructions::PhysicalAddressOperation"));
    let emission = root.join("omega-rust/omega/backend/machine-emission/src");
    for retired in [
        "function_realization/structural_unit",
        "fragments/production/structural_unit",
        "fragments/validation/structural.rs",
        "text_placement/production/structural_unit.rs",
    ] {
        assert!(
            !emission.join(retired).exists(),
            "parallel realization survives: {retired}"
        );
    }
    let replay = std::fs::read_to_string(root.join("omega-rust/omega/backend/images/image-emission/src/function_fragments/structural/validation.rs")).unwrap();
    for forbidden in [
        "= settlement_attributions(",
        "structural::settlement_attributions(",
        "settlement_offset(",
        "copy_extent(",
        "populate(",
    ] {
        assert!(
            !replay.contains(forbidden),
            "object metadata replay reenters producer {forbidden}"
        );
    }
    for required in [
        "validate_copy(",
        "validate_settlement_position(",
        "completion_claim_sources",
        "validate_completion_custody(",
    ] {
        assert!(
            replay.contains(required),
            "independent structural metadata replay loses {required}"
        );
    }
}

#[test]
fn native_coordination_and_target_setup_are_not_program_stages() {
    let root = repository();
    let pipeline = root.join("omega-rust/omega/pipeline");
    for retired in [
        "terminal-psi-to-native-artifact",
        "native-realization",
        "target-to-register-environment",
        "register-environment",
        "post-allocation-machine-to-frame-layout",
        "optimization-validation",
    ] {
        assert!(!pipeline.join(retired).join("Cargo.toml").exists());
    }
    let entrances = std::fs::read_dir(&pipeline)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("terminal-psi-to-"))
        .collect::<Vec<_>>();
    assert_eq!(entrances, ["terminal-psi-to-abstract-operations"]);
    let coordinator = std::fs::read_to_string(
        root.join("omega-rust/omega/compiler/native-realization/Cargo.toml"),
    )
    .unwrap();
    assert!(coordinator.contains("../../pipeline/terminal-psi-to-abstract-operations"));
    assert!(coordinator.contains("../../backend/register-environment"));
    let setup = std::fs::read_to_string(
        root.join("omega-rust/omega/backend/register-environment/Cargo.toml"),
    )
    .unwrap();
    assert!(setup.contains("isa-aarch64"));
    assert!(setup.contains("isa-x86_64"));
    assert!(!setup.contains("/pipeline/"));
}

#[test]
fn optimization_records_and_independent_checks_have_distinct_owners() {
    let root = repository();
    let representation = root.join("omega-rust/omega/representations/optimization-unit/src");
    let program = std::fs::read_to_string(representation.join("optimization_unit.rs")).unwrap();
    assert!(program.contains("pub struct PsiOptimizationUnit"));
    assert!(!representation.join("model.rs").exists());
    let records = rust_source(&representation);
    for record in [
        "PrePhysicalOptimizationManifest",
        "OptimizerCycleComponentSnapshot",
        "OptimizerRankingCertificateSnapshot",
    ] {
        assert_eq!(
            records.matches(&format!("pub struct {record} {{")).count(),
            1
        );
    }
    assert!(!records.contains("pub struct ValidatedPrePhysicalOptimizationManifest"));
    assert!(!records.contains("pub struct ValidatedOptimizerCycleComponents"));
    let semantics = root.join("omega-rust/omega/semantics/optimization-unit-semantics");
    let manifest = std::fs::read_to_string(semantics.join("Cargo.toml")).unwrap();
    let production = manifest.split("[dev-dependencies]").next().unwrap();
    assert!(!production.contains("/pipeline/"));
    let validators = rust_source(&semantics.join("src"));
    assert!(!validators.contains("VerifiedPsiOptimizationInput"));
    assert!(!validators.contains("pub fn project_pre_physical_optimization_manifest"));
    let stage = root.join(
        "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/validation",
    );
    let custody = std::fs::read_to_string(stage.join("context/ranked_cycles/model.rs")).unwrap();
    assert!(custody.contains("pub(in crate::validation::context) const fn new"));
    assert!(!custody.contains("pub(crate) const fn new"));
    let context = std::fs::read_to_string(stage.join("context/mod.rs")).unwrap();
    let admission = context
        .find("let cycle_admission = ranked_cycles::validate_exact_ranked_cycles")
        .unwrap();
    let structure = context
        .find("validate_psi_optimization_unit_with_admitted_cycle_machines(unit")
        .unwrap();
    assert!(admission < structure);
}

#[test]
fn optimization_phase_directories_name_both_identical_endpoints() {
    let root = repository();
    let pipeline = root.join("omega-rust/omega/pipeline");
    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    for representation in [
        "abstract-operations",
        "resolved-layout",
        "selected-instructions",
    ] {
        let name = format!("{representation}-to-{representation}");
        let manifest = std::fs::read_to_string(pipeline.join(&name).join("Cargo.toml"))
            .unwrap_or_else(|error| panic!("missing X-to-X phase {name}: {error}"));
        assert!(manifest.contains(&format!("name = \"{name}\"")));
        assert!(workspace.contains(&format!("omega/pipeline/{name}\"")));
    }
    for retired in [
        "omega-abstract-operations-optimizer",
        "omega-post-allocation-machine-to-optimized-machine",
    ] {
        assert!(!pipeline.join(retired).exists(), "retired phase: {retired}");
        assert!(!workspace.contains(retired));
    }
}

#[test]
fn generic_data_normalization_is_private_work_inside_name_resolution() {
    let root = repository();
    assert!(
        !root
            .join("omega-rust/psi/pipeline/psi-generic-instances/Cargo.toml")
            .exists()
    );
    let owner = root.join("omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src");
    let entrance = std::fs::read_to_string(owner.join("lib.rs")).unwrap();
    assert!(entrance.contains("mod generic_data;"));
    assert!(entrance.contains("pub use generic_data::{"));
    for operation in [
        "normalize_generic_data",
        "normalize_generic_data_with_sources_and_top_level_bindings",
    ] {
        assert!(entrance.contains(operation));
    }
    let normalization = std::fs::read_to_string(owner.join("generic_data/mod.rs")).unwrap();
    assert!(normalization.contains("mut syntax: SyntaxTrees"));
    assert!(normalization.contains("Result<SyntaxTrees, Vec<Diagnostic>>"));
    let scratch = std::fs::read_to_string(owner.join("generic_data/discovery.rs")).unwrap();
    for name in ["GenericData", "PendingRewrite", "Instantiation"] {
        assert!(scratch.contains(&format!("pub(super) struct {name}")));
        assert!(
            !entrance.contains(name),
            "working state is not a public program representation"
        );
    }
    let declarations = std::fs::read_to_string(owner.join("item.rs")).unwrap();
    assert!(
        declarations
            .contains("crate::generic_data::canonicalize_selected_declared_const_definition")
    );
    let selection =
        std::fs::read_to_string(owner.join("generic_data/constant_selection.rs")).unwrap();
    assert!(selection.contains("pub(crate) struct ConstantSelection"));
    assert!(selection.contains("find_top_level_by_name_and_kinds_from_source"));
    assert!(!entrance.contains("ConstantSelection"));
}

#[test]
fn frame_records_are_data_and_backend_validation_remains_sealed() {
    let root = repository();
    let records = rust_source(&root.join(
        "omega-rust/omega/representations/machine-code/src/machine_code/storage/frame_layout",
    ));
    for name in [
        "TargetFrameLayoutPlan",
        "NonAuthoritativeCalleeSaveStoragePlan",
        "NonAuthoritativeSpillFrameRequirementPlan",
    ] {
        assert!(records.contains(&format!("pub struct {name}")));
    }
    for forbidden in [
        "ValidatedTarget",
        "ValidatedNonAuthoritative",
        "stage_",
        "fn compute",
    ] {
        assert!(
            !records.contains(forbidden),
            "raw frame records contain {forbidden}"
        );
    }
    let allocation = std::fs::read_to_string(root.join(
        "omega-rust/omega/representations/register-homes/src/register_homes/preservation.rs",
    ))
    .unwrap();
    assert!(allocation.contains("pub struct AllocatedCalleeSavedRequirementPlan"));
    assert!(!allocation.contains("pub struct ValidatedAllocated"));
    let backend = root.join("omega-rust/omega/backend/machine-emission/src/frame_layout");
    assert!(
        std::fs::read_to_string(backend.join("model.rs"))
            .unwrap()
            .contains("pub struct ValidatedTargetFrameLayout")
    );
    let manifest = std::fs::read_to_string(
        root.join("omega-rust/omega/representations/machine-code/Cargo.toml"),
    )
    .unwrap();
    assert!(!manifest.contains("/pipeline/"));
    assert!(!manifest.contains("/backend/"));
}

#[test]
fn frame_calculations_have_phase_owners_and_replay_does_not_run_producers() {
    let root = repository();
    for owner in [
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/preservation",
        "omega-rust/omega/backend/machine-emission/src/frame_layout/save_storage",
        "omega-rust/omega/backend/machine-emission/src/frame_layout/spill_requirements",
        "omega-rust/omega/backend/machine-emission/src/frame_layout",
        "omega-rust/omega/backend/machine-emission/src/frame_protocol",
    ] {
        let owner = root.join(owner);
        let validator = std::fs::read_to_string(owner.join("validation.rs")).unwrap();
        let replay = if owner.join("replay.rs").exists() {
            std::fs::read_to_string(owner.join("replay.rs")).unwrap()
        } else {
            rust_source(&owner.join("replay"))
        };
        for source in [&validator, &replay] {
            for forbidden in ["compute::", "super::compute", "function_layout("] {
                assert!(
                    !source.contains(forbidden),
                    "{}: replay imports {forbidden}",
                    owner.display()
                );
            }
        }
    }
}

#[test]
fn effect_records_do_not_derive_typed_body_or_provider_summaries() {
    let root = repository();
    let effects = root.join("omega-rust/psi/representations/flow-effects");
    let manifest = std::fs::read_to_string(effects.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("typed-trees"));
    let records = rust_source(&effects.join("src"));
    for forbidden in [
        "typed_trees",
        "fn infer_",
        "struct MachineWork",
        "struct MachineReachWork",
    ] {
        assert!(
            !records.contains(forbidden),
            "effect records contain {forbidden}"
        );
    }
    let inference =
        rust_source(&root.join("omega-rust/psi/semantics/validation/src/effect_inference"));
    for function in [
        "infer_operational_may",
        "infer_service_reaches",
        "infer_synchronous_invocations",
    ] {
        assert_eq!(inference.matches(&format!("pub fn {function}(")).count(), 1);
    }
    let provider_records = std::fs::read_to_string(
        root.join("omega-rust/omega/representations/effects/src/capabilities/provider_plan.rs"),
    )
    .unwrap();
    let provider_derivation = std::fs::read_to_string(
        root.join("omega-rust/omega/build/provider-planning/src/service_schema.rs"),
    )
    .unwrap();
    for function in [
        "from_typed",
        "from_typed_instance",
        "from_typed_operator",
        "from_typed_boundary_requirement",
    ] {
        assert!(!provider_records.contains(&format!("pub fn {function}(")));
        assert_eq!(
            provider_derivation
                .matches(&format!("pub fn {function}("))
                .count(),
            1
        );
    }
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
fn optimization_decision_records_do_not_own_candidate_selection() {
    let root = repository();
    let records =
        rust_source(&root.join("omega-rust/omega/representations/optimization-core/src/decisions"));
    assert!(records.contains("pub struct BaselineDecisionLogBuilder"));
    assert!(!records.contains("fn choose("));
    assert!(!records.contains("fn choose_baseline("));
    assert!(!records.contains("abstract_operations_to_abstract_operations::"));
    let chooser = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/baseline.rs",
    )).unwrap();
    assert!(chooser.contains("pub(super) fn choose_baseline("));
    assert!(
        !root
            .join("omega-rust/omega/pipeline/omega-optimization-policy/Cargo.toml")
            .exists()
    );
}

#[test]
fn text_placement_data_and_independent_checking_have_separate_owners() {
    let root = repository();
    let data = rust_source(&root.join(
        "omega-rust/omega/representations/machine-code/src/machine_code/layout/text_section",
    ));
    assert!(data.contains("omega.terminal.relocation-free-text-section.v9"));
    let record = std::fs::read_to_string(root.join(
        "omega-rust/omega/representations/machine-code/src/machine_code/layout/text_section.rs",
    ))
    .unwrap();
    assert!(record.contains("pub struct RelocationFreeTextSectionPlacement"));
    let coordinator =
        root.join("omega-rust/omega/backend/machine-emission/src/text_placement/custody");
    let placement = rust_source(&coordinator.join("placement"));
    assert!(placement.contains("place_fragment_text_section"));
    assert!(!placement.contains("PlacedFunctionFragment {"));
    let replay = std::fs::read_to_string(coordinator.join("validation.rs")).unwrap();
    assert!(replay.contains("validate_fragment_text_section"));
    assert!(!replay.contains("compute("));
    assert!(!replay.contains("compute_fixed_frame("));
    let backend = root.join("omega-rust/omega/backend/machine-emission/src/text_placement");
    let checker = rust_source(&backend.join("validation"));
    for forbidden in [
        "production::",
        "place_fragment_text_section(",
        "resolve_x86_64_structural_unit_internal_call",
        "PlacedFunctionFragment {",
        "PlacedInternalMachineCallResolution {",
    ] {
        assert!(
            !checker.contains(forbidden),
            "checker re-enters production: {forbidden}"
        );
    }
    // Admission/replay joins live in custody; the placement algorithm and its
    // independent checker still consume only current fragment data.
    let source = format!(
        "{}\n{}\n{}",
        rust_source(&backend.join("production")),
        checker,
        rust_source(&backend.join("source")),
    );
    for forbidden in [
        "StagedOptimized",
        "FunctionFragmentReplayInputs",
        "object_file::",
    ] {
        assert!(
            !source.contains(forbidden),
            "placement depends on upstream stage: {forbidden}"
        );
    }
}

#[test]
fn text_publication_records_and_codec_belong_to_the_representation() {
    let root = repository();
    let representation = root.join("omega-rust/omega/representations/machine-code/src/machine_code/layout/text_section/publication.rs");
    let data = std::fs::read_to_string(&representation).unwrap();
    for declaration in [
        "pub struct FunctionFragmentTextSectionManifest",
        "pub struct FunctionFragmentTextSectionStatistics",
        "pub frame_application: FunctionFragmentFrameApplicationIdentity",
    ] {
        assert!(data.contains(declaration));
    }
    let codec = rust_source(&representation.with_extension(""));
    assert!(codec.contains("const MANIFEST_VERSION: u32 = 16;"));
    assert!(!data.contains("pub enum FunctionFragmentTextSectionSourceCustody"));
    assert!(!data.contains("source_kind:"));
    for forbidden in [
        "native_realization::",
        "machine_emission::",
        "post_allocation_machine_to_post_allocation_machine::",
        "object_file::",
    ] {
        assert!(
            !data.contains(forbidden) && !codec.contains(forbidden),
            "representation imports producer: {forbidden}"
        );
    }
    let coordinator =
        root.join("omega-rust/omega/backend/machine-emission/src/text_placement/custody");
    let source = rust_source(&coordinator);
    assert!(!source.contains("pub struct FunctionFragmentTextSectionManifest {"));
    assert!(!source.contains("fn encode_manifest_content"));
    assert!(!source.contains("fn statistics("));
    assert!(source.contains("Arc<FunctionFragmentTextSectionManifest>"));
    assert!(source.contains("text_section_statistics(section, fragments)"));
}

#[test]
fn selected_program_has_one_representation_entrance() {
    let directory = repository().join("omega-rust/omega/representations/selected-instructions/src");
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
    for (half, package, module, program, areas) in [
        (
            "psi",
            "facts",
            "fact_plan",
            "FactPlan",
            &["places", "contexts", "evidence"][..],
        ),
        (
            "omega",
            "machine-code",
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
            "omega",
            "physical-instructions",
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
            "omega",
            "register-homes",
            "register_homes",
            "AllocatedProgram",
            &["storage", "constraints", "recovery", "identity", "codec"][..],
        ),
        (
            "omega",
            "abstract-operations",
            "abstract_operations",
            "AbstractOperationPlan",
            &["control_flow", "values", "calls", "ownership", "operations"][..],
        ),
        (
            "omega",
            "target-operations",
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
            "omega",
            "legalized-operations",
            "legalized_operations",
            "LegalizedOperationPlan",
            &["control_flow", "calls", "legality", "identity"][..],
        ),
        (
            "psi",
            "syntax-trees",
            "syntax_trees",
            "SyntaxTrees",
            &[
                "declarations",
                "control_flow",
                "values",
                "type_system",
                "names",
                "inspection",
            ][..],
        ),
        (
            "psi",
            "symbol-resolved-trees",
            "symbol_resolved_trees",
            "SymbolResolvedTrees",
            &[
                "declarations",
                "control_flow",
                "calls",
                "values",
                "type_system",
                "names",
                "evidence",
                "storage",
                "inspection",
            ][..],
        ),
        (
            "psi",
            "typed-trees",
            "typed_trees",
            "TypedTrees",
            &[
                "declarations",
                "control_flow",
                "calls",
                "values",
                "type_system",
                "names",
                "evidence",
                "inspection",
            ][..],
        ),
    ] {
        let directory = repository()
            .join(format!("omega-rust/{half}/representations"))
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
fn every_psi_representation_has_one_named_entry() {
    let root = repository().join("omega-rust/psi/representations");
    let entries = [
        ("checked-trees", "checked_trees"),
        ("facts", "fact_plan"),
        ("flow-effects", "flow_effects"),
        ("lowered-psi", "lowered_psi"),
        ("optimization", "optimization_selections"),
        ("symbol-resolved-trees", "symbol_resolved_trees"),
        ("syntax-trees", "syntax_trees"),
        ("terminal-psi", "terminal_module"),
        ("tokens", "token_stream"),
        ("typed-trees", "typed_trees"),
    ];
    let mut packages = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("Cargo.toml").exists())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    packages.sort();
    assert_eq!(packages, entries.map(|(package, _)| package));
    for (package, entry) in entries {
        let directory = root.join(package).join("src");
        let mut files = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        files.sort();
        let mut expected = vec!["lib.rs".to_owned(), format!("{entry}.rs")];
        expected.sort();
        assert_eq!(files, expected, "ambiguous entry in {package}");
        let entrance = std::fs::read_to_string(directory.join("lib.rs")).unwrap();
        assert!(entrance.contains(&format!("mod {entry};")));
    }
    let facts = rust_source(&root.join("facts/src"));
    assert!(!facts.contains("fn build_definition_fact_plan"));
    assert!(!facts.contains("fn append_domain_definition_facts"));
    let producer = std::fs::read_to_string(
        repository().join("omega-rust/psi/semantics/validation/src/definition_facts.rs"),
    )
    .unwrap();
    assert!(producer.contains("pub fn build_definition_fact_plan"));
}

#[test]
fn allocation_algorithms_and_staging_have_one_transform_owner() {
    let root = repository();
    let retired = root.join("omega-rust/omega/pipeline/omega-regalloc");
    assert!(!retired.join("Cargo.toml").exists());
    assert!(!retired.join("src/lib.rs").exists());
    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(!workspace.contains("omega-regalloc"));

    let owner =
        root.join("omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src");
    let source = rust_source(&owner);
    assert!(!source.contains("omega_regalloc::"));
    for declaration in [
        "pub fn analyze_liveness",
        "pub fn analyze_liveness_reusing",
        "pub fn validate_liveness",
        "pub fn stage_optimized_liveness",
        "pub fn analyze_live_ranges",
        "pub fn analyze_live_ranges_reusing",
        "pub fn validate_live_ranges",
        "pub fn stage_optimized_live_ranges",
    ] {
        // A suffixed API is not another definition of this function.
        let declarations = source.matches(&format!("{declaration}(")).count()
            + source.matches(&format!("{declaration}<")).count();
        assert_eq!(declarations, 1, "{declaration}");
    }
    let allocation = rust_source(
        &root.join("omega-rust/omega/pipeline/selected-instructions-to-register-homes/src"),
    );
    assert_eq!(
        allocation
            .matches("pub fn stage_register_allocation")
            .count(),
        1
    );
    assert!(!source.contains("pub fn stage_register_allocation"));
    assert!(!allocation.contains("pub fn run_selected_lowering_optimizations"));
    // Assignment may obtain target ABI policy, but the read-only liveness and
    // interval algorithms consume explicit selected facts and register data.
    for analysis in ["liveness", "live_ranges"] {
        let source = rust_source(&owner.join("analyses").join(analysis));
        for forbidden in ["omega_isa_", "machine_emission", "native_realization"] {
            assert!(
                !source.contains(forbidden),
                "{analysis} depends on {forbidden}"
            );
        }
    }
}

#[test]
fn register_home_data_is_independent_of_allocation_authority() {
    let owner = repository().join("omega-rust/omega/representations/register-homes");
    let representation = rust_source(&owner.join("src"));
    let allocator = rust_source(
        &repository().join("omega-rust/omega/pipeline/selected-instructions-to-register-homes/src"),
    );
    for declaration in [
        "pub struct RegisterHomePlan {",
        "pub struct FunctionRegisterHomes {",
        "pub struct VirtualRegisterHome {",
        "pub struct RegisterHomeIdentity(",
        "pub struct AllocationLegalityIdentity(",
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
    assert!(!manifest.contains("selected-instructions-to-register-homes"));
    assert!(!manifest.contains("/pipeline/"));
}

#[test]
fn physical_instruction_data_is_independent_of_optimizer_authority() {
    let owner = repository().join("omega-rust/omega/representations/physical-instructions");
    let representation = rust_source(&owner.join("src"));
    let optimizer = rust_source(
        &repository()
            .join("omega-rust/omega/pipeline/register-homes-to-post-allocation-machine/src"),
    );
    for declaration in [
        "pub struct PostAllocationMachinePlan {",
        "pub struct PostAllocationMachineFunction {",
        "pub struct PostAllocationMachineBlock {",
        "pub struct PostAllocationMachineInstruction {",
        "pub enum PhysicalAddressOperation {",
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
    let construction = rust_source(
        &repository()
            .join("omega-rust/omega/pipeline/register-homes-to-post-allocation-machine/src"),
    );
    assert!(construction.contains("pub struct ValidatedPostAllocationMachinePlan {"));
    let manifest = std::fs::read_to_string(owner.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("post-allocation-machine-to-post-allocation-machine"));
    assert!(!manifest.contains("selected-instructions-to-register-homes"));
    assert!(!manifest.contains("/pipeline/"));
}

#[test]
fn machine_construction_precedes_and_does_not_depend_on_optimization() {
    let root = repository();
    let pipeline = root.join("omega-rust/omega/pipeline");
    assert!(!pipeline.join("omega-machine-optimizer/Cargo.toml").exists());
    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(!workspace.contains("omega-machine-optimizer"));

    for (owner, entry) in [
        (
            "selected-instructions-to-selected-instructions",
            "pub fn analyze_pre_allocation_machine_effects",
        ),
        (
            "register-homes-to-post-allocation-machine",
            "pub fn analyze_post_allocation_machine_plan",
        ),
    ] {
        let source = rust_source(&pipeline.join(owner).join("src"));
        assert_eq!(source.matches(entry).count(), 1);
        let manifest = std::fs::read_to_string(pipeline.join(owner).join("Cargo.toml")).unwrap();
        for forbidden in [
            "omega-machine-optimizer",
            "post-allocation-machine-to-post-allocation-machine",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "{owner} depends on later {forbidden}"
            );
        }
    }
    assert!(
        !pipeline
            .join("post-allocation-machine-to-post-allocation-machine/Cargo.toml")
            .exists()
    );
    let physical =
        rust_source(&root.join("omega-rust/omega/representations/physical-instructions/src"));
    assert_eq!(physical.matches("pub struct TargetCostModel {").count(), 1);
}

#[test]
fn structural_memory_and_call_data_do_not_require_the_isa_implementation() {
    let root = repository();
    let machine = rust_source(&root.join("omega-rust/omega/representations/machine-code/src"));
    let physical =
        rust_source(&root.join("omega-rust/omega/representations/physical-instructions/src"));
    let selected =
        rust_source(&root.join("omega-rust/omega/representations/selected-instructions/src"));
    let isa = rust_source(
        &root.join("omega-rust/omega/backend/instruction_set_architectures/isa-x86_64/src"),
    );
    for (owner, declaration) in [
        (&machine, "pub struct ResolvedPhysicalAddress {"),
        (&machine, "pub struct SelectedFormInternalMachineFixup {"),
        (&physical, "pub enum PhysicalAddressOperation {"),
        (&selected, "pub struct SelectedCallContract {"),
        (&selected, "pub struct SelectedMemoryAccess {"),
        (&selected, "pub struct SelectedOutgoingArgumentSlot {"),
    ] {
        assert_eq!(owner.matches(declaration).count(), 1, "{declaration}");
        assert!(!isa.contains(declaration), "ISA still owns {declaration}");
    }
    assert!(isa.contains("pub fn validate_x86_64_selected_memory_form("));
    assert!(!machine.contains("ValidatedX86_64SelectedStructuralUnitCallTemplate"));
    assert!(!isa.contains("ValidatedX86_64SelectedStructuralUnitCallTemplate"));
    for package in [
        "machine-code",
        "physical-instructions",
        "selected-instructions",
    ] {
        let manifest = std::fs::read_to_string(
            root.join("omega-rust/omega/representations")
                .join(package)
                .join("Cargo.toml"),
        )
        .unwrap();
        assert!(!manifest.contains("/backend/"));
        assert!(!manifest.contains("/pipeline/"));
    }
}

#[test]
fn resolved_layout_data_and_identity_do_not_require_a_producing_stage() {
    let root = repository();
    let machine = rust_source(&root.join("omega-rust/omega/representations/machine-code/src"));
    let physical =
        rust_source(&root.join("omega-rust/omega/representations/physical-instructions/src"));
    let pipeline = rust_source(&root.join("omega-rust/omega/pipeline"));
    for declaration in [
        "pub struct ResolvedMachineLayout {",
        "pub struct ResolvedSelectedFunctionLayout {",
        "pub struct ResolvedSelectedFormRow {",
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
    assert!(!machine.contains("post_allocation_machine_to_post_allocation_machine::"));
    assert!(!machine.contains("pub struct StagedOptimizedResolvedSelectedFormLayout"));
    assert!(machine.contains("omega.terminal.resolved-selected-form-layout.v16"));
    assert!(!pipeline.contains("omega.terminal.resolved-selected-form-layout.v16"));

    let stage = root.join("omega-rust/omega/pipeline/selected-form-encoding-to-resolved-layout/src/resolved_selected_form_layout");
    let wrapper = std::fs::read_to_string(stage.join("model.rs")).unwrap();
    assert!(wrapper.contains("program: Arc<ResolvedMachineLayout>"));
    assert!(wrapper.contains("Arc::clone(&self.program)"));
    assert!(!wrapper.contains("pub(super) functions:"));
    assert!(!wrapper.contains("pub(super) structural_unit_functions:"));
    let admission = std::fs::read_to_string(stage.join("mod.rs")).unwrap();
    assert!(admission.contains("pub fn admit_resolved_machine_layout"));
    assert!(
        admission
            .contains("validation::validate(selected, machine, physical, pre_layout, &artifact)?")
    );
    for package in ["machine-code", "physical-instructions"] {
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
fn exit_contract_records_and_identities_are_representation_owned() {
    let root = repository();
    let machine = rust_source(&root.join("omega-rust/omega/representations/machine-code/src"));
    let physical =
        rust_source(&root.join("omega-rust/omega/representations/physical-instructions/src"));
    let pipeline = rust_source(&root.join("omega-rust/omega/pipeline"));
    for declaration in [
        "pub struct WholeFunctionExitContract {",
        "pub struct WholeFunctionExitEvidence {",
        "pub struct WholeFunctionReturnEvidence {",
        "pub struct WholeFunctionExitContractIdentity(",
        "pub struct TargetFrameLayoutIdentity(",
        "pub struct TargetFrameProtocolEncodingIdentity(",
        "pub struct X86BranchRelaxationIdentity(",
    ] {
        assert_eq!(machine.matches(declaration).count(), 1, "{declaration}");
        assert!(
            !pipeline.contains(declaration),
            "producer still owns {declaration}"
        );
    }
    for declaration in [
        "pub struct Aarch64CbnzFusionIdentity(",
        "pub struct Aarch64MovnMaterializationIdentity(",
    ] {
        assert_eq!(physical.matches(declaration).count(), 1, "{declaration}");
        assert!(
            !pipeline.contains(declaration),
            "producer still owns {declaration}"
        );
    }
    let wrapper = "pub struct ValidatedWholeFunctionExitContract {";
    let emission =
        rust_source(&root.join("omega-rust/omega/backend/machine-emission/src/exit_contract"));
    assert!(emission.contains(wrapper));
    assert!(!pipeline.contains(wrapper));
    assert!(!machine.contains(wrapper));
    assert!(machine.contains("omega.terminal.whole-function-exit-contract.v13"));
    assert!(!pipeline.contains("omega.terminal.whole-function-exit-contract.v13"));
    assert!(!machine.contains("post_allocation_machine_to_post_allocation_machine::"));
    assert!(!machine.contains("native_realization::"));
    assert!(
        emission
            .contains("pub fn shared_contract(&self) -> std::sync::Arc<WholeFunctionExitContract>")
    );
}

#[test]
fn exit_replay_checks_claimed_records_without_reentering_the_producer() {
    let root = repository();
    let owner = root.join("omega-rust/omega/backend/machine-emission/src/exit_contract");
    let replay = rust_source(&owner.join("validation"));
    for forbidden in [
        "compute::",
        "compute(",
        "compute_with_frame(",
        "validate_return(",
        "validate_structural_unit_functions(",
        "WholeFunctionExitContract {",
        "WholeFunctionReturnEvidence {",
        "WholeFunctionStructuralUnitCallEvidence {",
        "WholeFunctionStructuralUnitExitEvidence {",
    ] {
        assert!(
            !replay.contains(forbidden),
            "exit replay uses record producer {forbidden}"
        );
    }
    for (file, expected_count) in [("stage.rs", 3), ("layout_optimization.rs", 2)] {
        let entrance = std::fs::read_to_string(owner.join(file)).unwrap();
        assert_eq!(
            entrance.matches("validation::validate(").count(),
            expected_count
        );
        assert!(!entrance.contains("let replayed = compute"));
    }
    let coordinator = rust_source(&root.join("omega-rust/omega/compiler/native-realization/src"));
    assert!(!coordinator.contains("pub fn stage_whole_function_exit_contract"));
    assert!(!coordinator.contains("pub fn validate_whole_function_exit_contract"));
    assert!(!replay.contains("native_realization::"));
}

#[test]
fn fragment_projection_is_backend_owned_and_replay_does_not_emit() {
    let root = repository();
    let backend = root.join("omega-rust/omega/backend/machine-emission/src/fragments");
    let replay = rust_source(&backend.join("validation"));
    for forbidden in [
        "production::",
        "::emit(",
        "FunctionFragmentEmissionPlan {",
        "FunctionFragmentInstructionSpan {",
        "StructuralUnitCallFragmentSpan {",
        "StagedOptimizedFunctionFragmentEmissionSource",
        "FunctionFragmentReplayInputs",
    ] {
        assert!(
            !replay.contains(forbidden),
            "fragment replay depends on {forbidden}"
        );
    }
    let source = rust_source(&backend);
    assert!(!source.contains("native_realization::"));
    assert!(!source.contains("source.replay()"));
    let coordinator = root.join("omega-rust/omega/backend/machine-emission/src/fragment_emission");
    let compute = rust_source(&coordinator.join("compute"));
    assert!(compute.contains("emit_resolved_function_fragments(source.program())"));
    assert!(!compute.contains("bytes.extend_from_slice"));
    assert!(!coordinator.join("compute/ordinary_function.rs").exists());
    assert!(!coordinator.join("compute/structural_unit.rs").exists());
    let entrance = std::fs::read_to_string(coordinator.join("mod.rs")).unwrap();
    assert!(entrance.contains("validate_resolved_function_fragments("));
    assert!(!entrance.contains("compute(&staged.source)"));
    let metadata = std::fs::read_to_string(coordinator.join("validation.rs")).unwrap();
    assert!(!metadata.contains("manifest::seal"));
    assert!(!metadata.contains("FunctionFragmentEmissionManifest {"));
}

#[test]
fn fragment_publication_data_and_codec_do_not_depend_on_admission() {
    let root = repository();
    let representation = root.join(
        "omega-rust/omega/representations/machine-code/src/machine_code/fragments/publication.rs",
    );
    let data = std::fs::read_to_string(&representation).unwrap();
    let pipeline = root.join("omega-rust/omega/backend/machine-emission/src/fragment_emission");
    let coordinator = rust_source(&pipeline);
    for declaration in [
        "pub struct FunctionFragmentEmissionManifest {",
        "pub struct FunctionFragmentEmissionStatistics {",
        "pub enum FunctionFragmentEmissionStage {",
        "pub enum FunctionFragmentEmissionUnavailableData {",
    ] {
        assert!(
            data.contains(declaration),
            "missing representation {declaration}"
        );
        assert!(
            !coordinator.contains(declaration),
            "coordinator owns {declaration}"
        );
    }
    let codec = rust_source(&representation.with_extension(""));
    assert!(codec.contains("omega.function-fragment-emission-manifest.v15"));
    assert!(!coordinator.contains("omega.function-fragment-emission-manifest.v15"));
    assert!(!pipeline.join("manifest.rs").exists());
    assert!(!pipeline.join("statistics.rs").exists());
    for forbidden in [
        "ValidatedFunctionFragmentEmissionManifest",
        "StagedOptimizedFunctionFragmentEmission",
        "native_realization",
    ] {
        assert!(!data.contains(forbidden));
        assert!(!codec.contains(forbidden));
    }
    assert!(coordinator.contains("pub struct ValidatedFunctionFragmentEmissionManifest"));
    assert!(coordinator.contains("function_fragment_emission_statistics"));
    let backend =
        root.join("omega-rust/omega/backend/machine-emission/src/fragments/statistics.rs");
    let counting = std::fs::read_to_string(backend).unwrap();
    assert!(counting.contains("pub fn function_fragment_emission_statistics("));
    assert!(!counting.contains("FunctionFragmentEmissionManifest {"));
}

#[test]
fn applied_frame_data_and_target_mechanics_have_separate_owners() {
    let root = repository();
    let data = rust_source(&root.join("omega-rust/omega/representations/machine-code/src"));
    let pipeline = rust_source(&root.join("omega-rust/omega/compiler/native-realization/src"));
    for name in [
        "FunctionFragmentFrameApplicationIdentity",
        "FunctionAppliedFrameEpilogue",
        "FunctionAppliedFrameProtocol",
        "FunctionFragmentFrameApplication",
    ] {
        let declaration = if name.ends_with("Identity") {
            format!("pub struct {name}(")
        } else {
            format!("pub struct {name} {{")
        };
        assert!(data.contains(&declaration), "missing representation {name}");
        assert!(!pipeline.contains(&declaration), "pipeline owns {name}");
    }
    assert!(data.contains("omega.function-fragment-frame-application.v3"));
    assert!(!pipeline.contains("omega.function-fragment-frame-application.v3"));
    let coordinator = root
        .join("omega-rust/omega/backend/machine-emission/src/fragment_emission/frame_application");
    for retired in [
        "compute.rs",
        "reflow.rs",
        "validation_branch.rs",
        "identity.rs",
    ] {
        assert!(
            !coordinator.join(retired).exists(),
            "coordinator owns {retired}"
        );
    }
    let backend =
        rust_source(&root.join("omega-rust/omega/backend/machine-emission/src/frame_application"));
    assert!(!backend.contains("native_realization::"));
    assert!(!backend.contains("StagedFunctionFragmentFrameApplication"));
    assert!(!data.contains("pub struct StagedFunctionFragmentFrameApplication"));
}

#[test]
fn resolved_layout_transformation_is_owned_outside_the_coordinator() {
    let root = repository();
    let owner = root.join("omega-rust/omega/pipeline/selected-form-encoding-to-resolved-layout");
    let coordinator = root.join("omega-rust/omega/compiler/native-realization/src");
    let algorithms = rust_source(&owner.join("src"));
    let optimization_owner =
        root.join("omega-rust/omega/pipeline/resolved-layout-to-resolved-layout");
    let optimization = rust_source(&optimization_owner.join("src"));
    let orchestration = rust_source(&coordinator);
    for definition in [
        "pub fn stage_optimized_resolved_selected_form_layout<",
        "pub fn admit_resolved_machine_layout<",
    ] {
        assert!(
            algorithms.contains(definition),
            "missing layout entrance {definition}"
        );
        assert!(
            !orchestration.contains(definition),
            "coordinator owns {definition}"
        );
    }
    for definition in [
        "pub fn stage_optimized_x86_branch_relaxation<",
        "pub fn validate_optimized_x86_branch_relaxation<",
        "pub fn execute_resolved_layout_optimization<",
        "pub fn validate_resolved_layout_optimization<",
    ] {
        assert!(
            optimization.contains(definition),
            "missing optimization entrance {definition}"
        );
        assert!(
            !algorithms.contains(definition),
            "baseline owner contains {definition}"
        );
        assert!(
            !orchestration.contains(definition),
            "coordinator owns {definition}"
        );
    }
    assert!(!algorithms.contains("with_replayed_functions"));
    assert!(!algorithms.contains("resolved_layout_to_resolved_layout"));
    assert!(!algorithms.contains("native_realization::"));
    assert!(!optimization.contains("native_realization::"));
    let manifest = std::fs::read_to_string(owner.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("native-realization"));
    assert!(!manifest.contains("resolved-layout-to-resolved-layout"));
    let optimization_manifest =
        std::fs::read_to_string(optimization_owner.join("Cargo.toml")).unwrap();
    assert!(optimization_manifest.contains("selected-form-encoding-to-resolved-layout"));
    assert!(!optimization_manifest.contains("native-realization"));
    let representation =
        rust_source(&root.join("omega-rust/omega/representations/machine-code/src"));
    assert!(representation.contains("pub struct ResolvedMachineLayout {"));
    assert!(!algorithms.contains("pub struct ResolvedMachineLayout {"));
    assert!(!optimization.contains("pub struct ResolvedMachineLayout {"));
}

#[test]
fn psi_program_roots_expose_concept_owners_without_flat_definition_dumps() {
    for (package, module, program, areas) in [
        (
            "terminal-psi",
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
            "checked-trees",
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
    let stage = root.join("omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/machine_effects");
    let source = rust_source(&stage);
    for forbidden in [
        "StagedOptimized",
        "RetainedAllocation",
        "AllocationSource",
        "_after_",
        "source_legality_stage",
        "pub struct PreAllocationMachineEffectPlan",
    ] {
        assert!(
            !source.contains(forbidden),
            "effect analysis leaked {forbidden}"
        );
    }
    // Effect replay may read the current selected facts, but must not rebuild
    // their producer or depend on the allocation history beside this module.
    for forbidden in [
        "allocation_legality_to_",
        "target_operations_to_selected_instructions",
    ] {
        assert!(
            !source.contains(forbidden),
            "effect analysis depends on a producer: {forbidden}"
        );
    }
    // Allocation owns the sealed current-selected interface as well as its
    // algorithms. Sharing that interface does not authorize stage ancestry.
    assert!(source.contains("ValidatedSelectedAnalysis"));
    let construction = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/register-homes-to-post-allocation-machine/src/construction/mod.rs",
    )).unwrap();
    assert!(construction.contains("analyze_machine_effects(selected, environment)"));
    let validation = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/register-homes-to-post-allocation-machine/src/validation.rs",
    ))
    .unwrap();
    assert!(
        validation.contains("validate_machine_effects(selected, environment, staged.effects())")
    );
}

#[test]
fn allocation_has_one_phase_owner_and_machine_consumers_ignore_history() {
    let pipeline = repository().join("omega-rust/omega/pipeline");
    for retired in [
        "omega-selected-instructions-to-machine-effects",
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
    let allocation = pipeline.join("selected-instructions-to-register-homes/src");
    for area in ["rewrites", "assignment", "output"] {
        assert!(
            allocation.join(area).join("mod.rs").is_file(),
            "missing allocation owner: {area}"
        );
    }
    let selected = pipeline.join("selected-instructions-to-selected-instructions/src");
    assert!(!allocation.join("analyses").exists());
    for area in ["analyses", "rewrites"] {
        assert!(selected.join(area).join("mod.rs").is_file());
    }
    let allocator_entry =
        std::fs::read_to_string(allocation.join("assignment/current.rs")).unwrap();
    assert!(allocator_entry.contains("SelectedInstructionOptimizationOutput"));
    assert!(!allocator_entry.contains("run_selected_lowering_optimizations("));
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
        "register-homes-to-post-allocation-machine/src",
        "selected-instructions-to-register-homes/src/preservation",
    ] {
        let source = rust_source(&pipeline.join(consumer));
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
fn fixed_frame_consumes_current_allocation_and_retains_exact_receipt_role() {
    let root =
        repository().join("omega-rust/omega/backend/machine-emission/src/function_realization");
    let route = std::fs::read_to_string(root.join("fixed_frame.rs")).unwrap();
    assert!(route.contains("allocation: RetainedAllocation"));
    let compact_route = route.split_whitespace().collect::<String>();
    assert!(compact_route.contains("staged.allocation.replay_allocation()"));
    let assembly = std::fs::read_to_string(root.join("assembly/fixed_frame.rs")).unwrap();
    assert!(route.contains("let source = current.evidence().clone()"));
    assert!(assembly.contains("AllocationEvidence::RegisterHomes(_)"));
    assert!(assembly.contains("AllocationEvidence::FixedViewCopies(receipt)"));
    assert!(assembly.contains("AllocationEvidence::ActiveResidentRematerialization(receipt)"));
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
fn realization_and_emission_replay_do_not_recover_programs_from_history() {
    let root = repository().join("omega-rust/omega/backend/machine-emission/src");
    let realization = root.join("function_realization");
    let source = rust_source(&realization);
    for forbidden in [
        "StagedOptimizedRegisterHomes",
        "selected_lowering_run()",
        "steps().last()",
        "source_legality_stage()",
        "legality_stage()",
        "selected_stage()",
        "optimized_target()",
    ] {
        assert!(
            !source.contains(forbidden),
            "realization recovers program from history: {forbidden}"
        );
    }
    let replay = std::fs::read_to_string(root.join("fragment_emission/replay.rs")).unwrap();
    assert!(replay.contains("fn allocation(&self)"));
    for forbidden in [
        "selected_lowering_run()",
        "steps().last()",
        "legality_stage()",
        "selected_stage()",
        "shared_selected_after_lowering",
    ] {
        assert!(
            !replay.contains(forbidden),
            "emission replay recovers program from history: {forbidden}"
        );
    }
    assert!(replay.contains("pub struct FunctionFragmentReplayInputs"));
    assert!(!replay.contains("pub enum FunctionFragmentReplayInputs"));
}

#[test]
fn rematerialization_uses_the_common_encoding_and_layout_stages() {
    let pipeline = repository().join("omega-rust/omega/pipeline");
    assert!(
        !pipeline
            .join("omega-active-resident-rematerialization-to-selected-form-encoding/Cargo.toml")
            .exists()
    );
    let stages = repository().join("omega-rust/omega/backend/machine-emission/src");
    for retired in [
        "layout/active_resident_resolved_selected_form_layout/mod.rs",
        "function_realization/active_resident_function_relative_realization/mod.rs",
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
fn physical_coordination_has_one_allocation_machine_and_realization_join() {
    let root = repository();
    let entrance = std::fs::read_to_string(root.join(
        "omega-rust/omega/compiler/native-realization/src/native_pipeline/physical_pipeline/mod.rs",
    ))
    .unwrap();
    for join in [
        "::stage_optimized_instruction_selection(",
        "optimize_selected_instructions(selected)",
        "stage_register_allocation(selected)",
        "::stage_optimized_post_allocation_machine_plan(",
        "stage_fixed_frame_function_relative_realization(allocation, machine, budget)",
    ] {
        assert_eq!(entrance.matches(join).count(), 1, "{join}");
    }
    let backend = rust_source(
        &root.join("omega-rust/omega/backend/machine-emission/src/function_realization"),
    );
    for forbidden in [
        "stage_register_allocation(",
        "::stage_optimized_instruction_selection(",
        "stage_optimized_post_allocation_machine_plan(",
    ] {
        assert!(
            !backend.contains(forbidden),
            "realization reenters upstream producer: {forbidden}"
        );
    }
}

#[test]
fn completed_physical_results_and_emission_do_not_fork_by_history() {
    let root = repository().join("omega-rust/omega/compiler/native-realization/src");
    let model =
        std::fs::read_to_string(root.join("native_pipeline/physical_pipeline/model.rs")).unwrap();
    assert!(model.contains("pub struct StagedOptimizedVerifiedPhysicalPipeline"));
    assert!(!model.contains("pub enum StagedOptimizedVerifiedPhysicalPipeline"));
    for ancestry in [
        "selected_stage()",
        "legality_stage()",
        "selected_lowering_run()",
    ] {
        assert!(!model.contains(ancestry));
    }
    let emission = repository().join("omega-rust/omega/backend/machine-emission/src");
    let compute = emission.join("fragment_emission/compute");
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
    for file in ["mod.rs", "manifest.rs"] {
        let source = std::fs::read_to_string(compute.join(file)).unwrap();
        assert!(!source.contains("StagedOptimizedFunctionFragmentEmissionSource::"));
        assert!(!source.contains("FunctionFragmentReplayInputs::"));
        assert!(!source.contains("selected_stage()"));
        assert!(!source.contains("steps().last()"));
    }
    assert!(
        !emission
            .join("function_realization/allocation_recovery/mod.rs")
            .exists()
    );
    let source =
        std::fs::read_to_string(emission.join("function_realization/fixed_frame.rs")).unwrap();
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
    let data = rust_source(&root.join("omega-rust/omega/representations/machine-code/src"));
    let backend = root.join("omega-rust/omega/backend/machine-emission/src");
    assert_eq!(
        data.matches("pub struct ResolvedMachineProgram {").count(),
        1
    );
    assert!(!rust_source(&backend).contains("pub struct ResolvedMachineProgram {"));
    let emission = backend.join("fragment_emission");
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
    assert!(custody.contains(
        "validate_fixed_frame_function_relative_realization(source.replay().fixed_frame())"
    ));
    assert!(custody.contains("source.validate_current()?"));
    for consumer in [
        "fragment_emission/frame_application",
        "text_placement/custody/placement",
        "fragment_emission/compute",
    ] {
        let text = rust_source(&backend.join(consumer));
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
