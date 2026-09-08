//! True Unit calls retain scalar/reference inputs without a synthetic scalar result.

use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, ScalarType, ValueId};
use target::NativeTarget;
use terminal_psi::{Operation, OperationKind, OperationResult, StructuralAccess, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration};

#[path = "unit_calls/admission.rs"]
mod admission;

fn unit_call_module() -> TerminalModule {
    let mut module = super::fixtures::byte_view_read_call_module();
    let helper = module.machines.iter_mut().find(|machine| machine.id == module.entry).unwrap();
    helper.result = TerminalMachineResult::Unit;
    helper.parameters.push(ValueDeclaration { id: ValueId::new(114).unwrap(), scalar_type: ScalarType::Boolean });
    helper.blocks[0].operations.truncate(1);
    // This is an actual scalar read whose value is unused, followed by a true Unit exit.
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(109).unwrap(), trivial_affine_discards: Vec::new(),
    };
    let mut entry = helper.clone();
    entry.id = MachineId::new(200).unwrap();
    entry.entry = BlockId::new(201).unwrap();
    entry.contract.id = ContractId::new(210).unwrap();
    entry.parameters[0].id = ValueId::new(203).unwrap();
    entry.parameters[1].id = ValueId::new(204).unwrap();
    entry.structural_parameters[0].place = PlaceId::new(202).unwrap();
    entry.structural_places[0].id = PlaceId::new(202).unwrap();
    entry.blocks[0].id = entry.entry;
    let call = |operation, newline| Operation {
        id: OperationId::new(operation).unwrap(), result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: helper.id,
            arguments: vec![entry.parameters[0].id, newline],
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: entry.structural_parameters[0].place,
                path: Vec::new(), access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(), requirement_obligations: Vec::new(), crash_continuations: Vec::new(),
        },
    };
    entry.blocks[0].operations = vec![
        call(205, entry.parameters[1].id),
        Operation {
            id: OperationId::new(206).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration { id: ValueId::new(206).unwrap(), scalar_type: ScalarType::Boolean }),
            kind: OperationKind::BooleanConstant { value: false },
        },
        call(207, ValueId::new(206).unwrap()),
    ];
    entry.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(209).unwrap(), trivial_affine_discards: Vec::new(),
    };
    module.entry = entry.id;
    module.machines.push(entry);
    module
}

#[test]
fn unit_byte_view_calls_cross_lower_with_true_unit_results() {
    let module = unit_call_module();
    for machine in module.machines.iter().filter(|machine| machine.id != MachineId::new(1).unwrap()) {
        assert_eq!(machine.result, TerminalMachineResult::Unit);
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if matches!(operation.kind, OperationKind::CallUnit { .. }) {
                assert_eq!(operation.result, OperationResult::Unit);
            }
        }
    }
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64(), NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let placed = super::calls::stage_call_text(target, &module);
        let text = placed.text_section();
        assert_eq!(text.functions.len(), 3);
        let actual = text.resolved_internal_machine_calls.iter()
            .map(|call| (call.caller, call.operation, call.callee)).collect::<Vec<_>>();
        assert_eq!(actual, [(100, 105, 1), (200, 205, 100), (200, 207, 100)].map(|(caller, operation, callee)| (
            MachineId::new(caller).unwrap(), OperationId::new(operation).unwrap(), MachineId::new(callee).unwrap(),
        )));
    }
}

#[test]
fn unit_byte_view_calls_return_without_corrupting_descriptor_or_stack() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        let module = unit_call_module();
        let placed = super::calls::stage_call_text(NativeTarget::host(), &module);
        let text = placed.text_section();
        assert_eq!(text.resolved_internal_machine_calls.len(), 3);
        let entry = text.functions.iter().find(|function| function.machine == module.entry).unwrap();
        // Unit supplies no byte-content or Boolean observation. Exact argument checks
        // live in the receiving controls; this oracle checks return and preservation.
        super::native_function::assert_c_text(&text.bytes, entry.section_offset.try_into().unwrap(), r#"
            #include <stdint.h>
            #include <stddef.h>
            #include <unistd.h>
            struct ByteView { const uint8_t *bytes; uint64_t length; };
            extern void omega_entry(uint64_t, _Bool, const struct ByteView *);
            int main(void) {
                alarm(10);
                const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41 };
                const uint8_t other[] = { 0x17, 0xfe };
                struct {
                    volatile uint64_t before[4];
                    struct ByteView view;
                    volatile uint64_t after[4];
                } frame;
                for (unsigned position = 0; position < 4; ++position) {
                    frame.before[position] = UINT64_C(0xfacecafe12345678) + position;
                    frame.after[position] = UINT64_C(0x87654321abcdef01) + position;
                }
                const struct ByteView views[] = { {NULL, 0}, {raw, sizeof(raw)}, {other, sizeof(other)}, {raw, 1} };
                const uint64_t indices[] = { 0, 1, 3, 4, UINT64_MAX };
                for (unsigned view = 0; view < 4; ++view) {
                    frame.view = views[view];
                    for (unsigned position = 0; position < 5; ++position) {
                        for (unsigned newline = 0; newline < 2; ++newline) {
                            omega_entry(indices[position], (_Bool)newline, &frame.view);
                            if (frame.view.bytes != views[view].bytes || frame.view.length != views[view].length) return 1;
                            for (unsigned canary = 0; canary < 4; ++canary) {
                                if (frame.before[canary] != UINT64_C(0xfacecafe12345678) + canary) return 2;
                                if (frame.after[canary] != UINT64_C(0x87654321abcdef01) + canary) return 3;
                            }
                        }
                    }
                }
                return 0;
            }
        "#);
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!("SKIP Unit byte-view call execution: Linux/macOS cc hosts supported; Windows runtime route unavailable");
}
