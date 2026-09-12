//! Calls receive a descriptor computed inside the caller, not a substituted input.
use super::*;
use semantic_vocabulary::{BlockId, EdgeId, ScalarType};
use terminal_psi::{
    Block, Operation, OperationResult, StructuralMultiplicity, StructuralOperationResult,
    SuccessorEdge, Terminator, ValueDeclaration,
};

pub(super) fn suffix_call_module() -> TerminalModule {
    let mut module = fixtures::byte_view_read_call_module();
    let caller = &mut module.machines[1];
    let scalar_type = caller.parameters[0].scalar_type;
    let position = ValueId::new(120).unwrap();
    let length = ValueId::new(121).unwrap();
    let condition = ValueId::new(122).unwrap();
    let suffix = PlaceId::new(130).unwrap();
    let structural_type = caller.structural_parameters[0].structural_type;
    caller.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: position,
        scalar_type,
    });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: suffix,
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(130).unwrap(),
            structural_type,
        },
    });
    let mut calls = caller.blocks.remove(0);
    calls.id = BlockId::new(130).unwrap();
    for operation in &mut calls.operations {
        let OperationKind::CallStructuralScalar {
            arguments,
            structural_arguments,
            ..
        } = &mut operation.kind
        else {
            panic!("reader call")
        };
        arguments[0] = position;
        structural_arguments[0].place = suffix;
    }
    calls.operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            id: OperationId::new(130).unwrap(),
            result: OperationResult::Structural(StructuralOperationResult {
                place: suffix,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::ByteSequenceSubslice {
                source: PlaceId::new(102).unwrap(),
                start: ValueId::new(103).unwrap(),
                end: length,
                length,
                obligation: ObligationId::new(2).unwrap(),
            },
        },
    );
    let successor = |edge, block| SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks = vec![
        Block {
            id: caller.entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(121).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: length,
                        scalar_type,
                    }),
                    kind: OperationKind::ByteSequenceLength {
                        source: PlaceId::new(102).unwrap(),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(122).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: condition,
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessOrEqual {
                        left: ValueId::new(103).unwrap(),
                        right: length,
                    },
                },
            ],
            terminator: Terminator::Conditional {
                condition,
                when_true: successor(123, 130),
                when_false: successor(124, 140),
            },
        },
        calls,
        Block {
            id: BlockId::new(140).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(141).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: ValueId::new(141).unwrap(),
                    scalar_type,
                }),
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(256),
                },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(142).unwrap(),
                value: ValueId::new(141).unwrap(),
                cleanup_actions: Vec::new(),
            },
        },
    ];
    module
}

