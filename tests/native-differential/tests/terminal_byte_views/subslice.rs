//! Native execution retains checked subslice bounds and the derived view.
use super::*;
use proof_admission::{EvidenceRoute, ProofRule};

#[path = "subslice/fixtures.rs"]
mod fixtures;

use fixtures::{derived_read_module, nested_suffix_module, subrange_module};
pub(super) use fixtures::{suffix_module, suffix_proof};

#[test]
fn borrowed_suffix_cross_lowers_on_hosted_targets() {
    for module in [
        suffix_module(),
        nested_suffix_module(),
        derived_read_module(false),
        derived_read_module(true),
        subrange_module(false),
        subrange_module(true),
    ] {
        let proof = suffix_proof(&module);
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
}

fn assert_subslice_execution(module: &TerminalModule, caller: &str) {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        let semantic = terminal_codec::encode_module(module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&suffix_proof(module)).unwrap();
        let selections = OptimizationSelections::new([]).unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .expect("canonical subslice reaches the optimizer");
        let physical = native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized, NativeTarget::host(), &[],
        ).expect("checked subslices reach the complete physical pipeline");
        let fragments = machine_emission::stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap();
        let framed =
            machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
        let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
        let text = placed.text_section();
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        native_function::assert_c_text(
            &text.bytes,
            entry.section_offset.try_into().unwrap(),
            caller,
        );
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    {
        let _ = (module, caller);
        eprintln!(
            "SKIP subslice native execution: Linux x86-64/AArch64 or macOS AArch64 cc harness required; Windows execution route unavailable"
        );
    }
}

#[test]
fn borrowed_suffix_executes_empty_full_and_endpoint_boundaries() {
    assert_subslice_execution(
        &suffix_module(),
        r#"
        #include <stdint.h>
        #include <stddef.h>
        #include <unistd.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern uint64_t omega_entry(uint64_t, const struct ByteView *);
        int main(void) {
            alarm(10);
            const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41, 0xfe };
            struct ByteView view = { NULL, 0 };
            if (omega_entry(0, &view) != 0) return 1;
            if (omega_entry(1, &view) != 256) return 2;
            if (omega_entry(UINT64_MAX, &view) != 256) return 3;
            view.bytes = raw; view.length = sizeof(raw);
            for (uint64_t start = 0; start <= sizeof(raw); ++start)
                if (omega_entry(start, &view) != sizeof(raw) - start) return 4;
            if (omega_entry(sizeof(raw) + 1, &view) != 256) return 5;
            if (omega_entry(UINT64_MAX, &view) != 256) return 6;
            if (view.bytes != raw || view.length != sizeof(raw)) return 7;
            view.length = 1;
            if (omega_entry(0, &view) != 1 || omega_entry(1, &view) != 0) return 8;
            view.bytes = NULL; view.length = 0;
            if (omega_entry(0, &view) != 0) return 9;
            return 0;
        }
    "#,
    );
}

#[test]
fn nested_suffix_executes_lengths_relative_to_each_descriptor() {
    assert_subslice_execution(
        &nested_suffix_module(),
        r#"
        #include <stdint.h>
        #include <stddef.h>
        #include <unistd.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern uint64_t omega_entry(uint64_t, uint64_t, const struct ByteView *);
        int main(void) {
            alarm(10);
            const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41, 0xfe };
            struct ByteView view = { NULL, 0 };
            if (omega_entry(0, 0, &view) != 0) return 1;
            if (omega_entry(0, 1, &view) != 256) return 2;
            view.bytes = raw; view.length = sizeof(raw);
            for (uint64_t first = 0; first <= sizeof(raw); ++first) {
                const uint64_t remaining = sizeof(raw) - first;
                for (uint64_t second = 0; second <= remaining; ++second)
                    if (omega_entry(first, second, &view) != remaining - second) return 3;
                if (omega_entry(first, remaining + 1, &view) != 256) return 4;
                if (omega_entry(first, UINT64_MAX, &view) != 256) return 5;
            }
            if (omega_entry(sizeof(raw) + 1, 0, &view) != 256) return 6;
            if (omega_entry(UINT64_MAX, UINT64_MAX, &view) != 256) return 7;
            if (view.bytes != raw || view.length != sizeof(raw)) return 8;
            return 0;
        }
    "#,
    );
}

