//! Exact Linux AAPCS64 `u32` countdown in the canonical incoming `w0` home.

pub const AARCH64_RANKED_U32_COUNTDOWN_BYTE_COUNT: usize = 24;
pub const AARCH64_RANKED_U32_PREHEADER_BRANCH_OFFSET: usize = 0;
pub const AARCH64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT: usize = 4;
pub const AARCH64_RANKED_U32_HEADER_OFFSET: usize = 4;
pub const AARCH64_RANKED_U32_COMPARE_OFFSET: usize = 4;
pub const AARCH64_RANKED_U32_COMPARE_BYTE_COUNT: usize = 4;
pub const AARCH64_RANKED_U32_EXIT_BRANCH_OFFSET: usize = 8;
pub const AARCH64_RANKED_U32_EXIT_BRANCH_BYTE_COUNT: usize = 4;
pub const AARCH64_RANKED_U32_POSITIVE_PATH_OFFSET: usize = 12;
pub const AARCH64_RANKED_U32_DECREMENT_OFFSET: usize = 12;
pub const AARCH64_RANKED_U32_DECREMENT_BYTE_COUNT: usize = 4;
pub const AARCH64_RANKED_U32_BACKWARD_BRANCH_OFFSET: usize = 16;
pub const AARCH64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT: usize = 4;
pub const AARCH64_RANKED_U32_EXIT_OFFSET: usize = 20;
pub const AARCH64_RANKED_U32_RETURN_OFFSET: usize = 20;
pub const AARCH64_RANKED_U32_RETURN_BYTE_COUNT: usize = 4;

/// Emit one fixed-width, relocation-free countdown body.
///
/// Every arithmetic instruction uses the 32-bit register view so the loop
/// carrier remains exactly `u32`; the backward branch targets the header, not
/// the one-time preheader branch.
pub const fn encode_ranked_u32_countdown_in_w0() -> [u8; AARCH64_RANKED_U32_COUNTDOWN_BYTE_COUNT] {
    [
        0x01, 0x00, 0x00, 0x14, // b header
        0x1f, 0x00, 0x00, 0x71, // cmp w0, #0
        0x60, 0x00, 0x00, 0x54, // b.eq exit
        0x00, 0x04, 0x00, 0x51, // sub w0, w0, #1
        0xfd, 0xff, 0xff, 0x17, // b header
        0xc0, 0x03, 0x5f, 0xd6, // ret
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn sign_extend(value: u32, bits: u32) -> i64 {
        let shift = 64 - bits;
        ((i64::from(value) << shift) >> shift) * 4
    }

    #[test]
    fn ranked_countdown_branches_target_header_and_exit() {
        let bytes = encode_ranked_u32_countdown_in_w0();
        let preheader = sign_extend(word(&bytes, 0) & 0x03ff_ffff, 26);
        assert_eq!(preheader, AARCH64_RANKED_U32_HEADER_OFFSET as i64);
        let exit = sign_extend((word(&bytes, 8) >> 5) & 0x7ffff, 19);
        assert_eq!(8_i64 + exit, AARCH64_RANKED_U32_EXIT_OFFSET as i64);
        let backedge = sign_extend(word(&bytes, 16) & 0x03ff_ffff, 26);
        assert_eq!(16_i64 + backedge, AARCH64_RANKED_U32_HEADER_OFFSET as i64);
    }
}
