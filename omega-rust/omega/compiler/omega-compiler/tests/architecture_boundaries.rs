use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn backend_crates_do_not_depend_on_frontend_crates() {
    let repo_root = repo_root();
    let backend_root = repo_root.join("omega-rust/omega/backend");
    let forbidden = [
        "omega-syntax-trees",
        "omega-tokens-to-syntax-trees",
        "omega-source-files-to-tokens",
    ];

    for cargo_toml in cargo_tomls_under(&backend_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        for crate_name in forbidden {
            assert!(
                !has_dependency(&contents, crate_name),
                "{} must not depend on early-phase crate `{crate_name}`",
                cargo_toml.display()
            );
        }
    }
}

#[test]
fn backend_crates_use_only_reviewed_physical_pipeline_dependencies() {
    let repo_root = repo_root();
    let backend_root = repo_root.join("omega-rust/omega/backend");
    // Backend and pipeline are peer physical roles. Backend publication owns
    // source-free Terminal admission and exact stage replay, not Omega frontend
    // state. Bind every dependency to its actual owner; this is not a blanket
    // allowance for backend crates to import arbitrary transforms.
    let mut expected = BTreeSet::from([
        (
            "omega-machine-emission/Cargo.toml",
            "omega-abstract-operations-to-target-operations",
            "../../pipeline/omega-abstract-operations-to-target-operations",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-psi-to-abstract-operations",
            "../../pipeline/omega-psi-to-abstract-operations",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-optimization-validation",
            "../../pipeline/omega-optimization-validation",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-target-to-register-environment",
            "../../pipeline/omega-target-to-register-environment",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-post-allocation-machine-to-frame-layout",
            "../../pipeline/omega-post-allocation-machine-to-frame-layout",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-post-allocation-machine-to-optimized-machine",
            "../../pipeline/omega-post-allocation-machine-to-optimized-machine",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-post-allocation-machine-to-selected-form-encoding",
            "../../pipeline/omega-post-allocation-machine-to-selected-form-encoding",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-register-homes-to-post-allocation-machine",
            "../../pipeline/omega-register-homes-to-post-allocation-machine",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-selected-form-encoding-to-resolved-layout",
            "../../pipeline/omega-selected-form-encoding-to-resolved-layout",
        ),
        (
            "omega-machine-emission/Cargo.toml",
            "omega-selected-instructions-to-register-homes",
            "../../pipeline/omega-selected-instructions-to-register-homes",
        ),
        (
            "object/omega-object-file/Cargo.toml",
            "omega-psi-to-abstract-operations",
            "../../../pipeline/omega-psi-to-abstract-operations",
        ),
        (
            "artifacts/omega-native-artifact/Cargo.toml",
            "omega-selected-instructions-to-register-homes",
            "../../../pipeline/omega-selected-instructions-to-register-homes",
        ),
    ]);

    for cargo_toml in cargo_tomls_under(&backend_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));
        for line in production_dependency_lines(&contents) {
            if !line.contains("/pipeline/") {
                continue;
            }
            let dependency = line.split_once('=').expect("pipeline dependency").0.trim();
            let allowance = expected
                .iter()
                .copied()
                .find(|(owner, name, _)| {
                    cargo_toml == backend_root.join(owner) && *name == dependency
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{} adds an unreviewed physical-pipeline dependency `{dependency}`",
                        cargo_toml.display(),
                    )
                });
            assert!(line.contains(&format!("\"{}\"", allowance.2)));
            assert!(expected.remove(&allowance));
        }
    }
    assert!(
        expected.is_empty(),
        "reviewed backend replay inputs changed: {expected:?}"
    );
}

#[test]
fn representation_crates_do_not_depend_on_frontend_crates() {
    let repo_root = repo_root();
    let representations_root = repo_root.join("omega-rust/omega/representations");
    let forbidden = [
        "omega-syntax-trees",
        "omega-tokens-to-syntax-trees",
        "omega-source-files-to-tokens",
    ];

    for cargo_toml in cargo_tomls_under(&representations_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));
        for crate_name in forbidden {
            assert!(
                !has_dependency(&contents, crate_name),
                "{} must not depend on early-phase crate `{crate_name}`; put transform edges under the Rust product pipeline instead",
                cargo_toml.display()
            );
        }
    }
}

