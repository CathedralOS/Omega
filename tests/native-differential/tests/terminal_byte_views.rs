//! Native byte observations start from encoded, independently verified Terminal.
//! The complete source writer closure remains outside these fixtures.

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout, stage_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout,
};
use semantic_vocabulary::{
    IntegerValue, ObligationId, OperationId, PlaceId, StructuralPlaceKind, ValueId,
};
use target::NativeTarget;
use terminal_psi::{OperationKind, StructuralPlaceDeclaration, TerminalModule};
use terminal_verifier::ProofBundle;

#[path = "terminal_byte_views/fixtures.rs"]
mod fixtures;
use fixtures::{byte_view_length_module, byte_view_read_module, byte_view_read_proof};
#[path = "terminal_byte_views/calls.rs"]
mod calls;
#[path = "terminal_byte_views/calls_admission.rs"]
mod calls_admission;
#[path = "terminal_byte_views/helper_admission.rs"]
mod helper_admission;
#[path = "terminal_byte_views/mixed_helpers.rs"]
mod mixed_helpers;
#[path = "terminal_byte_views/subslice.rs"]
mod subslice;
#[path = "terminal_byte_views/subslice_admission.rs"]
mod subslice_admission;
#[path = "terminal_byte_views/unit_calls.rs"]
mod unit_calls;

#[test]
fn byte_view_length_helper_cross_lowers_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for depth in [1, 3] {
            let layout = stage_byte_view(
                &fixtures::byte_view_length_helper_chain(depth),
                &ProofBundle::default(),
                target,
            );
            assert_eq!(layout.functions().len(), depth as usize + 1);
        }
    }
}

#[test]
fn byte_view_length_helper_executes_with_original_descriptor() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        for depth in [1, 3] {
            let module = fixtures::byte_view_length_helper_chain(depth);
            let semantic = terminal_codec::encode_module(&module).unwrap();
            let proof = terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap();
            let selections = OptimizationSelections::new([]).unwrap();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                compiler_baseline_request_v1(&selections),
            )
            .unwrap();
            let physical = native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized, NativeTarget::host(), &[],
        ).expect("borrowed helper reaches the complete physical pipeline");
            let fragments = machine_emission::stage_optimized_function_fragment_emission(
                physical.into_function_fragment_emission_source(),
            )
            .unwrap();
            let framed =
                machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
            let placed =
                machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
            let text = placed.text_section();
            assert_eq!(text.resolved_internal_machine_calls.len(), depth as usize);
            let entry = text
                .functions
                .iter()
                .find(|function| function.machine == module.entry)
                .unwrap();
            native_function::assert_c_text(
                &text.bytes,
                entry.section_offset.try_into().unwrap(),
                r#"
            #include <stdint.h>
            #include <stddef.h>
            #include <unistd.h>
            struct ByteView { const uint8_t *bytes; uint64_t length; };
            extern uint64_t omega_entry(const struct ByteView *);
            int main(void) {
                alarm(10);
                const uint8_t raw[] = { 0xff, 0, 0x80, 0x41 };
                struct ByteView view = { NULL, 0 };
                if (omega_entry(&view) != 0) return 1;
                view.bytes = raw; view.length = sizeof(raw);
                if (omega_entry(&view) != sizeof(raw)) return 2;
                if (view.bytes != raw || view.length != sizeof(raw)) return 3;
                view.length = 1;
                if (omega_entry(&view) != 1) return 4;
                view.bytes = NULL; view.length = 0;
                if (omega_entry(&view) != 0) return 5;
                return 0;
            }
        "#,
            );
        }
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!("SKIP borrowed helper execution: Linux/macOS cc host harness unavailable");
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
#[path = "common/native_function.rs"]
mod native_function;

fn stage_byte_view_length(target: NativeTarget) -> StagedOptimizedResolvedSelectedFormLayout {
    stage_byte_view(&byte_view_length_module(), &ProofBundle::default(), target)
}

fn stage_byte_view(
    module: &TerminalModule,
    proof: &ProofBundle,
    target: NativeTarget,
) -> StagedOptimizedResolvedSelectedFormLayout {
    let target = byte_view_target(module, proof, target);
    select_byte_view(target)
}

fn byte_view_target(
    module: &TerminalModule,
    proof: &ProofBundle,
    target: NativeTarget,
) -> abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations {
    let semantic = terminal_codec::encode_module(module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(proof).unwrap();
    let selections = OptimizationSelections::new([]).unwrap();
    // This public boundary decodes and verifies the artifact before projection.
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .expect("verified byte observation reaches the ordinary optimizer input");
    abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized, target,
    )
    .expect("byte observation reaches target operations")
}

