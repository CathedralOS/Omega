//! Local literal backing survives repeated native reader calls.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, StructuralPlaceKind, ValueId,
};
use target::NativeTarget;
use terminal_psi::{
    Operation, OperationKind, OperationResult, TerminalMachineResult, TerminalModule, Terminator,
};

const LITERALS: [&[u8]; 3] = [
    &[],
    &[0xff, 0x00, 0x80],
    &[
        0x4f, 0x6d, 0x65, 0x67, 0x61, 0x00, 0xff, 0x80, 0x17, 0xfe, 0x41,
    ],
];

fn literal_call_module(bytes: &[u8]) -> TerminalModule {
    let mut module = super::fixtures::byte_view_read_call_module();
    let caller = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = caller.structural_parameters.remove(0);
    let place = caller
        .structural_places
        .iter_mut()
        .find(|place| place.id == parameter.place)
        .unwrap();
    place.kind = StructuralPlaceKind::ByteSequenceLiteral {
        declaration_ordinal: 0,
        structural_type: parameter.structural_type,
    };
    caller.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(104).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place.id,
                bytes: bytes.to_vec(),
            },
        },
    );
    module
}

#[test]
fn literal_byte_view_calls_cross_lower_with_local_backing() {
    for bytes in LITERALS {
        let module = literal_call_module(bytes);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let placed = super::calls::stage_call_text(target, &module);
            let text = placed.text_section();
            assert_eq!(text.functions.len(), 2);
            let calls = text
                .resolved_internal_machine_calls
                .iter()
                .map(|call| (call.caller, call.operation, call.callee))
                .collect::<Vec<_>>();
            assert_eq!(
                calls,
                [105, 107].map(|operation| (
                    module.entry,
                    OperationId::new(operation).unwrap(),
                    MachineId::new(1).unwrap(),
                ))
            );
        }
    }
}

fn nested_literal_call_module(bytes: &[u8]) -> TerminalModule {
    let mut module = literal_call_module(bytes);
    let mut helper = super::fixtures::byte_view_read_call_module()
        .machines
        .remove(1);
    helper.id = MachineId::new(200).unwrap();
    helper.entry = BlockId::new(201).unwrap();
    helper.contract.id = ContractId::new(210).unwrap();
    helper.parameters[0].id = ValueId::new(203).unwrap();
    helper.structural_parameters[0].place = PlaceId::new(202).unwrap();
    helper.structural_places[0].id = PlaceId::new(202).unwrap();
    let TerminalMachineResult::Scalar(result) = &mut helper.result else {
        panic!("scalar helper")
    };
    result.id = ValueId::new(204).unwrap();
    helper.blocks[0].id = helper.entry;
    for (operation, (identity, result)) in helper.blocks[0]
        .operations
        .iter_mut()
        .zip([(205, 206), (207, 208)])
    {
        operation.id = OperationId::new(identity).unwrap();
        let OperationResult::Scalar(declaration) = &mut operation.result else {
            panic!("scalar call")
        };
        declaration.id = ValueId::new(result).unwrap();
        let OperationKind::CallStructuralScalar {
            arguments,
            structural_arguments,
            ..
        } = &mut operation.kind
        else {
            panic!("reader call")
        };
        arguments[0] = helper.parameters[0].id;
        structural_arguments[0].place = helper.structural_parameters[0].place;
    }
    helper.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(209).unwrap(),
        value: ValueId::new(208).unwrap(),
        cleanup_actions: Vec::new(),
    };
    for operation in &mut module.machines[1].blocks[0].operations {
        if let OperationKind::CallStructuralScalar { callee, .. } = &mut operation.kind {
            *callee = helper.id;
        }
    }
    module.machines.push(helper);
    module
}

const OTHER_LITERAL: &[u8] = &[
    0xfe, 0x81, 0x00, 0x18, 0x72, 0x40, 0x17, 0x80, 0xff, 0x01, 0x7f,
];

#[test]
fn source_literal_argument_composes_with_an_affine_call_result() {
    let source = r#"
        data Outcome { case Count(count: u64); }
        machine measure(bytes: &[u8]) -> Outcome {
            Outcome::Count { count: bytes.len }
        }
        data Root {}
        machine Root::enter() {
            _ = measure("abc");
            _ = measure("");
        }
    "#;
    let lowered = lower_result_call_source(source);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let text = super::calls::stage_call_text_with_proof(
            target,
            &lowered.semantic_module,
            &lowered.proof_bundle,
        );
        assert_eq!(text.text_section().resolved_internal_machine_calls.len(), 2);
    }
}

fn lower_result_call_source(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .unwrap()
}

