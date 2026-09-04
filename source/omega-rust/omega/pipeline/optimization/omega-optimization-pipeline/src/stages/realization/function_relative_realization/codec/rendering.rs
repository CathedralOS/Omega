use super::super::model::{
    FunctionRelativeFrameDisposition, FunctionRelativeOptimizationRealizationManifest,
};
use super::super::prelude::*;
use super::target::{architecture_name, object_format_name};

pub(super) fn render_manifest(
    manifest: &FunctionRelativeOptimizationRealizationManifest,
) -> String {
    let mut output = String::new();
    writeln!(output, "Omega function-relative optimization realization").unwrap();
    writeln!(
        output,
        "stage: validated function-relative selected forms and whole-function exit v1"
    )
    .unwrap();
    writeln!(
        output,
        "manifest identity: {}",
        hex(&manifest.identity.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "full named suite: {}",
        hex(&manifest.selections.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "selected-lowering suite: {}",
        hex(&manifest.selected_lowering_selections.bytes())
    )
    .unwrap();
    match manifest.selected_lowering_completion {
        Some(identity) => writeln!(
            output,
            "selected-lowering completion: {}",
            hex(&identity.bytes())
        )
        .unwrap(),
        None => writeln!(output, "selected-lowering completion: not run").unwrap(),
    }
    writeln!(
        output,
        "allocation-recovery suite: {}",
        hex(&manifest.allocation_recovery_selections.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "post-allocation-machine suite: {}",
        hex(&manifest.post_allocation_machine_selections.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "function-relative-layout suite: {}",
        hex(&manifest.function_relative_layout_selections.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "pre-physical manifest: {}",
        hex(&manifest.pre_physical_manifest.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "post-allocation manifest: {}",
        hex(&manifest.post_allocation_manifest.bytes())
    )
    .unwrap();
    writeln!(output, "selected CFG: {}", hex(&manifest.selected.bytes())).unwrap();
    writeln!(
        output,
        "pre-allocation machine effects: {}",
        hex(&manifest.pre_allocation_machine_effects.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "post-allocation machine: {}",
        hex(&manifest.post_allocation_machine.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "baseline pre-layout encoding: {}",
        hex(&manifest.baseline_pre_layout.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "pre-layout encoding: {}",
        hex(&manifest.pre_layout.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "baseline resolved layout: {}",
        hex(&manifest.baseline_resolved_layout.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "final resolved layout: {}",
        hex(&manifest.resolved_layout.bytes())
    )
    .unwrap();
    match manifest.x86_branch_relaxation {
        Some(identity) => {
            writeln!(output, "x86 branch relaxation: {}", hex(&identity.bytes())).unwrap()
        }
        None => writeln!(output, "x86 branch relaxation: not run").unwrap(),
    }
    match manifest.post_allocation_machine_optimization {
        Some(custody) => {
            writeln!(
                output,
                "post-allocation machine optimization: {}",
                custody.optimization().build_case_name()
            )
            .unwrap();
            writeln!(
                output,
                "post-allocation machine artifact: {}",
                hex(&custody.artifact_identity())
            )
            .unwrap();
            writeln!(
                output,
                "post-allocation machine actions: {}",
                custody.action_count()
            )
            .unwrap();
            writeln!(
                output,
                "post-allocation machine bytes: {} -> {}",
                custody.baseline_bytes(),
                custody.selected_bytes()
            )
            .unwrap();
        }
        None => writeln!(output, "post-allocation machine optimization: not run").unwrap(),
    }
    writeln!(
        output,
        "whole-function exit contract: {}",
        hex(&manifest.whole_function_exit_contract.bytes())
    )
    .unwrap();
    writeln!(
        output,
        "target: {}/{} pointers={}/{}",
        architecture_name(manifest.target.architecture),
        object_format_name(manifest.target.object_format),
        manifest.target.pointer_size,
        manifest.target.pointer_alignment
    )
    .unwrap();
    writeln!(
        output,
        "layout policy: {}",
        match manifest.layout_policy {
            SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 =>
                "entry-then-zero-fallthrough-then-nonzero-v1",
            SelectedFunctionLayoutPolicy::SingleEntryBlockV1 => "single-entry-block-v1",
            SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1 =>
                "structural-unit-call-then-return-single-entry-block-v1",
            SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1 =>
                "entry-then-not-less-fallthrough-then-less-v1",
            SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1 =>
                "per-function-canonical-shape-v1",
        }
    )
    .unwrap();
    writeln!(
        output,
        "scope: function-relative-fragments-with-validated-whole-function-exit-v1"
    )
    .unwrap();
    writeln!(output, "functions: {}", manifest.statistics.functions).unwrap();
    writeln!(output, "blocks: {}", manifest.statistics.blocks).unwrap();
    writeln!(output, "instructions: {}", manifest.statistics.instructions).unwrap();
    writeln!(
        output,
        "function-relative bytes: {}",
        manifest.statistics.bytes
    )
    .unwrap();
    writeln!(
        output,
        "resolved conditional branches: {}",
        manifest.statistics.resolved_conditional_branches
    )
    .unwrap();
    writeln!(
        output,
        "structural functions: {}",
        manifest.statistics.structural_unit_functions
    )
    .unwrap();
    writeln!(
        output,
        "structural blocks: {}",
        manifest.statistics.structural_unit_blocks
    )
    .unwrap();
    writeln!(
        output,
        "structural instructions: {}",
        manifest.statistics.structural_unit_instructions
    )
    .unwrap();
    writeln!(
        output,
        "structural function-relative bytes: {}",
        manifest.statistics.structural_unit_bytes
    )
    .unwrap();
    writeln!(
        output,
        "unresolved internal-Machine fixups: {}",
        manifest.statistics.unresolved_internal_machine_fixups
    )
    .unwrap();
    match manifest.frame {
        FunctionRelativeFrameDisposition::Unavailable => {
            writeln!(output, "frame: unavailable").unwrap()
        }
        FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 { layout, protocol } => {
            writeln!(output, "frame: canonical-fixed-v1").unwrap();
            writeln!(output, "frame layout: {}", hex(&layout.bytes())).unwrap();
            writeln!(output, "frame protocol: {}", hex(&protocol.bytes())).unwrap();
        }
    }
    writeln!(output, "machine emission: unavailable").unwrap();
    writeln!(output, "section placement: unavailable").unwrap();
    writeln!(output, "symbols: unavailable").unwrap();
    writeln!(output, "object relocations: unavailable").unwrap();
    writeln!(output, "executable image: unavailable").unwrap();
    writeln!(output, "installation: unavailable").unwrap();
    writeln!(output, "publication: unavailable").unwrap();
    output
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}
