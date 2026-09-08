//! Source-authored natural slice ranking reaches the ordinary native pipeline.
use super::*;
#[path = "natural_writer/optimization.rs"]
mod optimization;
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use terminal_psi::{TerminalNaturalRankComparison, TerminalRankedScc, Terminator};

const WRITER: &str = r#"
    boundary trait Output { machine write(byte: i32) reaches Output; }
    machine relay(bytes: &[u8], newline: bool)
    terminates by bytes -> Slice::Length;
    reaches Output {
        transition bytes.len > 0 {
            true -> emit(bytes[0], bytes[1..], newline)
            false -> finish(newline)
        }
        state emit(byte: u8, bytes: &[u8], newline: bool) {
            Output::write(byte as i32);
            transition bytes.len > 0 {
                true -> emit(bytes[0], bytes[1..], newline)
                false -> finish(newline)
            }
        }
        state finish(newline: bool) {
            transition newline {
                true -> emit_newline(10u8)
                false -> done()
            }
        }
        state emit_newline(byte: u8) { Output::write(byte as i32); }
        state done() {}
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("", false);
        relay("", true);
        relay("\x80\0\xffAB", false);
        relay("Z", true);
        Output::write(33i32);
    }
"#;

fn writer() -> lowered_psi::LoweredPsi {
    let tokens = source_files_to_tokens::Lexer::new(WRITER)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("authored natural writer and caller produce their actual Terminal proof");
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    assert_eq!(verified.accepted_control_cycles().len(), 1);
    assert_eq!(
        verified.accepted_control_cycles()[0]
            .acceptance
            .decreases
            .len(),
        2
    );
    assert_eq!(lowered.proof_bundle.control_cycles.len(), 1);
    assert!(matches!(
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry),
        Err(terminal_fixed_fuel::FixedFuelError::ControlCycle(_))
    ));
    lowered
}

fn publish(target: NativeTarget) -> (image_emission::ExecutableImage, usize) {
    let lowered = writer();
    let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
        panic!("one authored output boundary");
    };
    // This stage fixture supplies its explicit closed realization. It does not
    // infer package/provider permission from the test's source spelling.
    let text = calls::stage_call_text_with_settlements(
        target,
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &[AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
            ),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        }],
    );
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    assert_eq!(object.text_bytes(), source.source().text_section().bytes);
    let entry_offset = object.entry_function().text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let decoded = image_emission::decode_installation_record(
        &image_emission::encode_installation_record(&record).unwrap(),
    )
    .unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    assert_eq!(
        image_emission::derive_stack_demand(&object, lowered.semantic_module.entry).unwrap(),
        image_emission::derive_installation_stack_demand(
            &decoded,
            &image,
            lowered.semantic_module.entry
        )
        .unwrap(),
    );
    (image, entry_offset)
}

#[test]
fn natural_slice_writer_publishes_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (image, _) = publish(target);
        assert!(!image.output().final_text_bytes.is_empty());
    }
}

#[test]
fn natural_slice_writer_executes_raw_empty_newline_and_caller_continuation() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, entry_offset) = publish(NativeTarget::host());
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            r#"
            #include <stdint.h>
            #include <unistd.h>
            extern void omega_entry(void);
            int main(void) {
                alarm(10);
                const uint8_t expected[] = {10, 0x80, 0, 0xff, 'A', 'B', 'Z', 10, '!'};
                int channel[2];
                if (pipe(channel)) return 1;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
                close(channel[1]);
                omega_entry();
                omega_entry();
                if (dup2(saved, STDOUT_FILENO) < 0) return 3;
                close(saved);
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (unsigned position = 0; position < sizeof(expected); ++position) {
                        uint8_t actual;
                        if (read(channel[0], &actual, 1) != 1 || actual != expected[position]) return 4;
                    }
                uint8_t extra;
                if (read(channel[0], &extra, 1) != 0) return 5;
                close(channel[0]);
                return 0;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: natural writer runtime requires Linux x64/AArch64 or macOS AArch64");
    #[cfg(not(target_os = "linux"))]
    eprintln!(
        "SKIP: Linux natural writer runtime requires a Linux host; cross-publication is separate"
    );
}

#[test]
fn natural_slice_writer_rejects_grouped_proof_substitution() {
    let lowered = writer();
    for mutation in 0..4 {
        let mut proof = lowered.proof_bundle.clone();
        match mutation {
            0 => proof.control_cycles.clear(),
            1 => proof.control_cycles[0].certificate.edges.reverse(),
            2 => proof.control_cycles[0].certificate.edges.clear(),
            _ => {
                proof.control_cycles[0].certificate.ranking_relation =
                    semantic_vocabulary::RankingRelationId::new(1).unwrap()
            }
        }
        assert!(
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &proof,
                &AdmissionProfile::default()
            )
            .is_err(),
            "grouped proof mutation {mutation}"
        );
    }
}

#[test]
fn natural_slice_writer_rejects_unchanged_tail_and_redirected_source() {
    let lowered = writer();
    for change_length in [false, true] {
        let mut module = lowered.semantic_module.clone();
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| machine.ranked_scc.is_some())
            .unwrap();
        let Some(TerminalRankedScc::Natural(components)) = &machine.ranked_scc else {
            panic!("natural component, not a fabricated countdown");
        };
        let strict = *components[0]
            .edges
            .iter()
            .find(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
            .unwrap();
        let incoming_view = machine
            .blocks
            .iter()
            .find(|block| block.id == strict.target)
            .unwrap()
            .structural_parameters[0]
            .place;
        let source = machine
            .blocks
            .iter_mut()
            .find(|block| block.id == strict.source)
            .unwrap();
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut source.terminator
        else {
            panic!("source-produced tail transfer");
        };
        structural_arguments[0].place = incoming_view;
        if change_length {
            let length = source
                .operations
                .iter_mut()
                .find(|operation| {
                    operation
                        .result
                        .scalar()
                        .is_some_and(|value| value.id == strict.successor_rank)
                })
                .unwrap();
            length.kind = OperationKind::ByteSequenceLength {
                source: incoming_view,
            };
            terminal_verifier::validate_module(&module)
                .expect("unchanged-tail substitution is well typed");
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &lowered.proof_bundle,
                &AdmissionProfile::default()
            )
            .is_err(),
            "changed tail length {change_length}"
        );
    }
}
