//! A caller-owned checked suffix reaches an effectful Unit reader.
use super::*;
use semantic_vocabulary::{BlockId, ContractId, MachineId};

#[path = "derived_unit/block_unit_transfers.rs"]
mod block_unit_transfers;
#[path = "derived_unit/publication.rs"]
mod publication;

fn output_call(identity: u64, argument: ValueId) -> Operation {
    Operation {
        static_reach_binding: None,
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: MachineId::new(200).unwrap(),
            arguments: vec![argument],
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }
}

fn derived_unit_output_module() -> TerminalModule {
    let mut module = super::super::subslice_calls::suffix_call_module();
    let mut output = widening::widened_byte_output_module();
    module.boundary_machines = output.boundary_machines;
    let leaf = &mut output.machines[0];
    leaf.id = MachineId::new(200).unwrap();
    leaf.entry = BlockId::new(202).unwrap();
    leaf.contract.id = ContractId::new(209).unwrap();
    leaf.parameters[0].id = ValueId::new(205).unwrap();
    leaf.blocks[0].id = leaf.entry;
    leaf.blocks[0].operations[0].id = OperationId::new(206).unwrap();
    leaf.blocks[0].operations[0].result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(206).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
    });
    leaf.blocks[0].operations[0].kind = OperationKind::IntegerWiden {
        operand: leaf.parameters[0].id,
    };
    leaf.blocks[0].operations[1].id = OperationId::new(207).unwrap();
    let OperationKind::BoundaryCall { arguments, .. } = &mut leaf.blocks[0].operations[1].kind
    else {
        panic!("byte boundary")
    };
    arguments[0] = ValueId::new(206).unwrap();
    leaf.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(208).unwrap(),
        trivial_affine_discards: Vec::new(),
    };

    let reader = &mut module.machines[0];
    reader.result = TerminalMachineResult::Unit;
    reader.blocks[1].operations.truncate(1);
    reader.blocks[1]
        .operations
        .push(output_call(14, ValueId::new(13).unwrap()));
    reader.blocks[2].operations.clear();
    for (block, edge) in reader.blocks[1..].iter_mut().zip([19, 20]) {
        block.terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(edge).unwrap(),
            trivial_affine_discards: Vec::new(),
        };
    }

    let caller = &mut module.machines[1];
    caller.result = TerminalMachineResult::Unit;
    caller.parameters.truncate(1);
    caller.blocks[1].operations.truncate(2);
    let call = &mut caller.blocks[1].operations[1];
    let OperationKind::CallStructuralScalar {
        callee,
        structural_arguments,
        ..
    } = &call.kind
    else {
        panic!("suffix reader")
    };
    call.kind = OperationKind::CallUnit {
        callee: *callee,
        arguments: vec![ValueId::new(120).unwrap()],
        structural_arguments: structural_arguments.clone(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    call.result = OperationResult::Unit;
    caller.blocks[1].operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            id: OperationId::new(120).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(120).unwrap(),
                scalar_type: caller.parameters[0].scalar_type,
            }),
            kind: OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
            },
        },
    );
    caller.blocks[1].terminator = Terminator::Jump {
        edge: EdgeId::new(109).unwrap(),
        target: BlockId::new(140).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks[2].operations[0].result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(141).unwrap(),
        scalar_type: leaf.parameters[0].scalar_type,
    });
    caller.blocks[2].operations[0].kind = OperationKind::IntegerConstant {
        value: semantic_vocabulary::IntegerValue::Unsigned(33),
    };
    caller.blocks[2]
        .operations
        .push(output_call(143, ValueId::new(141).unwrap()));
    caller.blocks[2].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(142).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    module.machines.push(output.machines.remove(0));
    module
}

fn stage_derived_unit_output(
    target: NativeTarget,
    module: &TerminalModule,
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    let proof = super::super::subslice::suffix_proof(module);
    let text = calls::stage_call_text_with_settlements(
        target,
        module,
        &proof,
        &[AdmittedBoundarySettlement {
            boundary: module.boundary_machines[0].id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(
                CompilerBuiltinExecution::HostedWriteByteI32,
            ),
            realization: HostedWriteByteI32Realization.into(),
        }],
    );
    assert_eq!(text.text_section().functions.len(), 3);
    let calls = &text.text_section().resolved_internal_machine_calls;
    assert_eq!(calls.len(), 3);
    for (caller, operation, callee) in [(1, 14, 200), (100, 105, 1), (100, 143, 200)] {
        assert!(
            calls
                .iter()
                .any(|call| call.caller == MachineId::new(caller).unwrap()
                    && call.operation == OperationId::new(operation).unwrap()
                    && call.callee == MachineId::new(callee).unwrap())
        );
    }
    text
}