#[test]
fn representation_crates_do_not_depend_on_native_bridge() {
    let repo_root = repo_root();
    let representations_root = repo_root.join("omega-rust/omega/representations");

    for cargo_toml in cargo_tomls_under(&representations_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        assert!(
            !contents.contains("omega-terminal-psi-to-native-artifact"),
            "{} must not depend on native-artifact orchestration",
            cargo_toml.display()
        );
    }
}

#[test]
fn only_exact_target_closing_pipeline_crates_depend_on_final_machinery() {
    // Most pipeline crates remain target-neutral. The repository architecture
    // deliberately places checked target-closing transformations in pipeline,
    // so their exact backend-primitive edges are an exhaustive contract rather
    // than a blanket layering violation.
    let repo_root = repo_root();
    let lowering_root = repo_root.join("omega-rust/omega/pipeline");
    let final_machinery_paths = [
        "backend/instruction_set_architectures/",
        "backend/object/",
        "backend/images/",
    ];
    let mut expected = BTreeSet::from([
        (
            "omega-terminal-psi-to-native-artifact",
            "omega-image-emission",
            "backend/images/omega-image-emission",
        ),
        (
            "omega-terminal-psi-to-native-artifact",
            "omega-isa-x86_64",
            "backend/instruction_set_architectures/omega-isa-x86_64",
        ),
        (
            "omega-terminal-psi-to-native-artifact",
            "omega-machine-emission",
            "backend/omega-machine-emission",
        ),
        (
            "omega-terminal-psi-to-native-artifact",
            "omega-object-file",
            "backend/object/omega-object-file",
        ),
    ]);
    // Extracted physical stages consume the same two ISA owners as their
    // former coordinator. Keep this closed roster, not a layer-wide escape.
    for owner in [
        "omega-post-allocation-machine-to-selected-form-encoding",
        "omega-selected-form-encoding-to-resolved-layout",
        "omega-selected-instructions-to-machine-effects",
        "omega-selected-instructions-to-register-homes",
        "omega-target-to-register-environment",
    ] {
        expected.insert((
            owner,
            "omega-isa-aarch64",
            "backend/instruction_set_architectures/omega-isa-aarch64",
        ));
        expected.insert((
            owner,
            "omega-isa-x86_64",
            "backend/instruction_set_architectures/omega-isa-x86_64",
        ));
    }

    for cargo_toml in cargo_tomls_under(&lowering_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));
        let crate_name = cargo_toml
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .expect("a pipeline manifest has a UTF-8 crate directory");
        for line in production_dependency_lines(&contents) {
            let is_final_machinery = final_machinery_paths
                .iter()
                .any(|path_fragment| line.contains(path_fragment))
                || line.starts_with("omega-machine-emission =");
            if !is_final_machinery {
                continue;
            }
            let dependency = line
                .split_once('=')
                .map(|(name, _)| name.trim())
                .expect("a dependency line contains `=`");
            let Some(allowance) = expected
                .iter()
                .copied()
                .find(|(owner, allowed, _)| *owner == crate_name && *allowed == dependency)
            else {
                panic!(
                    "{} adds unauthorized target-closing dependency `{dependency}`; only the exact reviewed pipeline/backend edges are allowed",
                    cargo_toml.display()
                );
            };
            assert!(
                line.contains(allowance.2),
                "{} dependency `{dependency}` must retain reviewed target-closing path `{}`",
                cargo_toml.display(),
                allowance.2,
            );
            assert!(
                expected.remove(&allowance),
                "{} repeats reviewed target-closing dependency `{dependency}`",
                cargo_toml.display(),
            );
        }
    }
    assert!(
        expected.is_empty(),
        "reviewed target-closing dependency allowances disappeared without updating the architecture contract: {expected:?}"
    );
}