#[test]
fn source_installed_provider_retains_literal_arguments_and_affine_results() {
    let lowered = lower_result_call_source(
        r#"
        data Outcome { case Count(count: u64); }
        boundary trait Sink {
            machine accept(bytes: &[u8]) reaches Sink;
            machine measure(bytes: &[u8]) -> Outcome reaches Sink;
        }
        data Meter {}
        machine Meter::accept(bytes: &[u8]) satisfies Sink::accept {}
        machine Meter::measure(bytes: &[u8]) -> Outcome satisfies Sink::measure {
            Outcome::Count { count: bytes.len }
        }
        data Root {}
        machine Root::enter() reaches Sink {
            Sink::accept("prefix");
            _ = Sink::measure("abc");
            _ = Sink::measure("");
        }
    "#,
    );
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let providers = lowered
        .semantic_module
        .provider_candidates
        .iter()
        .map(
            |provider| terminal_psi_to_abstract_operations::SelectedProviderAdapter {
                requirement_identity: provider.requirement_identity.clone(),
                provider_identity: provider.provider_identity.clone(),
                machine_identity: provider.candidate_identity.clone(),
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(providers.len(), 2);
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let selections = optimization_core::OptimizationSelections::new([]).unwrap();
        let optimized = native_realization::optimize_artifact_sections(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            native_realization::compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let installation = terminal_psi_to_abstract_operations::admit_provider_installation(
            optimized.plan(),
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &providers,
        )
        .unwrap();
        let target =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized,
                abstract_operations_to_target_operations::OptimizedTargetLoweringRequest {
                    target: native,
                    settlements: &[],
                    installation: Some(installation),
                    ieee_float_fma: &[],
                    native_callbacks: &[],
                },
            )
            .unwrap();
        let physical = native_realization::stage_optimized_verified_physical_pipeline(
            target,
            selections.project_post_terminal().selections(),
        )
        .unwrap();
        let fragments = machine_emission::stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap();
        let framed =
            machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
        let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
        machine_emission::validate_optimized_fixed_frame_text_section(&text).unwrap();
        assert_eq!(text.text_section().resolved_internal_machine_calls.len(), 3);
    }
}

fn independent_literal_call_module(return_first: bool) -> TerminalModule {
    let mut module = literal_call_module(LITERALS[2]);
    let caller = &mut module.machines[1];
    let mut second = caller.structural_places[0];
    second.id = PlaceId::new(112).unwrap();
    let StructuralPlaceKind::ByteSequenceLiteral {
        declaration_ordinal,
        ..
    } = &mut second.kind
    else {
        panic!("literal place")
    };
    *declaration_ordinal = 1;
    caller.structural_places.push(second);
    caller.blocks[0].operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(114).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: PlaceId::new(112).unwrap(),
                bytes: OTHER_LITERAL.to_vec(),
            },
        },
    );
    let mut last = caller.blocks[0].operations[3].clone();
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[3].kind
    else {
        panic!("second reader")
    };
    structural_arguments[0].place = PlaceId::new(112).unwrap();
    last.id = OperationId::new(115).unwrap();
    let OperationResult::Scalar(result) = &mut last.result else {
        panic!("scalar reader result")
    };
    result.id = ValueId::new(116).unwrap();
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut last.kind
    else {
        panic!("final reader")
    };
    structural_arguments[0].place = PlaceId::new(if return_first { 102 } else { 112 }).unwrap();
    caller.blocks[0].operations.push(last);
    let Terminator::Return { value, .. } = &mut caller.blocks[0].terminator else {
        panic!("scalar return")
    };
    *value = ValueId::new(116).unwrap();
    module
}

fn stage_literal_case(
    target: NativeTarget,
    module: &TerminalModule,
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    let placed = super::calls::stage_call_text(target, module);
    let mut expected = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block
                    .operations
                    .iter()
                    .filter_map(move |operation| match operation.kind {
                        OperationKind::CallStructuralScalar { callee, .. } => {
                            Some((machine.id, operation.id, callee))
                        }
                        _ => None,
                    })
            })
        })
        .collect::<Vec<_>>();
    let mut actual = placed
        .text_section()
        .resolved_internal_machine_calls
        .iter()
        .map(|call| (call.caller, call.operation, call.callee))
        .collect::<Vec<_>>();
    expected.sort();
    actual.sort();
    assert_eq!(
        actual, expected,
        "all authored helper calls retain resolved native occurrences"
    );
    assert_eq!(placed.text_section().functions.len(), module.machines.len());
    placed
}

#[test]
fn nested_and_independent_literal_calls_cross_lower_on_hosted_targets() {
    let modules = [
        nested_literal_call_module(LITERALS[2]),
        independent_literal_call_module(true),
        independent_literal_call_module(false),
    ];
    for module in &modules {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let placed = stage_literal_case(target, module);
            assert!(!placed.text_section().bytes.is_empty());
        }
    }
}

#[test]
fn literal_calls_execute_exact_raw_bytes_with_independent_local_backing() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        let mut cases = Vec::new();
        for bytes in LITERALS {
            cases.push((literal_call_module(bytes), bytes));
            cases.push((nested_literal_call_module(bytes), bytes));
        }
        cases.push((independent_literal_call_module(true), LITERALS[2]));
        cases.push((independent_literal_call_module(false), OTHER_LITERAL));
        for (module, expected) in cases {
            let placed = stage_literal_case(NativeTarget::host(), &module);
            let text = placed.text_section();
            let entry = text
                .functions
                .iter()
                .find(|function| function.machine == module.entry)
                .unwrap();
            // The C bytes are an output oracle only; Omega receives just the index.
            let expected_initializer = if expected.is_empty() {
                "0".into()
            } else {
                expected
                    .iter()
                    .map(|byte| byte.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let driver = format!(
                r#"
                #include <stdint.h>
                #include <unistd.h>
                extern uint64_t omega_entry(uint64_t);
                int main(void) {{
                    alarm(10);
                    const uint8_t expected[] = {{ {expected_initializer} }};
                    const uint64_t length = {};
                    for (unsigned repetition = 0; repetition < 3; ++repetition) {{
                        for (uint64_t position = 0; position < length; ++position)
                            if (omega_entry(position) != expected[position]) return 1;
                        if (omega_entry(length) != 256) return 2;
                        if (omega_entry(UINT64_MAX) != 256) return 3;
                    }}
                    return 0;
                }}
            "#,
                expected.len()
            );
            super::native_function::assert_c_text(
                &text.bytes,
                entry.section_offset.try_into().unwrap(),
                &driver,
            );
        }
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP literal-call runtime: Linux/macOS cc hosts supported; Windows execution route unavailable"
    );
}
