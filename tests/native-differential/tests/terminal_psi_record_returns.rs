//! Incoming record values remain live across ordinary writes and calls.
use target::NativeTarget;

#[path = "common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[path = "pipeline_ownership/native_execution.rs"]
#[allow(dead_code)]
mod native_execution;

const SOURCE: &str = "machine replace(destination: &write u64, value: u64) { destination = value; }
machine forward(destination: &write u64) {
    destination = 17;
    replace(&write destination, 29);
    destination = 41;
}";

fn native_text(target: NativeTarget) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    native_text_for(target, true, &[64])
}

fn optimized_record(
    effects: bool,
    field_bits: &[u16],
) -> abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "forward")
        .produce_artifact()
        .unwrap();
    // The frontend does not yet produce mixed structural-result bodies. Retain
    // its ordinary verified write/call operations and explicitly author the
    // independent Terminal parameter/result contract for this native customer.
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let record = semantic_vocabulary::StructuralTypeId::new(90001).unwrap();
    let input = semantic_vocabulary::PlaceId::new(90002).unwrap();
    let result = semantic_vocabulary::PlaceId::new(90003).unwrap();
    module
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: record,
            identity: "NativeRecord".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(90004).unwrap(),
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                64,
                            )
                            .unwrap(),
                        ),
                    ),
                }],
            },
        });
    {
        let terminal_psi::StructuralTypeShape::Record { fields } =
            &mut module.structural_types.last_mut().unwrap().shape
        else {
            unreachable!()
        };
        let template = fields[0].clone();
        *fields = field_bits
            .iter()
            .enumerate()
            .map(|(position, bits)| {
                let mut field = template.clone();
                field.id =
                    semantic_vocabulary::StructuralFieldId::new(90004 + position as u64).unwrap();
                field.identity = format!("field_{position}");
                field.field_type = terminal_psi::StructuralFieldType::Scalar(
                    semantic_vocabulary::ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            *bits,
                        )
                        .unwrap(),
                    ),
                );
                field
            })
            .collect();
    }
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    for parameter in &mut machine.structural_parameters {
        parameter.position += 1;
    }
    for place in &mut machine.structural_places {
        if let semantic_vocabulary::StructuralPlaceKind::Parameter { position, .. } =
            &mut place.kind
        {
            *position += 1;
        }
    }
    machine.structural_parameters.insert(
        0,
        terminal_psi::StructuralParameterDeclaration {
            place: input,
            position: 0,
            is_self: false,
            structural_type: record,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    );
    machine.structural_places.extend([
        terminal_psi::StructuralPlaceDeclaration {
            id: input,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: result,
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ]);
    machine.result = terminal_psi::TerminalMachineResult::Structural(
        terminal_psi::StructuralResultDeclaration {
            place: result,
            structural_type: record,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    );
    let block = machine.blocks.last_mut().unwrap();
    block.terminator = terminal_psi::Terminator::ReturnStructural {
        edge: block.terminator.edge(),
        source: input,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    if !effects {
        machine
            .structural_parameters
            .retain(|parameter| parameter.place == input);
        machine
            .structural_places
            .retain(|place| place.id == input || place.id == result);
        machine.blocks[0].operations.clear();
        module.machines.retain(|machine| machine.id == module.entry);
    }
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    native_realization::optimize_artifact_sections(
        &semantic,
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("record return with ordered writes must reach ordinary abstract operations")
}

fn native_text_for(
    target: NativeTarget,
    effects: bool,
    field_bits: &[u16],
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    let optimized = optimized_record(effects, field_bits);
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&placed).unwrap();
    placed
}

#[test]
fn incoming_record_return_replay_rejects_substituted_source_placement_and_capture() {
    use target_operations_to_selected_instructions::{
        legalize_target_operations, validate_legalized_operations, validate_selected_instructions,
    };
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let optimized = optimized_record(true, &[64]);
        let target =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized, native,
            )
            .unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let staged =
            target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                target,
                environment,
            )
            .unwrap();
        let target = staged.optimized_target();
        let abstracted = target.optimized();
        let entry = abstracted.plan().entry;
        for mutation in 0..3 {
            let mut raw = target.target_operations().clone();
            let function = raw
                .functions
                .iter_mut()
                .find(|function| function.machine == entry)
                .unwrap();
            let graph = &mut function.graph;
            let target_operations::TargetControlTerminator::ReturnStructural {
                source: target_operations::TargetStructuralReturnSource::Parameter(source),
                ..
            } = &mut graph.blocks[0].terminator
            else {
                unreachable!()
            };
            match mutation {
                0 => source.place = semantic_vocabulary::PlaceId::new(90003).unwrap(),
                1 => source.access = terminal_psi::StructuralAccess::SharedBorrow,
                _ => source.placement.locations = graph.parameters[1].placement.locations.clone(),
            }
            assert!(
                legalize_target_operations(&raw, abstracted.plan(), abstracted.unit()).is_err()
            );
        }
        let mut raw = staged.legalized().plan().clone();
        let function = raw
            .scalar_functions
            .iter_mut()
            .find(|function| function.machine == entry)
            .unwrap();
        let legalized_operations::LegalizedScalarTerminator::Return(returned) =
            &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        returned.value = legalized_operations::LegalizedScalarReturnValue::StructuralParameter {
            place: semantic_vocabulary::PlaceId::new(90003).unwrap(),
        };
        assert!(
            validate_legalized_operations(
                target.target_operations(),
                abstracted.plan(),
                abstracted.unit(),
                raw
            )
            .is_err()
        );
        let mut raw = staged.selected().plan().clone();
        let function = raw
            .functions
            .iter_mut()
            .find(|function| function.machine == entry)
            .unwrap();
        let capture = &mut function.blocks[0].instructions[0];
        assert_eq!(
            capture.kind,
            selected_instructions::SelectedInstructionKind::CopyI64
        );
        capture.operands[0].virtual_register = capture.operands[1].virtual_register;
        let constraints = target_operations_to_selected_instructions::selection_constraints(
            staged.legalized(),
            staged.register_environment(),
        );
        assert!(
            validate_selected_instructions(
                staged.legalized(),
                &constraints,
                staged.register_environment().physical(),
                staged.register_environment().constraints(),
                raw
            )
            .is_err()
        );
    }
}

