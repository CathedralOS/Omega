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

/// Target-owned byte layout recovered by independently validating the exact
/// ranked countdown encoding. Its private construction prevents unvalidated
/// bytes from acquiring layout evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64RankedU32CountdownLayout {
    _private: (),
}

impl Aarch64RankedU32CountdownLayout {
    pub const fn preheader_branch(self) -> (usize, usize) {
        (
            AARCH64_RANKED_U32_PREHEADER_BRANCH_OFFSET,
            AARCH64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT,
        )
    }

    pub const fn header_offset(self) -> usize {
        AARCH64_RANKED_U32_HEADER_OFFSET
    }

    pub const fn compare(self) -> (usize, usize) {
        (
            AARCH64_RANKED_U32_COMPARE_OFFSET,
            AARCH64_RANKED_U32_COMPARE_BYTE_COUNT,
        )
    }

    pub const fn positive_path_offset(self) -> usize {
        AARCH64_RANKED_U32_POSITIVE_PATH_OFFSET
    }

    pub const fn decrement(self) -> (usize, usize) {
        (
            AARCH64_RANKED_U32_DECREMENT_OFFSET,
            AARCH64_RANKED_U32_DECREMENT_BYTE_COUNT,
        )
    }

    pub const fn backward_branch(self) -> (usize, usize) {
        (
            AARCH64_RANKED_U32_BACKWARD_BRANCH_OFFSET,
            AARCH64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT,
        )
    }

    pub const fn exit_offset(self) -> usize {
        AARCH64_RANKED_U32_EXIT_OFFSET
    }

    pub const fn return_instruction(self) -> (usize, usize) {
        (
            AARCH64_RANKED_U32_RETURN_OFFSET,
            AARCH64_RANKED_U32_RETURN_BYTE_COUNT,
        )
    }
}

/// Opaque evidence that one byte slice is exactly the canonical Linux
/// AAPCS64 ranked `u32` countdown and that all three branches target their
/// required sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64RankedU32CountdownEncoding {
    bytes: [u8; AARCH64_RANKED_U32_COUNTDOWN_BYTE_COUNT],
    layout: Aarch64RankedU32CountdownLayout,
}

impl ValidatedAarch64RankedU32CountdownEncoding {
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn layout(&self) -> Aarch64RankedU32CountdownLayout {
        self.layout
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64RankedU32CountdownEncodingError {
    WrongByteCount,
    MalformedInstruction,
    PreheaderTargetMismatch,
    ExitTargetMismatch,
    BackedgeTargetMismatch,
}

impl std::fmt::Display for Aarch64RankedU32CountdownEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid AArch64 ranked u32 countdown: {self:?}")
    }
}

impl std::error::Error for Aarch64RankedU32CountdownEncodingError {}

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

/// Independently decode and validate the exact countdown. This deliberately
/// does not compare against [`encode_ranked_u32_countdown_in_w0`]: instruction
/// width, registers, immediates, condition, and branch targets are replayed
/// from the encoded words.
pub fn validate_aarch64_ranked_u32_countdown_in_w0(
    bytes: &[u8],
) -> Result<ValidatedAarch64RankedU32CountdownEncoding, Aarch64RankedU32CountdownEncodingError> {
    use Aarch64RankedU32CountdownEncodingError as Error;

    let bytes: &[u8; AARCH64_RANKED_U32_COUNTDOWN_BYTE_COUNT] =
        bytes.try_into().map_err(|_| Error::WrongByteCount)?;
    let word = |offset: usize| {
        u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("an AArch64 instruction is four bytes"),
        )
    };
    let preheader = word(0);
    let exit_branch = word(8);
    let backedge = word(16);
    if preheader & 0xfc00_0000 != 0x1400_0000
        || word(4) != 0x7100_001f
        || exit_branch & 0xff00_001f != 0x5400_0000
        || word(12) != 0x5100_0400
        || backedge & 0xfc00_0000 != 0x1400_0000
        || word(20) != 0xd65f_03c0
    {
        return Err(Error::MalformedInstruction);
    }
    if branch26_target(preheader, 0) != AARCH64_RANKED_U32_HEADER_OFFSET as i64 {
        return Err(Error::PreheaderTargetMismatch);
    }
    if conditional_branch19_target(exit_branch, 8) != AARCH64_RANKED_U32_EXIT_OFFSET as i64 {
        return Err(Error::ExitTargetMismatch);
    }
    if branch26_target(backedge, 16) != AARCH64_RANKED_U32_HEADER_OFFSET as i64 {
        return Err(Error::BackedgeTargetMismatch);
    }
    Ok(ValidatedAarch64RankedU32CountdownEncoding {
        bytes: *bytes,
        layout: Aarch64RankedU32CountdownLayout { _private: () },
    })
}

