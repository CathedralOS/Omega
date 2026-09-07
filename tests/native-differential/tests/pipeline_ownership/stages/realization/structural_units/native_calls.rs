//! Native ABI observation is separate from immutable admitted publication.

use crate::tests::{StagedOptimizedRelocationFreeObjectContainer, native_execution::Code};

pub(super) fn check_owned_pair(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    published: &image_emission::ObjectArtifact,
) {
    let first_expected = [0x0123_4567_89ab_cdef, 0x1357_2468];
    let second_expected = [0xfedc_ba98_7654_3210, 0x2468_1357];
    let mut first = first_expected;
    let mut second = second_expected;
    let entry = published.entry_function().text_offset;

    // Execute the exact independently validated publication, with a Unit FFI
    // result. RAX is not an observable result of this contract.
    let code = Code::new(published.text_bytes());
    code.call_unit_indirect_pair(entry, &mut first, &mut second);
    assert_eq!(first, first_expected);
    assert_eq!(second, second_expected);

    // A no-op callee cannot observe whether the caller copied its arguments.
    // Redirect only this separate test byte vector to a logging callee that
    // overwrites both received referents. Never submit it as a valid artifact.
    let text = source.source().text_section();
    let [call] = text.resolved_internal_machine_calls.as_slice() else {
        panic!("the fixture has exactly one internal structural call");
    };
    let mut observed = [0_u64; 6];
    let mut stress = published.text_bytes().to_vec();
    let destination = stress.len();
    stress.extend(logging_callee(observed.as_mut_ptr() as usize));
    let field = usize::try_from(call.field_section_offset).unwrap();
    let next = i64::try_from(call.next_instruction_section_offset).unwrap();
    let displacement = i32::try_from(i64::try_from(destination).unwrap() - next).unwrap();
    stress[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
    let first_address = first.as_mut_ptr() as usize as u64;
    let second_address = second.as_mut_ptr() as usize as u64;
    Code::new(&stress).call_unit_indirect_pair(entry, &mut first, &mut second);
    assert_ne!(
        observed[0], first_address,
        "first owned argument needs its own ABI temporary"
    );
    assert_ne!(
        observed[1], second_address,
        "second owned argument needs its own ABI temporary"
    );
    assert_ne!(
        observed[0], observed[1],
        "the two owned transfers must not alias"
    );
    assert_eq!(&observed[2..4], &first_expected);
    assert_eq!(&observed[4..6], &second_expected);
    assert_eq!(first, first_expected);
    assert_eq!(second, second_expected);
}

fn logging_callee(observed: usize) -> Vec<u8> {
    // Microsoft x64 leaf: only volatile RAX/R11 are changed; RCX/RDX retain
    // distinct incoming pointers. Six writable log words remain live at call.
    let mut bytes = vec![0x49, 0xbb]; // mov r11, observed
    bytes.extend_from_slice(&(observed as u64).to_le_bytes());
    bytes.extend_from_slice(&[
        0x49, 0x89, 0x0b, // mov [r11], rcx
        0x49, 0x89, 0x53, 0x08, // mov [r11+8], rdx
        0x48, 0x8b, 0x01, 0x49, 0x89, 0x43, 0x10, // first.base
        0x48, 0x8b, 0x41, 0x08, 0x49, 0x89, 0x43, 0x18, // first.length
        0x48, 0x8b, 0x02, 0x49, 0x89, 0x43, 0x20, // second.base
        0x48, 0x8b, 0x42, 0x08, 0x49, 0x89, 0x43, 0x28, // second.length
        0x48, 0xc7, 0x01, 0, 0, 0, 0, // overwrite first.base
        0x48, 0xc7, 0x41, 0x08, 0, 0, 0, 0, // overwrite first.length
        0x48, 0xc7, 0x02, 0, 0, 0, 0, // overwrite second.base
        0x48, 0xc7, 0x42, 0x08, 0, 0, 0, 0, // overwrite second.length
        0xc3,
    ]);
    bytes
}
