//! Observe the selected sum in original storage, not the root or first element.
use super::{NativeTarget, membership, produce_source, publish};

const PROJECTED_SOURCE: &str = "data Color [copy] { case Red; case Blue; }
    data Palette { prefix: u64; colors: [Color; 3]; }
    data Frame { header: u32; palette: Palette; }
    machine Frame::is_blue(&self) -> bool {
        self.palette.colors[1] in Color::Blue
    }";

#[test]
fn nested_fixed_index_case_observation_reaches_native_execution() {
    let artifact = produce_source("Frame::is_blue", PROJECTED_SOURCE);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        r#"
        #include <stdbool.h>
        #include <stdint.h>
        struct palette { uint64_t prefix; uint32_t colors[3]; };
        struct frame { uint32_t header; struct palette palette; };
        extern bool omega_entry(const struct frame *);
        int main(void) {
            struct frame value = {19, {UINT64_MAX, {1, 0, 1}}};
            if (omega_entry(&value)) return 1;
            value.palette.colors[1] = 1;
            value.palette.colors[0] = value.palette.colors[2] = 0;
            if (!omega_entry(&value)) return 2;
            value.palette.colors[1] = 0;
            if (omega_entry(&value)) return 3;
            return value.header != 19 || value.palette.prefix != UINT64_MAX;
        }
    "#,
    );
}

#[test]
fn projected_case_native_replay_rejects_another_element_or_offset() {
    let artifact = produce_source("Frame::is_blue", PROJECTED_SOURCE);
    let selections = super::OptimizationSelections::new([]).unwrap();
    let optimized = super::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &super::AdmissionProfile::default(),
        super::compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let compiled = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized,
        abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(
            NativeTarget::macos_arm64(),
        ),
    )
    .unwrap();
    let legalized = target_operations_to_selected_instructions::legalize_target_operations(
        compiled.target_operations(),
        compiled.optimized().plan(),
        compiled.optimized(),
    )
    .unwrap();
    for mutation in 0..4 {
        let mut proposed = legalized.plan().clone();
        let observation = proposed
            .scalar_functions
            .iter_mut()
            .flat_map(|function| &mut function.blocks)
            .flat_map(|block| &mut block.instructions)
            .find_map(|instruction| {
                match &mut instruction.kind {
                legalized_operations::LegalizedScalarInstructionKind::StructuralCaseMembership {
                    path, tag_byte_offset, ..
                } => Some((path, tag_byte_offset)),
                _ => None,
            }
            })
            .expect("retained projected observation");
        let (path, tag_byte_offset) = observation;
        match mutation {
            0 => *path.last_mut().unwrap() = terminal_psi::StructuralPathSegment::FixedIndex(0),
            1 => *tag_byte_offset = 0,
            2 => path.clear(),
            _ => {
                *path.last_mut().unwrap() = terminal_psi::StructuralPathSegment::FixedIndex(0);
                *tag_byte_offset -= 4;
            }
        }
        assert!(
            target_operations_to_selected_instructions::validate_legalized_operations(
                compiled.target_operations(),
                compiled.optimized().plan(),
                compiled.optimized(),
                proposed,
            )
            .is_err(),
            "projected observation substitution {mutation}"
        );
    }
}

#[test]
fn owned_record_case_projection_uses_the_selected_value_bytes() {
    let artifact = produce_source(
        "observe",
        "data Color [copy] { case Red; case Blue; }
         data Pair [copy] { first: Color; second: Color; }
         machine observe(value: Pair) -> bool { value.second in Color::Blue }",
    );
    membership::execute(
        &artifact,
        r#"
        #include <stdbool.h>
        #include <stdint.h>
        struct pair { uint32_t first, second; };
        extern bool omega_entry(struct pair);
        int main(void) {
            struct pair left = {1, 0}, right = {0, 1};
            return omega_entry(left) || !omega_entry(right)
                || left.first != 1 || left.second != 0
                || right.first != 0 || right.second != 1;
        }
    "#,
    );
}