fn branch26_target(word: u32, instruction_offset: i64) -> i64 {
    let immediate = word & 0x03ff_ffff;
    let signed_words = ((immediate << 6) as i32) >> 6;
    instruction_offset + i64::from(signed_words) * 4
}

fn conditional_branch19_target(word: u32, instruction_offset: i64) -> i64 {
    let immediate = ((word >> 5) & 0x7ffff) as i32;
    let signed_words = (immediate << 13) >> 13;
    instruction_offset + i64::from(signed_words) * 4
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
        let validated = validate_aarch64_ranked_u32_countdown_in_w0(&bytes).unwrap();
        assert_eq!(validated.bytes(), bytes);
        assert_eq!(validated.layout().header_offset(), 4);
    }

    #[test]
    fn decoder_rejects_instruction_width_register_condition_and_immediate_mutations() {
        for (offset, mutated_word) in [
            (0, 0x9400_0001_u32),
            (4, 0x7100_003f_u32),
            (8, 0x5400_0061_u32),
            (12, 0x5100_0800_u32),
            (16, 0x97ff_fffd_u32),
            (20, 0xd65f_03a0_u32),
        ] {
            let mut bytes = encode_ranked_u32_countdown_in_w0();
            bytes[offset..offset + 4].copy_from_slice(&mutated_word.to_le_bytes());
            assert_eq!(
                validate_aarch64_ranked_u32_countdown_in_w0(&bytes),
                Err(Aarch64RankedU32CountdownEncodingError::MalformedInstruction)
            );
        }
    }

    #[test]
    fn decoder_rejects_each_wrong_branch_target() {
        let mut preheader = encode_ranked_u32_countdown_in_w0();
        preheader[0..4].copy_from_slice(&0x1400_0002_u32.to_le_bytes());
        assert_eq!(
            validate_aarch64_ranked_u32_countdown_in_w0(&preheader),
            Err(Aarch64RankedU32CountdownEncodingError::PreheaderTargetMismatch)
        );

        let mut exit = encode_ranked_u32_countdown_in_w0();
        exit[8..12].copy_from_slice(&0x5400_0040_u32.to_le_bytes());
        assert_eq!(
            validate_aarch64_ranked_u32_countdown_in_w0(&exit),
            Err(Aarch64RankedU32CountdownEncodingError::ExitTargetMismatch)
        );

        let mut backedge_to_preheader = encode_ranked_u32_countdown_in_w0();
        backedge_to_preheader[16..20].copy_from_slice(&0x17ff_fffc_u32.to_le_bytes());
        assert_eq!(
            validate_aarch64_ranked_u32_countdown_in_w0(&backedge_to_preheader),
            Err(Aarch64RankedU32CountdownEncodingError::BackedgeTargetMismatch)
        );
    }

    #[test]
    fn decoder_rejects_truncated_and_trailing_bytes() {
        let bytes = encode_ranked_u32_countdown_in_w0();
        assert_eq!(
            validate_aarch64_ranked_u32_countdown_in_w0(&bytes[..20]),
            Err(Aarch64RankedU32CountdownEncodingError::WrongByteCount)
        );
        let mut trailing = bytes.to_vec();
        trailing.extend_from_slice(&0xd503_201f_u32.to_le_bytes());
        assert_eq!(
            validate_aarch64_ranked_u32_countdown_in_w0(&trailing),
            Err(Aarch64RankedU32CountdownEncodingError::WrongByteCount)
        );
    }
}