#[test]
fn suffix_reads_execute_raw_bytes_and_skip_empty_or_invalid_views() {
    assert_subslice_execution(
        &derived_read_module(false),
        r#"
        #include <stdint.h>
        #include <stddef.h>
        #include <unistd.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern uint64_t omega_entry(uint64_t, uint64_t, const struct ByteView *);
        int main(void) {
            alarm(10);
            const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41, 0xfe };
            const uint8_t other[] = { 0x17, 0xfe };
            struct ByteView view = { NULL, 0 };
            if (omega_entry(0, 0, &view) != 256) return 1;
            view.bytes = raw; view.length = sizeof(raw);
            for (uint64_t start = 0; start <= sizeof(raw); ++start) {
                const uint64_t remaining = sizeof(raw) - start;
                for (uint64_t position = 0; position < remaining; ++position)
                    if (omega_entry(start, position, &view) != raw[start + position]) return 2;
                if (omega_entry(start, remaining, &view) != 256) return 3;
                if (omega_entry(start, UINT64_MAX, &view) != 256) return 4;
            }
            if (omega_entry(sizeof(raw) + 1, 0, &view) != 256) return 5;
            if (omega_entry(UINT64_MAX, 0, &view) != 256) return 6;
            if (view.bytes != raw || view.length != sizeof(raw)) return 7;
            view.bytes = other; view.length = sizeof(other);
            if (omega_entry(1, 0, &view) != 0xfe) return 8;
            view.bytes = NULL; view.length = 0;
            if (omega_entry(0, 0, &view) != 256) return 9;
            return 0;
        }
    "#,
    );
}

#[test]
fn nested_suffix_reads_execute_accumulated_offsets_and_derived_bounds() {
    assert_subslice_execution(
        &derived_read_module(true),
        r#"
        #include <stdint.h>
        #include <stddef.h>
        #include <unistd.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern uint64_t omega_entry(uint64_t, uint64_t, uint64_t, const struct ByteView *);
        int main(void) {
            alarm(10);
            const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41, 0xfe };
            struct ByteView view = { NULL, 0 };
            if (omega_entry(0, 0, 0, &view) != 256) return 1;
            view.bytes = raw; view.length = sizeof(raw);
            for (uint64_t first = 0; first <= sizeof(raw); ++first) {
                for (uint64_t second = 0; second <= sizeof(raw) - first; ++second) {
                    const uint64_t remaining = sizeof(raw) - first - second;
                    for (uint64_t position = 0; position < remaining; ++position)
                        if (omega_entry(first, second, position, &view) != raw[first + second + position]) return 2;
                    if (omega_entry(first, second, remaining, &view) != 256) return 3;
                    if (omega_entry(first, second, UINT64_MAX, &view) != 256) return 4;
                }
                if (omega_entry(first, sizeof(raw) - first + 1, 0, &view) != 256) return 5;
                if (omega_entry(first, UINT64_MAX, 0, &view) != 256) return 6;
            }
            if (omega_entry(UINT64_MAX, 0, 0, &view) != 256) return 7;
            if (view.bytes != raw || view.length != sizeof(raw)) return 8;
            view.bytes = NULL; view.length = 0;
            if (omega_entry(0, 0, 0, &view) != 256) return 9;
            return 0;
        }
    "#,
    );
}

#[test]
fn dynamic_subrange_executes_middle_empty_reversed_and_overrun_lengths() {
    assert_subslice_execution(
        &subrange_module(false),
        r#"
        #include <stdint.h>
        #include <stddef.h>
        #include <unistd.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern uint64_t omega_entry(uint64_t, uint64_t, const struct ByteView *);
        int main(void) {
            alarm(10);
            const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41, 0xfe };
            struct ByteView view = { raw, sizeof(raw) };
            if (omega_entry(1, 3, &view) != 2) return 1;
            if (omega_entry(2, 2, &view) != 0) return 2;
            if (omega_entry(3, 1, &view) != 256) return 3;
            if (omega_entry(1, 6, &view) != 256) return 4;
            for (uint64_t start = 0; start <= sizeof(raw) + 1; ++start)
                for (uint64_t end = 0; end <= sizeof(raw) + 1; ++end) {
                    uint64_t expected = start <= end && end <= sizeof(raw) ? end - start : 256;
                    if (omega_entry(start, end, &view) != expected) return 5;
                }
            if (omega_entry(1, UINT64_MAX, &view) != 256) return 6;
            if (omega_entry(UINT64_MAX, 1, &view) != 256) return 7;
            if (omega_entry(UINT64_MAX, UINT64_MAX, &view) != 256) return 8;
            if (view.bytes != raw || view.length != sizeof(raw)) return 9;
            view.bytes = NULL; view.length = 0;
            if (omega_entry(0, 0, &view) != 0) return 10;
            if (omega_entry(0, 1, &view) != 256) return 11;
            return 0;
        }
    "#,
    );
}

