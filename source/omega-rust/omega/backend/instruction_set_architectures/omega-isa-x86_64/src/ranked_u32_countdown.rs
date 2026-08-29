//! Exact Linux System-V `u32` countdown in the canonical incoming `edi` home.

pub const X86_64_RANKED_U32_COUNTDOWN_BYTE_COUNT: usize = 21;
pub const X86_64_RANKED_U32_PREHEADER_BRANCH_OFFSET: usize = 0;
pub const X86_64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT: usize = 5;
pub const X86_64_RANKED_U32_HEADER_OFFSET: usize = 5;
pub const X86_64_RANKED_U32_COMPARE_OFFSET: usize = 5;
pub const X86_64_RANKED_U32_COMPARE_BYTE_COUNT: usize = 2;
pub const X86_64_RANKED_U32_EXIT_BRANCH_OFFSET: usize = 7;
pub const X86_64_RANKED_U32_EXIT_BRANCH_BYTE_COUNT: usize = 6;
pub const X86_64_RANKED_U32_POSITIVE_PATH_OFFSET: usize = 13;
pub const X86_64_RANKED_U32_DECREMENT_OFFSET: usize = 13;
pub const X86_64_RANKED_U32_DECREMENT_BYTE_COUNT: usize = 2;
pub const X86_64_RANKED_U32_BACKWARD_BRANCH_OFFSET: usize = 15;
pub const X86_64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT: usize = 5;
pub const X86_64_RANKED_U32_EXIT_OFFSET: usize = 20;
pub const X86_64_RANKED_U32_RETURN_OFFSET: usize = 20;
pub const X86_64_RANKED_U32_RETURN_BYTE_COUNT: usize = 1;

/// Emit one fixed-width, relocation-free countdown body.
///
/// The entry branch makes the preheader a distinct executable site. The
/// backward branch targets the header at byte 5, so it cannot replay the
/// preheader site. Near branches are intentional: later fuel rebasing can
/// preserve one stable source encoding while changing their displacements.
pub const fn encode_ranked_u32_countdown_in_edi() -> [u8; X86_64_RANKED_U32_COUNTDOWN_BYTE_COUNT] {
    [
        0xe9, 0x00, 0x00, 0x00, 0x00, // jmp header
        0x85, 0xff, // test edi, edi
        0x0f, 0x84, 0x07, 0x00, 0x00, 0x00, // je exit
        0xff, 0xcf, // dec edi
        0xe9, 0xf1, 0xff, 0xff, 0xff, // jmp header
        0xc3, // ret
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranked_countdown_branches_target_header_and_exit() {
        let bytes = encode_ranked_u32_countdown_in_edi();
        let preheader = i32::from_le_bytes(bytes[1..5].try_into().unwrap());
        assert_eq!(
            5_i64 + i64::from(preheader),
            X86_64_RANKED_U32_HEADER_OFFSET as i64
        );
        let exit = i32::from_le_bytes(bytes[9..13].try_into().unwrap());
        assert_eq!(
            13_i64 + i64::from(exit),
            X86_64_RANKED_U32_EXIT_OFFSET as i64
        );
        let backedge = i32::from_le_bytes(bytes[16..20].try_into().unwrap());
        assert_eq!(
            20_i64 + i64::from(backedge),
            X86_64_RANKED_U32_HEADER_OFFSET as i64
        );
    }
}