fn select_byte_view(
    target: abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
) -> StagedOptimizedResolvedSelectedFormLayout {
    let environment =
        register_environment::baseline_target_register_environment(target.target()).unwrap();
    let selected =
        target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            target,
            environment,
        )
        .expect("byte observation selects native instructions");
    let liveness =
        selected_instructions_to_register_homes::stage_optimized_liveness(selected).unwrap();
    let ranges =
        selected_instructions_to_register_homes::stage_optimized_live_ranges(liveness).unwrap();
    let legality =
        selected_instructions_to_register_homes::stage_optimized_allocation_legality(ranges)
            .unwrap();
    let homes =
        selected_instructions_to_register_homes::stage_optimized_register_homes(legality).unwrap();
    let machine =
        register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan(
            &homes,
        )
        .unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let encoding = post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(), &machine, physical, None,
    ).unwrap();
    let layout = stage_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        &machine,
        physical,
        &encoding,
    )
    .unwrap();
    validate_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        &machine,
        physical,
        &encoding,
        &layout,
    )
    .expect("byte observation's retained machine bytes independently replay");
    layout
}

#[test]
fn byte_view_read_cross_lowers_on_hosted_targets() {
    let module = byte_view_read_module();
    let proof = byte_view_read_proof(&module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let layout = stage_byte_view(&module, &proof, target);
        assert_eq!(layout.functions().len(), 1);
        assert!(layout.functions()[0].byte_count > 0);
    }
}

#[test]
fn byte_view_read_executes_raw_bytes_and_skips_out_of_bounds_access() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        let module = byte_view_read_module();
        let proof = byte_view_read_proof(&module);
        native_function::assert_c_driver(
            &stage_byte_view(&module, &proof, NativeTarget::host()),
            r#"
            #include <stdint.h>
            #include <stddef.h>
            struct ByteView { const uint8_t *bytes; uint64_t length; };
            _Static_assert(sizeof(struct ByteView) == 16, "descriptor size");
            _Static_assert(offsetof(struct ByteView, length) == 8, "length offset");
            extern uint64_t omega_entry(uint64_t, const struct ByteView *);
            int main(void) {
                const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41 };
                const uint8_t other[] = { 0x17, 0xfe };
                struct ByteView view = { NULL, 0 };
                if (omega_entry(0, &view) != 256) return 1;
                if (omega_entry(UINT64_MAX, &view) != 256) return 2;
                view.bytes = raw; view.length = sizeof(raw);
                for (uint64_t position = 0; position < sizeof(raw); ++position)
                    if (omega_entry(position, &view) != raw[position]) return 3;
                if (omega_entry(sizeof(raw), &view) != 256) return 4;
                if (omega_entry(UINT64_MAX, &view) != 256) return 5;
                view.bytes = other; view.length = sizeof(other);
                if (omega_entry(0, &view) != 0x17) return 6;
                if (omega_entry(1, &view) != 0xfe) return 7;
                view.length = 1;
                if (omega_entry(1, &view) != 256) return 8;
                view.bytes = NULL; view.length = 0;
                if (omega_entry(0, &view) != 256) return 9;
                return 0;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP guarded byte-read native execution: existing cc harness supports Linux x86-64/AArch64 and macOS AArch64; Windows runtime route unavailable"
    );
}

#[test]
fn byte_view_length_cross_lowers_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let layout = stage_byte_view_length(target);
        assert_eq!(layout.functions().len(), 1);
        assert!(layout.functions()[0].byte_count > 0);
    }
}

#[test]
fn byte_view_length_executes_with_dynamic_native_descriptors() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    native_function::assert_c_driver(
        &stage_byte_view_length(NativeTarget::host()),
        r#"
        #include <stdint.h>
        #include <stddef.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        _Static_assert(sizeof(void *) == 8, "parameter is one eight-byte pointer");
        _Static_assert(sizeof(struct ByteView) == 16, "referent is sixteen bytes");
        _Static_assert(_Alignof(struct ByteView) == 8, "referent alignment is eight");
        _Static_assert(offsetof(struct ByteView, length) == 8, "length lives at offset eight");
        extern uint64_t omega_entry(const struct ByteView *);
        int main(void) {
            const uint8_t text[] = { 'O', 'm', 'e', 'g', 'a' };
            const uint8_t raw[] = { 0xff, 0x00, 0x80, 0xfe };
            uint8_t longer[257] = { 0xff };
            struct ByteView view = { NULL, 0 };
            if (omega_entry(&view) != 0) return 1;
            view.bytes = text; view.length = sizeof(text);
            if (omega_entry(&view) != 5) return 2;
            view.bytes = raw; view.length = sizeof(raw);
            if (omega_entry(&view) != 4) return 3;
            view.bytes = longer; view.length = sizeof(longer);
            if (omega_entry(&view) != 257) return 4;
            view.length = 0;
            if (omega_entry(&view) != 0) return 5;
            return 0;
        }
    "#,
    );
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP byte-view native execution: the existing cc/assembly harness supports Linux x86-64/AArch64 and macOS AArch64; Windows requires a separate host execution route"
    );
}