#[test]
fn artifact_crates_do_not_depend_on_native_bridge() {
    let repo_root = repo_root();
    let tooling_root = repo_root.join("omega-rust/omega/tooling");

    for cargo_toml in cargo_tomls_under(&tooling_root) {
        let Some(crate_dir) = cargo_toml.parent() else {
            continue;
        };
        if !crate_dir
            .file_name()
            .is_some_and(|file_name| file_name.to_string_lossy().contains("artifacts"))
        {
            continue;
        }

        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        assert!(
            !contents.contains("omega-terminal-psi-to-native-artifact"),
            "{} must not depend on native-artifact orchestration",
            cargo_toml.display()
        );
    }
}

#[test]
fn canonical_terminal_native_route_uses_one_composition_edge() {
    let repo_root = repo_root();
    let route = [
        "omega-rust/omega/build/omega-build-evaluation/Cargo.toml",
        "omega-rust/omega/build/omega-provider-planning/Cargo.toml",
        "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/Cargo.toml",
        "omega-rust/omega/backend/plans/omega-program-entry-plan/Cargo.toml",
    ];
    let forbidden = ["omega-checked-trees-to-state-graph", "omega-state-graph"];

    for crate_manifest in route {
        let cargo_toml = repo_root.join(crate_manifest);
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));
        for forbidden in &forbidden {
            assert!(
                !has_dependency(&contents, forbidden),
                "{} must not depend on retired `{forbidden}` lowering in the canonical native route",
                cargo_toml.display()
            );
        }
    }
}

