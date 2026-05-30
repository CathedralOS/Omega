use omega_encoded_machine::EncodedInstruction;
use omega_encoded_machine::EncodedMachine;
use omega_encoded_machine::EncodedMachineCode;
use omega_relocation_records::DispatchGuardRelocationInfo;
use omega_relocation_records::DispatchRelocationContext;
use omega_relocation_records::RelocationComputationInput;
use omega_relocation_records::RelocationRecord;
use omega_relocations::compute_relocations;

/// Encodes a representative dispatch machine mirroring the failing canary:
/// two dispatch guards separated by a SetDispatchState that carries a 4-byte index immediate.
///
/// The crux: dispatch *selection* assigns a `selected_instruction_index` to each guard's storage
/// load. Later, encoding interleaves a synthetic `SetDispatchState` (mov r12d, imm32 -> 41 bc ..)
/// into the machine stream. That synthetic instruction occupies an extra slot in the encoded
/// array but is NOT one of the selected instructions, so the encoded array position of guard #2's
/// storage load no longer equals its `selected_instruction_index`.
///
/// `selected_instruction_text_offset` must therefore locate the storage load by matching each
/// encoded instruction's own `selected_instruction_index`, not by array position. Matching by
/// array position lands guard #2's 8-byte Absolute64 storage address on the preceding
/// SetDispatchState's 4-byte index immediate (41 bc ..), spilling the high bytes past the field
/// and producing the `0xC0000005` access violation seen at runtime.
#[test]
fn second_dispatch_guard_anchors_at_its_own_storage_load() {
    // Encoded machine stream (array position -> selected_instruction_index | bytes):
    //   [0] sel 0  movabs r15, machine_storage        (49 bf ...)  10 bytes
    //   [1] sel 1  cmp scaffolding                     (48 39 f8)    3 bytes
    //   [2] sel 2  movabs r15, runtime_frame_storage   (49 bf ...)  10 bytes  <- guard #1 storage load
    //   [3] sel 4  SetDispatchState mov r12d, imm32    (41 bc ..)    6 bytes  <- synthetic, sel 4
    //   [4] sel 3  movabs r15, runtime_frame_storage   (49 bf ...)  10 bytes  <- guard #2 storage load, sel 3
    //   [5] sel 5  cmp r12d, imm32                      (41 81 fc ..) 7 bytes
    //
    // Note positions [3] and [4]: the synthetic SetDispatchState (sel 4) is encoded BEFORE
    // guard #2's storage load (sel 3). So array position 3 is NOT selected index 3; the guard's
    // load sits at array position 4. A position-based walk for index 3 wrongly returns position 3
    // (the SetDispatchState) at offset 23 (10+3+10); the correct field-based walk returns the
    // load at offset 29 (10+3+10+6).
    let instructions = vec![
        EncodedInstruction {
            machine_bytes: vec![0x49, 0xbf, 0, 0, 0, 0, 0, 0, 0, 0],
            selected_instruction_index: 0,
        },
        EncodedInstruction {
            machine_bytes: vec![0x48, 0x39, 0xf8],
            selected_instruction_index: 1,
        },
        EncodedInstruction {
            machine_bytes: vec![0x49, 0xbf, 0, 0, 0, 0, 0, 0, 0, 0],
            selected_instruction_index: 2,
        },
        EncodedInstruction {
            // synthetic SetDispatchState, encoded out of selected order
            machine_bytes: vec![0x41, 0xbc, 0x08, 0x30, 0x00, 0x40],
            selected_instruction_index: 4,
        },
        EncodedInstruction {
            // guard #2 storage load: selected index 3, but array position 4
            machine_bytes: vec![0x49, 0xbf, 0, 0, 0, 0, 0, 0, 0, 0],
            selected_instruction_index: 3,
        },
        EncodedInstruction {
            machine_bytes: vec![0x41, 0x81, 0xfc, 0, 0, 0, 0],
            selected_instruction_index: 5,
        },
    ];

    let encoded_machine = EncodedMachine {
        code: EncodedMachineCode { instructions },
    };

    // Guard metadata as produced by the dispatch lowering / selection stage.
    let dispatch = DispatchRelocationContext {
        dispatch_guards: vec![
            DispatchGuardRelocationInfo {
                // guard #1 storage load, selected index 2
                selected_instruction_index: 2,
                storage_data_identifier: "runtime_frame_storage".to_string(),
            },
            DispatchGuardRelocationInfo {
                // guard #2 storage load, selected index 3 (NOT the SetDispatchState)
                selected_instruction_index: 3,
                storage_data_identifier: "runtime_frame_storage".to_string(),
            },
        ],
    };

    let mut records: Vec<RelocationRecord> = Vec::new();
    let input = RelocationComputationInput {
        encoded_machine: &encoded_machine,
        dispatch: &dispatch,
    };

    compute_relocations(&input, &mut records);

    // Guard #1 storage load begins at text offset 13 (10 + 3).
    // Guard #2 storage load begins at text offset 29 (10 + 3 + 10 + 6).
    let expected_guard_one_offset = 13usize;
    let expected_guard_two_offset = 29usize;

    let mut offsets: Vec<usize> = records.iter().map(|r| r.text_offset).collect();
    offsets.sort_unstable();

    assert_eq!(
        offsets,
        vec![expected_guard_one_offset, expected_guard_two_offset],
        "dispatch guard relocations mis-anchored: guard #2 must land on its own movabs r15 \
         storage load (offset 29), not on the preceding SetDispatchState's index immediate \
         (offset 23, 41 bc ..)"
    );
}
