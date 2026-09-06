//! Rehashed fragment changes must fail projection checks without producer history.

use crate::tests::*;
use machine_code::{FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan};
use machine_emission::{
    ResolvedFragmentEmissionError, emit_resolved_function_fragments,
    validate_resolved_function_fragments,
};

type Mutation = fn(&mut FunctionFragmentEmissionPlan);

#[test]
fn raw_fragment_replay_rejects_reauthenticated_bytes_rosters_and_provenance() {
    let mutations: [(&str, Mutation); 14] = [
        ("missing function", |value| {
            value.functions.pop();
        }),
        ("duplicate function", |value| {
            value.functions.push(value.functions[0].clone())
        }),
        ("function bytes", |value| value.functions[0].bytes[0] ^= 1),
        ("function extent", |value| {
            value.functions[0].byte_count += 1
        }),
        ("block order", |value| value.functions[0].blocks.reverse()),
        ("block extent", |value| {
            value.functions[0].blocks[0].byte_count += 1
        }),
        ("block offset", |value| {
            value.functions[0].blocks[0].offset += 1
        }),
        ("missing span", |value| {
            value.functions[0].blocks[0].instructions.pop();
        }),
        ("span offset", |value| {
            value.functions[0].blocks[0].instructions[0].offset += 1
        }),
        ("span bytes", |value| {
            value.functions[0].blocks[0]
                .instructions
                .iter_mut()
                .find(|row| !row.bytes.is_empty())
                .unwrap()
                .bytes[0] ^= 1
        }),
        ("alternative", |value| {
            value.functions[0].blocks[0].instructions[0]
                .alternative
                .variant += 1
        }),
        ("branch displacement", |value| {
            let machine_code::FunctionFragmentBranchEvidence::Conditional(branch) = value.functions
                [0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find_map(|row| row.branch.as_deref_mut())
            .unwrap() else {
                panic!("conditional fixture must retain conditional evidence");
            };
            branch.byte_displacement += 1
        }),
        ("branch successor", |value| {
            let machine_code::FunctionFragmentBranchEvidence::Conditional(branch) = value.functions
                [0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find_map(|row| row.branch.as_deref_mut())
            .unwrap() else {
                panic!("conditional fixture must retain conditional evidence");
            };
            branch.when_taken_offset += 1
        }),
        ("control provenance", |value| {
            let row = value.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|row| {
                    matches!(
                        row.control,
                        FunctionFragmentControlProvenance::ConditionalBranch { .. }
                    )
                })
                .unwrap();
            row.control = FunctionFragmentControlProvenance::None;
        }),
    ];
    for (target, selections) in [
        (NativeTarget::linux_x64(), OptimizationSelections::default()),
        (
            NativeTarget::linux_x64(),
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap(),
        ),
        (
            NativeTarget::linux_arm64(),
            OptimizationSelections::default(),
        ),
    ] {
        let source = super::current_program::source(target, selections);
        let program = source.program().clone();
        let staged = stage_optimized_function_fragment_emission(source).unwrap();
        let original = staged.fragments().clone();
        assert!(original.functions[0].blocks.len() > 1);
        drop(staged);
        // A raw projection remains possible without retaining the producer or
        // its authority. It is not executable publication admission.
        assert_eq!(
            emit_resolved_function_fragments(&program).unwrap(),
            original
        );
        for (name, mutate) in mutations {
            let mut changed = original.clone();
            mutate(&mut changed);
            changed.identity = changed.recomputed_identity();
            assert_ne!(changed.identity, original.identity, "{name}");
            assert_eq!(
                validate_resolved_function_fragments(&program, &changed),
                Err(ResolvedFragmentEmissionError::ArtifactMismatch),
                "{target:?}: {name}"
            );
        }
        validate_resolved_function_fragments(&program, &original).unwrap();
    }
}