#[test]
fn bare_incoming_record_return_is_an_observation_without_a_write_companion() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let placed = native_text_for(target, false, &[64]);
        assert!(
            placed
                .text_section()
                .resolved_internal_machine_calls
                .is_empty()
        );
    }
}

#[test]
fn two_register_record_returns_preserve_both_fragments_across_calls() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for fields in [&[32, 32, 32][..], &[64, 64][..]] {
            let placed = native_text_for(target, true, fields);
            assert_eq!(
                placed.text_section().resolved_internal_machine_calls.len(),
                1
            );
        }
    }
}

#[test]
fn four_byte_record_keeps_the_existing_direct_return_admission_fence() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let result = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized_record(false, &[32]),
            target,
        );
        assert!(matches!(result, Err(abstract_operations_to_target_operations::LoweringError::UnsupportedStructuralReturnPlacement(_))));
    }
}

#[test]
fn incoming_record_return_composes_with_writes_and_calls_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let placed = native_text(target);
        assert_eq!(
            placed.text_section().resolved_internal_machine_calls.len(),
            1
        );
    }
}

#[test]
fn caller_observes_preserved_record_and_ordered_write() {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        let placed = native_text(NativeTarget::windows_x64());
        let text = placed.text_section();
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == text.semantic_entry)
            .unwrap();
        let code = native_execution::Code::new(&text.bytes);
        let mut destination = 0_u64;
        let input = 0xabcdef1234567890;
        // The validated Microsoft x64 placement transports this 8-byte record
        // in the same integer register as its sole field and returns it in RAX.
        let output = code.call_scalar(
            entry.section_offset.try_into().unwrap(),
            [input, (&mut destination as *mut u64) as usize as u64, 0, 0],
        );
        assert_eq!(output, input);
        assert_eq!(destination, 41);
    }
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let placed = native_text(NativeTarget::host());
        let text = placed.text_section();
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == text.semantic_entry)
            .unwrap();
        native_function::assert_c_text(
            &text.bytes,
            entry.section_offset.try_into().unwrap(),
            r#"
            #include <stdint.h>
            typedef struct { uint64_t value; } Record;
            extern Record omega_entry(Record record, uint64_t *destination);
            int main(void) {
                uint64_t destination = 0;
                Record input = { UINT64_C(0xabcdef1234567890) };
                Record output = omega_entry(input, &destination);
                return output.value != input.value || destination != 41;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: native C caller requires supported Linux/macOS host; hosted targets are cross-lowered separately"
    );
}