#[test]
fn derived_view_unit_output_cross_lowers_on_linux_targets() {
    let module = derived_unit_output_module();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        assert!(
            !stage_derived_unit_output(target, &module)
                .text_section()
                .bytes
                .is_empty()
        );
    }
}

#[test]
fn derived_view_unit_output_rejects_unavailable_or_owned_suffix_sources() {
    let module = derived_unit_output_module();
    let proof = super::super::subslice::suffix_proof(&module);
    for mutation in [
        "future",
        "sibling",
        "owned",
        "missing",
        "producer",
        "guard_edge",
    ] {
        let mut changed = module.clone();
        let caller = &mut changed.machines[1];
        match mutation {
            "future" => {
                let producer = caller.blocks[1].operations.remove(0);
                caller.blocks[1].operations.push(producer);
            }
            "sibling" => {
                // Move both the zero operand and call, leaving the view's
                // producer only on the other arrival to the continuation.
                let call = caller.blocks[1].operations.remove(2);
                let zero = caller.blocks[1].operations.remove(1);
                caller.blocks[2].operations.splice(0..0, [zero, call]);
            }
            "owned" | "missing" => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut caller.blocks[1].operations[2].kind
                else {
                    panic!("derived Unit call")
                };
                if mutation == "owned" {
                    structural_arguments[0].access = terminal_psi::StructuralAccess::Owned;
                } else {
                    structural_arguments[0].place = PlaceId::new(999).unwrap();
                }
            }
            "producer" => caller.blocks[1].operations[0].id = OperationId::new(999).unwrap(),
            "guard_edge" => {
                let Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } = &mut caller.blocks[0].terminator
                else {
                    panic!("suffix guard")
                };
                std::mem::swap(when_true, when_false);
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(&changed, &proof, &AdmissionProfile::default())
                .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn derived_view_unit_output_executes_raw_suffix_head_then_continuation() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, entry_offset) = publication::published_image(NativeTarget::host());
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            r#"
            #include <stdint.h>
            #include <stddef.h>
            #include <unistd.h>
            struct ByteView { const uint8_t *bytes; uint64_t length; };
            extern void omega_entry(uint64_t start, const struct ByteView *view);
            int main(void) {
                alarm(10);
                const uint8_t raw[] = { 0xff, 0, 0x80, 0x41, 0xfe };
                const uint8_t other[] = { 0x19, 0x81, 0 };
                const struct ByteView cases[] = {
                    {raw, sizeof(raw)}, {other, sizeof(other)}, {NULL, 0},
                    {raw, 1}, {other, sizeof(other)}, {raw, sizeof(raw)}
                };
                uint8_t expected[256]; size_t count = 0;
                int channel[2];
                if (pipe(channel)) return 1;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
                close(channel[1]);
                struct ByteView view;
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (size_t sample = 0; sample < sizeof(cases)/sizeof(cases[0]); ++sample) {
                        view = cases[sample];
                        for (uint64_t start = 0; start <= view.length + 1; ++start) {
                            omega_entry(start, &view);
                            if (start < view.length) expected[count++] = view.bytes[start];
                            expected[count++] = '!';
                            if (view.bytes != cases[sample].bytes || view.length != cases[sample].length) return 3;
                        }
                        omega_entry(UINT64_MAX, &view); expected[count++] = '!';
                    }
                /* Empty suffix must not dereference even wrapped address zero. */
                view.bytes = (const uint8_t *)(uintptr_t)UINT64_MAX; view.length = 1;
                omega_entry(1, &view); expected[count++] = '!';
                if (dup2(saved, STDOUT_FILENO) < 0) return 4;
                close(saved);
                for (size_t position = 0; position < count; ++position) {
                    uint8_t actual;
                    if (read(channel[0], &actual, 1) != 1 || actual != expected[position]) return 5;
                }
                uint8_t extra;
                if (read(channel[0], &extra, 1) != 0) return 6;
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
    eprintln!("SKIP: derived-view byte output requires a supported Linux or macOS host");
}