#[test]
fn compiler_driver_delegates_terminal_product_semantics_to_one_owner() {
    let repo_root = repo_root();
    let driver_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/driver.rs");
    let owner_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/terminal_product.rs");
    let driver = fs::read_to_string(&driver_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", driver_path.display()));
    let owner = fs::read_to_string(&owner_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", owner_path.display()));

    assert!(
        driver.contains("terminal_product::produce_retained_terminal_artifact("),
        "the compiler driver must stop Terminal production through its named product owner"
    );
    for forbidden in [
        "produce_terminal_artifact_with_callback_custody",
        "verify_module",
        "TerminalNativeRealizationProposal::new",
        "RetainedTerminalArtifact::new_with_native_realization_proposal",
        "derive_compiler_intrinsic_settlement_proposals",
    ] {
        assert!(
            !driver.contains(forbidden),
            "the compiler driver must not recover Terminal-product algorithm `{forbidden}`"
        );
    }

    let mut ordered_owner = owner.as_str();
    for stage in [
        "produce_terminal_artifact_with_callback_custody_and_optimizations(",
        "verify_terminal_artifact(",
        "project_terminal_native_realization_proposal(",
        "RetainedTerminalArtifact::new_with_native_realization_proposal(",
    ] {
        let offset = ordered_owner.find(stage).unwrap_or_else(|| {
            panic!("the Terminal-product owner must contain ordered stage `{stage}`")
        });
        ordered_owner = &ordered_owner[offset + stage.len()..];
    }
}

#[test]
fn compiler_driver_has_one_admission_frontend_and_exhaustive_product_stop() {
    let repo_root = repo_root();
    let driver_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/driver.rs");
    let request_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/request.rs");
    let optimization_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/mod.rs");
    let native_report_path = repo_root.join(
        "omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/native_report/mod.rs",
    );
    let driver = fs::read_to_string(&driver_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", driver_path.display()));
    let request = fs::read_to_string(&request_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", request_path.display()));
    let mut optimization = fs::read_to_string(&optimization_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", optimization_path.display()));
    optimization.push_str(
        &fs::read_to_string(&native_report_path).unwrap_or_else(|error| {
            panic!("failed to read {}: {error}", native_report_path.display())
        }),
    );
    let compact_driver = without_ascii_whitespace(&driver);
    let compact_optimization = without_ascii_whitespace(&optimization);

    assert_eq!(
        compact_driver
            .matches("request.validate_for_execution()?")
            .count(),
        1,
        "the compiler driver must admit every requested product through one request-owner entrance"
    );
    assert_eq!(
        compact_driver
            .matches("compile_checked_with_observations(&request)?")
            .count(),
        0,
        "the common checked-Psi frontend now receives the optional prepared source checkpoint explicitly"
    );
    assert_eq!(
        compact_driver
            .matches("compile_checked_with_observations(&request,prepared)?")
            .count(),
        1,
        "every single or batched child product must share one checked-Psi frontend continuation"
    );
    assert!(
        compact_driver.contains(
            "letrequest=request.validate_for_execution()?;compile_validated(request,None)"
        ),
        "the single-request entrance must admit once before the common product continuation"
    );
    let mut ordered_driver = compact_driver.as_str();
    for stage in [
        "fncompile_validated(",
        "compile_checked_with_observations(&request,prepared)?;",
        "matchrequest.requested_product(){",
    ] {
        let offset = ordered_driver.find(stage).unwrap_or_else(|| {
            panic!("the compiler driver must contain ordered common stage `{stage}`")
        });
        ordered_driver = &ordered_driver[offset + stage.len()..];
    }
    for product in ["Check", "TerminalArtifact", "NativeArtifact"] {
        assert!(
            compact_driver.contains(&format!("RequestedCompileProduct::{product}=>")),
            "the common product stop must exhaustively name `{product}`"
        );
    }
    assert!(
        !driver.contains("compile_native_with_checked_receipt"),
        "native production must remain an arm of the common product stop, not an alternate entrance"
    );
    let native_arm = compact_driver
        .split_once("RequestedCompileProduct::NativeArtifact=>")
        .map(|(_, native_arm)| native_arm)
        .expect("the common product stop must contain its native arm");
    native_arm
        .find("optimization::native_report(request,checked).map(finalize_report)")
        .expect("the native product arm must invoke its report owner");
    let checked_receipt = compact_optimization
        .find("NativeCompilationWithCheckedReceipt::new(checked,report)")
        .expect("the native report owner must retain checked/report custody validation");
    let report_assembly = compact_optimization
        .find("letreport=CompileReport::from_retained_native_artifact(")
        .expect("the native report owner must assemble the report");
    assert!(
        report_assembly < checked_receipt,
        "native report assembly must precede checked/report custody validation"
    );
    assert!(
        compact_driver.contains("optimization::prepare_native_report(request,checked)?"),
        "the native batch route must delegate each child's Terminal preparation to the same owner"
    );
    assert!(
        !request.contains("validate_for_native_execution"),
        "the request owner must expose one production admission operation"
    );
}

#[test]
fn compiler_surface_and_reporting_close_driver_cleanup_contract() {
    let repo_root = repo_root();
    let compiler_path = repo_root.join("omega-rust/omega/compiler/omega-compiler/src/compiler.rs");
    let driver_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/driver.rs");
    let reporting_path = repo_root.join(
        "omega-rust/omega/compiler/omega-compiler/src/pipeline/reporting/checked_observations.rs",
    );
    let compiler = fs::read_to_string(&compiler_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", compiler_path.display()));
    let driver = fs::read_to_string(&driver_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", driver_path.display()));
    let reporting = fs::read_to_string(&reporting_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", reporting_path.display()));
    let compact_compiler = without_ascii_whitespace(&compiler);
    let compact_driver = without_ascii_whitespace(&driver);

    assert_eq!(
        compact_compiler.matches("pubfncompile(").count(),
        2,
        "the compiler surface must remain one typed operation exposed as the Compiler method and its free-function facade"
    );
    assert_eq!(
        compact_compiler.matches("request:CompileRequest").count(),
        2,
        "both production facades must accept the same complete CompileRequest"
    );
    for retired in [
        "compile_with_",
        "CompileHarnessRequest",
        "write_output",
        "entry_override",
        "worker_ceiling",
    ] {
        assert!(
            !compiler.contains(retired) && !driver.contains(retired),
            "retired compiler mode/control `{retired}` must not return to the production surface"
        );
    }

    for reporting_policy in [
        "emits_auxiliary_artifacts(",
        "ArtifactWriter",
        "write_checked_snapshots(",
        "00_timings.html",
    ] {
        assert!(
            !driver.contains(reporting_policy),
            "the compiler driver must not branch or write reporting concern `{reporting_policy}`"
        );
    }
    assert!(
        reporting.contains("if input.artifact_policy.emits_auxiliary_artifacts()"),
        "the checked-observation owner must retain the sole auxiliary-report policy branch"
    );
    assert_eq!(
        compact_driver
            .matches("report_checked_observations(")
            .count(),
        1,
        "the driver must submit the complete checked result to one reporting operation"
    );
}

#[test]
fn typed_to_checked_surface_owns_contract_stand_down_capture() {
    let repo_root = repo_root();
    let transition_path = repo_root
        .join("omega-rust/omega/compiler/omega-compiler/src/pipeline/phase_transitions.rs");
    let transition = fs::read_to_string(&transition_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", transition_path.display()));
    assert!(
        transition.contains("collect_contract_entailment_stand_downs(&typed)"),
        "typed-derived contract stand-downs must be captured at the ownership-moving phase boundary"
    );

    let driver_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/checked_entry.rs");
    let driver = fs::read_to_string(&driver_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", driver_path.display()));
    assert!(
        !driver.contains("collect_contract_entailment_stand_downs(&typed)"),
        "checked orchestration must consume the phase-owned ledger instead of couriering a raw typed-derived vector"
    );

    let certificate_path = repo_root.join(
        "omega-rust/psi/pipeline/psi-typed-trees-to-checked-trees/src/proof/contract_entailment.rs",
    );
    let certificate = fs::read_to_string(&certificate_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", certificate_path.display()));
    assert!(
        certificate.contains("collect_contract_entailment_stand_downs(program)"),
        "checked-IR contract-entailment certificates must reconstruct the stand-down roster inside typed-to-checked lowering"
    );
    assert!(
        certificate.contains("psi_proof_admission::accept_certificate("),
        "typed-to-checked certificate construction must invoke the proof kernel"
    );
    assert!(
        !transition.contains("CheckedContractEntailmentAssumptionDischarge"),
        "compiler orchestration must not construct checked-IR proof certificates"
    );
}

#[test]
fn checked_build_orchestration_consumes_an_admitted_checkpoint() {
    let repo_root = repo_root();
    let checked_entry_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/checked_entry.rs");
    let checked_entry = fs::read_to_string(&checked_entry_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", checked_entry_path.display()));
    let checked_entry = without_ascii_whitespace(&checked_entry);

    assert!(
        checked_entry.contains("structAdmittedBuildCheckpoint"),
        "checked orchestration must retain the admitted build inputs in one activation-local checkpoint"
    );
    assert!(
        checked_entry.contains("admit_build_program("),
        "checked orchestration must admit the build program before executing it"
    );
    assert!(
        checked_entry.contains("retain_generated_syntax_extension("),
        "checked orchestration must retain generated syntax as an explicit continuation input"
    );
    assert!(
        checked_entry.contains("pre_check.evaluate_extension(&muttyped"),
        "each generated unit must consume its matching post-typing continuation"
    );
    assert!(
        checked_entry.contains("retained_typed_base_is_exact_prefix("),
        "checked orchestration must validate the retained semantic prefix after extension pre-check"
    );
    assert!(
        !checked_entry.contains("compute_build_config("),
        "checked orchestration must consume the admitted carrier instead of using the compatibility wrapper"
    );
    assert_eq!(
        checked_entry.matches("lower_checked_frontend(").count(),
        2,
        "checked orchestration must contain one frontend definition and exactly one call site"
    );
    for retired_reconstruction in [
        "use_rebuild",
        "append_to(",
        "prepass_build_identity",
        "selected_build_identity",
        "rebindthebuildmachine",
    ] {
        assert!(
            !checked_entry.contains(retired_reconstruction),
            "D18 must not retain whole-frontend reconstruction machinery `{retired_reconstruction}`"
        );
    }

    let source_assembly_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/source_assembly.rs");
    let source_assembly = fs::read_to_string(&source_assembly_path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", source_assembly_path.display())
    });
    for retired_carrier in ["seeded_syntax_trees", "pub(super)files:", "fnappend_to("] {
        assert!(
            !without_ascii_whitespace(&source_assembly).contains(retired_carrier),
            "generated-source custody must not retain reconstruction-only carrier `{retired_carrier}`"
        );
    }
}

#[test]
fn typed_to_checked_transition_owns_post_check_settlements_inside_its_surface() {
    let repo_root = repo_root();
    let transition_path = repo_root
        .join("omega-rust/omega/compiler/omega-compiler/src/pipeline/phase_transitions.rs");
    let transition = fs::read_to_string(&transition_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", transition_path.display()));
    let transition = without_ascii_whitespace(&transition);
    let settlement = transition
        .find("close_outbound_callback_materializations(")
        .expect("the typed-to-checked phase transition must own explicit callback settlement");
    let shared_ownership = transition
        .find("Arc::new(program)")
        .expect("the typed-to-checked phase transition must publish checked Psi through Arc");
    assert!(
        settlement < shared_ownership,
        "callback settlement must complete before checked Psi enters shared Arc ownership"
    );
    assert!(
        transition.contains("bind_selected_provider_plan_facts("),
        "the typed-to-checked phase transition must own provider fact settlement"
    );
    assert!(
        transition.contains(
            "pub(super)selected_provider_plan_facts:omega_effects::SelectedProviderPlanFacts,"
        ),
        "the final checked phase surface must require its settled provider facts"
    );
    assert!(
        transition.contains("fntyped_trees_to_preliminary_checked_trees("),
        "preliminary package validation must use a distinct checked observation rather than an incomplete final surface"
    );
    let returned_surface = transition
        .split_once("Ok(CheckedProgramSurface{")
        .map(|(_, returned_surface)| returned_surface)
        .expect("the typed-to-checked transition must return its checked phase surface");
    assert!(
        returned_surface.contains("selected_provider_plan_facts"),
        "the typed-to-checked transition must return the provider facts settled beside the program"
    );

    let selected_execution_dispatches = [
        "omega_selected_dispatch::settle_selected_execution_dispatch_with_source_edits(",
        "omega_selected_dispatch::retain_selected_compiler_intrinsic_review_identities(",
        "omega_selected_dispatch::settle_selected_boundary_adapter_dispatch_with_source_edits(",
    ];
    let mut ordered_transition_suffix = transition.as_str();
    for settlement_step in [
        "build_selected_component_progress_manifest(",
        selected_execution_dispatches[0],
        selected_execution_dispatches[1],
        selected_execution_dispatches[2],
        "elaborate_task_activation_plans(",
    ] {
        let offset = ordered_transition_suffix
            .find(settlement_step)
            .unwrap_or_else(|| {
                panic!(
                    "the selected-execution phase transition must own ordered step `{settlement_step}`"
                )
            });
        ordered_transition_suffix = &ordered_transition_suffix[offset + settlement_step.len()..];
    }
    for settled_output in ["component_progress", "task_activations"] {
        assert!(
            transition.contains(&format!("pub(super){settled_output}:")),
            "the final selected-execution settlement surface must carry `{settled_output}`"
        );
        let returned_settlement = transition
            .split_once("Ok(SelectedExecutionSettlementSurface{")
            .map(|(_, returned_settlement)| returned_settlement)
            .expect("selected execution must return its named settlement surface");
        assert!(
            returned_settlement.contains(settled_output),
            "selected execution must return settled output `{settled_output}`"
        );
    }
    assert!(
        transition.contains("structSelectedExecutionSettlement"),
        "phase transitions must publish selected execution through a named settlement surface"
    );

    for driver_relative_path in [
        "omega-rust/omega/compiler/omega-compiler/src/compiler/driver.rs",
        "omega-rust/omega/compiler/omega-compiler/src/pipeline/checked_entry.rs",
    ] {
        let driver_path = repo_root.join(driver_relative_path);
        let driver = fs::read_to_string(&driver_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", driver_path.display()));
        assert!(
            !without_ascii_whitespace(&driver).contains("Arc::get_mut("),
            "{} must not recover unique ownership to rewrite checked Psi after checking",
            driver_path.display()
        );
    }

    let checked_entry_path =
        repo_root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/checked_entry.rs");
    let checked_entry = fs::read_to_string(&checked_entry_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", checked_entry_path.display()));
    let checked_entry = without_ascii_whitespace(&checked_entry);
    assert!(
        !checked_entry.contains("bind_selected_provider_plan_facts("),
        "checked orchestration must consume phase-settled provider facts instead of binding them after checking"
    );
    assert!(
        !checked_entry.contains("checked.program="),
        "checked orchestration must not replace the checked program after its phase transition"
    );
    for dispatch in selected_execution_dispatches {
        assert!(
            !checked_entry.contains(dispatch),
            "checked orchestration must consume selected-execution settlement instead of directly calling `{dispatch}`"
        );
    }
    for phase_owned_step in [
        "build_selected_component_progress_manifest(",
        "elaborate_task_activation_plans(",
    ] {
        assert!(
            !checked_entry.contains(phase_owned_step),
            "checked orchestration must not directly perform phase-owned settlement step `{phase_owned_step}`"
        );
    }
    assert!(
        checked_entry.contains("selected_execution_settlement"),
        "checked orchestration must consume a named selected-execution settlement surface"
    );
    assert!(
        checked_entry.contains("ExactComponentProgressRoot::new("),
        "checked orchestration may derive only the exact component-progress root passed into selected-execution settlement"
    );
}

#[test]
fn maintained_omega_sources_do_not_declare_target_support() {
    let root = repo_root();
    let mut sources = Vec::new();
    for owned_root in ["build.omg", "samples", "source", "tests"] {
        collect_omega_sources(&root.join(owned_root), &mut sources);
    }
    sources.sort();

    let mut target_declarations = Vec::new();
    for source_path in sources {
        let source = fs::read_to_string(&source_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
        for (line_index, line) in source.lines().enumerate() {
            let code = line.split_once("//").map_or(line, |(code, _)| code);
            let mut words = code.split_whitespace();
            if words.next() == Some("target") && words.next().is_some() {
                target_declarations.push(format!("{}:{}", source_path.display(), line_index + 1));
            }
        }
    }

    assert!(
        target_declarations.is_empty(),
        "exact target identity and policy are immutable invocation/package inputs; remove authored target declarations:\n{}",
        target_declarations.join("\n"),
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/omega-compiler")
        .to_path_buf()
}

fn collect_omega_sources(path: &Path, sources: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().and_then(|extension| extension.to_str()) == Some("omg") {
            sources.push(path.to_path_buf());
        }
        return;
    }
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to read directory entry: {error}"))
            .path();
        if path.is_dir() || path.extension().and_then(|extension| extension.to_str()) == Some("omg")
        {
            collect_omega_sources(&path, sources);
        }
    }
}

fn cargo_tomls_under(root: &Path) -> Vec<PathBuf> {
    let mut cargo_tomls = Vec::new();
    collect_cargo_tomls(root, &mut cargo_tomls);
    cargo_tomls.sort();
    cargo_tomls
}

fn collect_cargo_tomls(path: &Path, cargo_tomls: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
        let path = entry.path();

        if path.is_dir() {
            collect_cargo_tomls(&path, cargo_tomls);
        } else if path
            .file_name()
            .is_some_and(|file_name| file_name == "Cargo.toml")
        {
            cargo_tomls.push(path);
        }
    }
}

/// Lines of the production `[dependencies]` section only. Layering rules
/// govern the shipped dependency structure; `[dev-dependencies]` used by unit
/// tests (which commonly drive the front of the pipeline to build real
/// programs in memory) do not create production edges.
fn production_dependency_lines(contents: &str) -> Vec<&str> {
    let mut in_dependencies = false;
    let mut lines = Vec::new();
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
            continue;
        }
        if in_dependencies {
            lines.push(line);
        }
    }
    lines
}

fn has_dependency(contents: &str, crate_name: &str) -> bool {
    let dependency_prefix = format!("{crate_name} =");
    production_dependency_lines(contents)
        .iter()
        .any(|line| line.starts_with(&dependency_prefix))
}

fn without_ascii_whitespace(contents: &str) -> String {
    contents
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect()
}