fn nested_suffix_call_module() -> TerminalModule {
    let mut module = suffix_call_module();
    let mut helper = fixtures::byte_view_read_call_module().machines.remove(1);
    helper.id = semantic_vocabulary::MachineId::new(200).unwrap();
    helper.entry = BlockId::new(201).unwrap();
    helper.contract.id = semantic_vocabulary::ContractId::new(210).unwrap();
    helper.parameters[0].id = ValueId::new(203).unwrap();
    helper.structural_parameters[0].place = PlaceId::new(202).unwrap();
    helper.structural_places[0].id = PlaceId::new(202).unwrap();
    let terminal_psi::TerminalMachineResult::Scalar(result) = &mut helper.result else {
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
    for operation in &mut module.machines[1].blocks[1].operations {
        if let OperationKind::CallStructuralScalar { callee, .. } = &mut operation.kind {
            *callee = helper.id;
        }
    }
    module.machines.push(helper);
    module
}

fn stage_suffix_calls(
    target: NativeTarget,
    module: &TerminalModule,
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    let proof = subslice::suffix_proof(module);
    let placed = calls::stage_call_text_with_proof(target, module, &proof);
    let mut expected = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block.operations.iter().filter_map(move |operation| {
                    if let OperationKind::CallStructuralScalar { callee, .. } = operation.kind {
                        Some((machine.id, operation.id, callee))
                    } else {
                        None
                    }
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
    expected.sort_unstable();
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "each authored call remains in independently resolved native text"
    );
    assert_eq!(placed.text_section().functions.len(), module.machines.len());
    placed
}

#[test]
fn nested_derived_suffix_calls_cross_lower_on_hosted_targets() {
    let module = nested_suffix_call_module();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        assert!(
            !stage_suffix_calls(target, &module)
                .text_section()
                .bytes
                .is_empty()
        );
    }
}

#[test]
fn derived_suffix_calls_execute_raw_empty_and_wrapped_empty_views() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    for module in [suffix_call_module(), nested_suffix_call_module()] {
        let placed = stage_suffix_calls(NativeTarget::host(), &module);
        let text = placed.text_section();
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
            extern uint64_t omega_entry(uint64_t start, uint64_t position, const struct ByteView *view);
            int main(void) {
                alarm(10);
                const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41, 0x17, 0xfe, 0x00 };
                const uint8_t other[] = { 0x19, 0x81, 0x00 };
                struct ByteView view = { raw, sizeof(raw) };
                for (unsigned repetition = 0; repetition < 3; ++repetition) {
                    for (uint64_t start = 0; start <= sizeof(raw); ++start) {
                        for (uint64_t position = 0; position < sizeof(raw) - start; ++position)
                            if (omega_entry(start, position, &view) != raw[start + position]) return 1;
                        if (omega_entry(start, sizeof(raw) - start, &view) != 256) return 2;
                        if (omega_entry(start, UINT64_MAX, &view) != 256) return 3;
                    }
                    if (omega_entry(sizeof(raw) + 1, 0, &view) != 256) return 4;
                    if (omega_entry(UINT64_MAX, 0, &view) != 256) return 5;
                    if (view.bytes != raw || view.length != sizeof(raw)) return 6;
                }
                view.bytes = other; view.length = sizeof(other);
                if (omega_entry(1, 0, &view) != 0x81) return 7;
                if (omega_entry(1, 1, &view) != 0) return 8;
                view.length = 1;
                if (omega_entry(1, 0, &view) != 256) return 9;
                view.bytes = NULL; view.length = 0;
                if (omega_entry(0, 0, &view) != 256) return 10;
                if (omega_entry(UINT64_MAX, UINT64_MAX, &view) != 256) return 11;
                /* The native descriptor address wraps; its zero length forbids a byte load.
                   The C harness never performs arithmetic on or dereferences this address. */
                view.bytes = (const uint8_t *)(uintptr_t)UINT64_MAX; view.length = 1;
                if (omega_entry(1, 0, &view) != 256) return 12;
                if (omega_entry(1, UINT64_MAX, &view) != 256) return 13;
                if (view.bytes != (const uint8_t *)(uintptr_t)UINT64_MAX || view.length != 1) return 14;
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
        "SKIP: native C execution requires supported Linux or macOS host; four-host cross-lowering is separate"
    );
}

#[test]
fn derived_suffix_calls_cross_lower_on_hosted_targets() {
    let module = suffix_call_module();
    let proof = subslice::suffix_proof(&module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let placed = calls::stage_call_text_with_proof(target, &module, &proof);
        assert_eq!(
            placed.text_section().resolved_internal_machine_calls.len(),
            2
        );
    }
}

#[test]
fn derived_suffix_calls_reject_future_sibling_missing_and_owned_sources() {
    let module = suffix_call_module();
    let proof = subslice::suffix_proof(&module);
    for corruption in 0..4 {
        let mut proposed = module.clone();
        let caller = &mut proposed.machines[1];
        match corruption {
            0 => caller.blocks[1].operations.swap(0, 1),
            1 => {
                let sibling_call = caller.blocks[1].operations.remove(1);
                caller.blocks[2].operations.push(sibling_call);
            }
            2 | 3 => {
                let OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } = &mut caller.blocks[1].operations[1].kind
                else {
                    panic!("reader call")
                };
                if corruption == 2 {
                    structural_arguments[0].place = PlaceId::new(999).unwrap();
                } else {
                    structural_arguments[0].access = terminal_psi::StructuralAccess::Owned;
                }
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(&proposed, &proof, &AdmissionProfile::default())
                .is_err(),
            "derived call source corruption {corruption}"
        );
    }
}