#[test]
fn byte_view_length_rejects_an_unavailable_descriptor_source() {
    let mut module = byte_view_length_module();
    module.machines[0].blocks[0].operations[0].kind = OperationKind::ByteSequenceLength {
        source: PlaceId::new(99).unwrap(),
    };
    assert_eq!(
        terminal_codec::encode_module(&module),
        Err(terminal_codec::CodecError::InvalidModule(
            terminal_verifier::ModuleError::InvalidByteSequenceLengthSource {
                operation: OperationId::new(7).unwrap(),
                source: PlaceId::new(99).unwrap(),
            },
        )),
        "a missing descriptor cannot enter a canonical artifact",
    );
}

fn assert_byte_read_proof_rejected(module: &TerminalModule, proof: &ProofBundle) {
    let semantic =
        terminal_codec::encode_module(module).expect("negative fixture remains structurally valid");
    let proof = terminal_codec::encode_proof_bundle(proof).unwrap();
    match terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
    ) {
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::Verification(_)) => {}
        Err(error) => panic!("byte-read proof must reject before native lowering: {error}"),
        Ok(_) => panic!("unproved byte-read bounds entered native lowering"),
    }
}

#[test]
fn byte_view_read_rejects_weakened_guards_and_retargeted_evidence() {
    let module = byte_view_read_module();
    let proof = byte_view_read_proof(&module);
    assert_byte_read_proof_rejected(&module, &ProofBundle::default());
    let mut retargeted = proof.clone();
    retargeted.evidence[0].obligation = ObligationId::new(2).unwrap();
    assert_byte_read_proof_rejected(&module, &retargeted);

    let mut weaker = module.clone();
    weaker.machines[0].blocks[0].operations[1].kind = OperationKind::IntegerLessOrEqual {
        left: ValueId::new(10).unwrap(),
        right: ValueId::new(5).unwrap(),
    };
    assert_byte_read_proof_rejected(&weaker, &proof);

    let mut wrong_arm = module;
    let terminal_psi::Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut wrong_arm.machines[0].blocks[0].terminator
    else {
        panic!("guarded fixture")
    };
    std::mem::swap(&mut when_true.target, &mut when_false.target);
    assert_byte_read_proof_rejected(&wrong_arm, &proof);
}

#[test]
fn byte_view_read_rejects_a_length_not_proven_by_its_guard() {
    let mut module = byte_view_read_module();
    let proof = byte_view_read_proof(&module);
    let mut fresh_length = module.machines[0].blocks[0].operations[0].clone();
    fresh_length.id = OperationId::new(21).unwrap();
    let terminal_psi::OperationResult::Scalar(result) = &mut fresh_length.result else {
        panic!("length is scalar")
    };
    result.id = ValueId::new(21).unwrap();
    module.machines[0].blocks[0].operations.push(fresh_length);
    let OperationKind::ByteSequenceRead { length, .. } =
        &mut module.machines[0].blocks[1].operations[0].kind
    else {
        panic!("byte read")
    };
    *length = ValueId::new(21).unwrap();
    // Equal contents do not make this SSA observation the guard's length fact.
    assert_byte_read_proof_rejected(&module, &proof);
}

#[test]
fn byte_view_read_rejects_counterfeit_and_different_descriptor_lengths() {
    for other_descriptor in [false, true] {
        let mut module = byte_view_read_module();
        let machine = &mut module.machines[0];
        if other_descriptor {
            let mut parameter = machine.structural_parameters[0].clone();
            parameter.place = PlaceId::new(21).unwrap();
            parameter.position = 1;
            machine.structural_parameters.push(parameter);
            machine.structural_places.push(StructuralPlaceDeclaration {
                id: PlaceId::new(21).unwrap(),
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            });
            machine.blocks[0].operations[0].kind = OperationKind::ByteSequenceLength {
                source: PlaceId::new(21).unwrap(),
            };
        } else {
            machine.blocks[0].operations[0].kind = OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(256),
            };
        }
        assert_eq!(
            terminal_codec::encode_module(&module),
            Err(terminal_codec::CodecError::InvalidModule(
                terminal_verifier::ModuleError::InvalidByteSequenceReadLength {
                    operation: OperationId::new(13).unwrap(),
                    source: PlaceId::new(3).unwrap(),
                    length: ValueId::new(5).unwrap(),
                }
            ))
        );
    }
}
