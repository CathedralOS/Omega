//! Source-produced projected borrows preserve their contracts through Omega admission.
//! Native stores remain an explicit downstream dependency, not execution coverage.

use target::NativeTarget;

fn artifact(source: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    terminal_production::produce_terminal_artifact(&checked, "forward").unwrap()
}

fn optimize(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan {
    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    let optimized = native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("source-produced receiver contract survives independent Omega admission");
    assert_eq!(optimized.plan(), optimized.verified_input().plan());
    assert_eq!(
        optimized.validation().initial_unit(),
        optimized.validation().final_unit()
    );
    assert!(optimized.commits().is_empty());
    optimized
}

const INDEXED_ALIAS: &str = "data Record [copy] { value: u16; }
    machine Record::replace(&write self) { self.value = 17; }
    machine forward(records: &write [Record; 2]) {
        let held: &write [Record; 2] = &write records;
        held[1].replace();
    }";

#[test]
fn source_projected_write_borrows_preserve_optimizer_contracts() {
    for (borrow, referent, receiver) in [
        ("write", "[Record; 2]", "root[1]"),
        ("mut", "[Record; 2]", "root[1]"),
        ("write", "[[[Record; 2]; 2]; 2]", "root[1][0][1]"),
        ("mut", "[[[Record; 2]; 2]; 2]", "root[1][0][1]"),
        ("write", "Container", "root.records[1]"),
        ("mut", "Container", "root.records[1]"),
        ("write", "[Container; 2]", "root[1].records[0]"),
        ("mut", "[Container; 2]", "root[1].records[0]"),
        ("mut", "Container", "root.record"),
    ] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
             data Container [copy] {{ record: Record; records: [Record; 2]; }}
             machine Record::replace(&write self) {{ self.value = 17; }}
             machine forward(root: &{borrow} {referent}) {{ {receiver}.replace(); }}"
        );
        let _optimized = optimize(&artifact(&source));
    }
    let _optimized = optimize(&artifact(INDEXED_ALIAS));
}

#[test]
fn indexed_write_only_alias_reaches_the_native_store_dependency() {
    let artifact = artifact(INDEXED_ALIAS);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let (store_machine, store_operation) = module
        .machines
        .iter()
        .find_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::StructuralScalarFieldStore { .. }
                    )
                    .then_some((machine.id, operation.id))
                })
        })
        .expect("source retains its receiver store");
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let result =
            native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimize(&artifact),
                target,
                &[],
            );
        assert!(
            matches!(result, Err(
            native_realization::OptimizedVerifiedPhysicalPipelineError::Selection(
                target_operations_to_selected_instructions::OptimizedSelectionPipelineError::Legalization(
                    target_operations_to_selected_instructions::LegalizationError::AttachedUnitStructuralScalarNotYetSelectable { machine, operation }
                )
            )
        ) if machine == store_machine && operation == store_operation),
            "{target:?}: {result:?}"
        );
    }
    // Replace this exact admission oracle with caller-owned byte observations
    // when common-graph stores and projected borrowed-pointer calls are realized.
    // Reaching a known rejection does not establish native execution.
}