#[test]
fn dynamic_subrange_reads_stop_at_its_own_end() {
    assert_subslice_execution(
        &subrange_module(true),
        r#"
        #include <stdint.h>
        #include <stddef.h>
        #include <unistd.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern uint64_t omega_entry(uint64_t, uint64_t, uint64_t, const struct ByteView *);
        int main(void) {
            alarm(10);
            const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41, 0xfe };
            struct ByteView view = { raw, sizeof(raw) };
            if (omega_entry(1, 3, 0, &view) != 0x00) return 1;
            if (omega_entry(1, 3, 1, &view) != 0x80) return 2;
            if (omega_entry(1, 3, 2, &view) != 256) return 3;
            if (omega_entry(2, 2, 0, &view) != 256) return 4;
            if (omega_entry(3, 1, 0, &view) != 256) return 5;
            if (omega_entry(1, 6, 0, &view) != 256) return 6;
            for (uint64_t start = 0; start <= sizeof(raw); ++start)
                for (uint64_t end = start; end <= sizeof(raw); ++end) {
                    for (uint64_t position = 0; position < end - start; ++position)
                        if (omega_entry(start, end, position, &view) != raw[start + position]) return 7;
                    if (omega_entry(start, end, end - start, &view) != 256) return 8;
                    if (omega_entry(start, end, UINT64_MAX, &view) != 256) return 9;
                }
            if (omega_entry(1, UINT64_MAX, 0, &view) != 256) return 10;
            if (view.bytes != raw || view.length != sizeof(raw)) return 11;
            view.bytes = NULL; view.length = 0;
            if (omega_entry(0, 0, 0, &view) != 256) return 12;
            return 0;
        }
    "#,
    );
}

#[test]
fn dynamic_subrange_requires_both_endpoint_guards() {
    let module = subrange_module(false);
    let proof = suffix_proof(&module);
    for guard_block in [0, 3] {
        let mut unguarded = module.clone();
        let terminal_psi::Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = &mut unguarded.machines[0].blocks[guard_block].terminator
        else {
            panic!("subrange guard")
        };
        std::mem::swap(&mut when_true.target, &mut when_false.target);
        assert_byte_read_proof_rejected(&unguarded, &proof);
    }
}

#[test]
fn suffix_rejects_endpoint_and_proof_substitutions_before_lowering() {
    let module = suffix_module();
    let proof = suffix_proof(&module);
    assert_byte_read_proof_rejected(&module, &ProofBundle::default());
    let mut retargeted = proof.clone();
    retargeted.evidence[0].obligation = ObligationId::new(2).unwrap();
    assert_byte_read_proof_rejected(&module, &retargeted);
    let mut missing_leg = proof.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut missing_leg.evidence[0].route else {
        panic!("canonical range certificate")
    };
    let ProofRule::ConjunctionIntroduction(legs) = &mut certificate.proof.rule else {
        panic!("two checked range legs")
    };
    assert_eq!(legs.len(), 2);
    legs.truncate(1);
    assert_byte_read_proof_rejected(&module, &missing_leg);
    let mut changed_endpoint = module;
    let OperationKind::ByteSequenceSubslice { end, .. } =
        &mut changed_endpoint.machines[0].blocks[1].operations[0].kind
    else {
        panic!("suffix")
    };
    *end = ValueId::new(10).unwrap();
    assert_byte_read_proof_rejected(&changed_endpoint, &proof);
}

#[test]
fn derived_read_rejects_a_guard_that_includes_the_empty_endpoint() {
    let mut module = derived_read_module(true);
    let proof = suffix_proof(&module);
    let guard = module.machines[0].blocks[3].operations.last_mut().unwrap();
    guard.kind = OperationKind::IntegerLessOrEqual {
        left: ValueId::new(50).unwrap(),
        right: ValueId::new(42).unwrap(),
    };
    assert_byte_read_proof_rejected(&module, &proof);
}

#[test]
fn suffix_rejects_a_length_witness_from_another_source() {
    let mut module = suffix_module();
    let machine = &mut module.machines[0];
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
    machine.structural_places.sort_by_key(|place| place.id);
    let OperationKind::ByteSequenceSubslice { source, .. } =
        &mut machine.blocks[1].operations[0].kind
    else {
        panic!("suffix")
    };
    *source = PlaceId::new(21).unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module),
        Err(terminal_codec::CodecError::InvalidModule(
            terminal_verifier::ModuleError::InvalidByteSequenceSubslice(
                OperationId::new(13).unwrap()
            ),
        )),
    );
}
