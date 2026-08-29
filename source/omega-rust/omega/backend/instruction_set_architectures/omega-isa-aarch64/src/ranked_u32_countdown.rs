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

/// Function-local coordinates for the three ranked control instructions after
/// native-fuel charges have been inserted. Destination coordinates name the
/// first charge in the destination site group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64RankedU32CountdownRebasedBranchLayout {
    pub preheader_branch_offset: usize,
    pub header_charge_offset: usize,
    pub exit_branch_offset: usize,
    pub exit_charge_offset: usize,
    pub backward_branch_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64RankedU32CountdownRebasedBranches {
    preheader: [u8; 4],
    exit: [u8; 4],
    backward: [u8; 4],
}

impl Aarch64RankedU32CountdownRebasedBranches {
    pub const fn preheader(self) -> [u8; 4] {
        self.preheader
    }

    pub const fn exit(self) -> [u8; 4] {
        self.exit
    }

    pub const fn backward(self) -> [u8; 4] {
        self.backward
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64RankedU32CountdownBranchError {
    CoordinateOverflow,
    MisalignedCoordinate,
    DistanceOutOfRange,
    TruncatedInstruction,
    MalformedInstruction,
    TargetMismatch,
}

impl std::fmt::Display for Aarch64RankedU32CountdownBranchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid rebased AArch64 ranked u32 countdown branch: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64RankedU32CountdownBranchError {}

/// Encode only the three control words whose immediates move when hot charges
/// are interleaved with the ranked semantic body.
pub fn encode_aarch64_rebased_ranked_u32_countdown_branches(
    layout: Aarch64RankedU32CountdownRebasedBranchLayout,
) -> Result<Aarch64RankedU32CountdownRebasedBranches, Aarch64RankedU32CountdownBranchError> {
    let preheader =
        aarch64_unconditional_branch(layout.preheader_branch_offset, layout.header_charge_offset)?;
    let exit = aarch64_equal_branch(layout.exit_branch_offset, layout.exit_charge_offset)?;
    let backward =
        aarch64_unconditional_branch(layout.backward_branch_offset, layout.header_charge_offset)?;
    Ok(Aarch64RankedU32CountdownRebasedBranches {
        preheader,
        exit,
        backward,
    })
}

/// Independently decode the supplied metered branch words and prove that they
/// enter the required hot-charge groups. This does not call the fragment
/// encoder above.
pub fn validate_aarch64_rebased_ranked_u32_countdown_branches(
    bytes: &[u8],
    layout: Aarch64RankedU32CountdownRebasedBranchLayout,
) -> Result<(), Aarch64RankedU32CountdownBranchError> {
    let word = |offset: usize| {
        bytes
            .get(offset..offset.saturating_add(4))
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(u32::from_le_bytes)
            .ok_or(Aarch64RankedU32CountdownBranchError::TruncatedInstruction)
    };
    let preheader = word(layout.preheader_branch_offset)?;
    let exit = word(layout.exit_branch_offset)?;
    let backward = word(layout.backward_branch_offset)?;
    if preheader & 0xfc00_0000 != 0x1400_0000
        || exit & 0xff00_001f != 0x5400_0000
        || backward & 0xfc00_0000 != 0x1400_0000
    {
        return Err(Aarch64RankedU32CountdownBranchError::MalformedInstruction);
    }
    let preheader_offset = i64::try_from(layout.preheader_branch_offset)
        .map_err(|_| Aarch64RankedU32CountdownBranchError::CoordinateOverflow)?;
    let exit_offset = i64::try_from(layout.exit_branch_offset)
        .map_err(|_| Aarch64RankedU32CountdownBranchError::CoordinateOverflow)?;
    let backward_offset = i64::try_from(layout.backward_branch_offset)
        .map_err(|_| Aarch64RankedU32CountdownBranchError::CoordinateOverflow)?;
    if branch26_target(preheader, preheader_offset)
        != i64::try_from(layout.header_charge_offset)
            .map_err(|_| Aarch64RankedU32CountdownBranchError::CoordinateOverflow)?
        || conditional_branch19_target(exit, exit_offset)
            != i64::try_from(layout.exit_charge_offset)
                .map_err(|_| Aarch64RankedU32CountdownBranchError::CoordinateOverflow)?
        || branch26_target(backward, backward_offset)
            != i64::try_from(layout.header_charge_offset)
                .map_err(|_| Aarch64RankedU32CountdownBranchError::CoordinateOverflow)?
    {
        return Err(Aarch64RankedU32CountdownBranchError::TargetMismatch);
    }
    Ok(())
}

fn aarch64_unconditional_branch(
    origin: usize,
    target: usize,
) -> Result<[u8; 4], Aarch64RankedU32CountdownBranchError> {
    let words = aarch64_branch_words(origin, target)?;
    if !(-(1_i64 << 25)..(1_i64 << 25)).contains(&words) {
        return Err(Aarch64RankedU32CountdownBranchError::DistanceOutOfRange);
    }
    Ok((0x1400_0000 | ((words as u32) & 0x03ff_ffff)).to_le_bytes())
}

fn aarch64_equal_branch(
    origin: usize,
    target: usize,
) -> Result<[u8; 4], Aarch64RankedU32CountdownBranchError> {
    let words = aarch64_branch_words(origin, target)?;
    if !(-(1_i64 << 18)..(1_i64 << 18)).contains(&words) {
        return Err(Aarch64RankedU32CountdownBranchError::DistanceOutOfRange);
    }
    Ok((0x5400_0000 | (((words as u32) & 0x7ffff) << 5)).to_le_bytes())
}

fn aarch64_branch_words(
    origin: usize,
    target: usize,
) -> Result<i64, Aarch64RankedU32CountdownBranchError> {
    let distance = i128::try_from(target)
        .ok()
        .and_then(|target| {
            i128::try_from(origin)
                .ok()
                .and_then(|origin| target.checked_sub(origin))
        })
        .ok_or(Aarch64RankedU32CountdownBranchError::CoordinateOverflow)?;
    if distance % 4 != 0 {
        return Err(Aarch64RankedU32CountdownBranchError::MisalignedCoordinate);
    }
    i64::try_from(distance / 4)
        .map_err(|_| Aarch64RankedU32CountdownBranchError::DistanceOutOfRange)
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

    #[test]
    fn rebased_branch_fragments_target_charge_group_entrances() {
        let layout = Aarch64RankedU32CountdownRebasedBranchLayout {
            preheader_branch_offset: 36,
            header_charge_offset: 40,
            exit_branch_offset: 116,
            exit_charge_offset: 272,
            backward_branch_offset: 268,
        };
        let fragments = encode_aarch64_rebased_ranked_u32_countdown_branches(layout).unwrap();
        assert_eq!(u32::from_le_bytes(fragments.preheader()), 0x1400_0001);
        assert_eq!(u32::from_le_bytes(fragments.exit()), 0x5400_04e0);
        assert_eq!(u32::from_le_bytes(fragments.backward()), 0x17ff_ffc7);
        let mut bytes = vec![0_u8; 348];
        bytes[36..40].copy_from_slice(&fragments.preheader());
        bytes[116..120].copy_from_slice(&fragments.exit());
        bytes[268..272].copy_from_slice(&fragments.backward());
        validate_aarch64_rebased_ranked_u32_countdown_branches(&bytes, layout).unwrap();

        bytes[116] ^= 0x20;
        assert_eq!(
            validate_aarch64_rebased_ranked_u32_countdown_branches(&bytes, layout),
            Err(Aarch64RankedU32CountdownBranchError::TargetMismatch)
        );
        bytes[116] ^= 0x20;
        bytes[119] ^= 0x01;
        assert_eq!(
            validate_aarch64_rebased_ranked_u32_countdown_branches(&bytes, layout),
            Err(Aarch64RankedU32CountdownBranchError::MalformedInstruction)
        );
        assert_eq!(
            validate_aarch64_rebased_ranked_u32_countdown_branches(&bytes[..270], layout),
            Err(Aarch64RankedU32CountdownBranchError::TruncatedInstruction)
        );
    }

    #[test]
    fn rebased_branch_encoder_rejects_alignment_and_immediate_overflow() {
        assert_eq!(
            encode_aarch64_rebased_ranked_u32_countdown_branches(
                Aarch64RankedU32CountdownRebasedBranchLayout {
                    preheader_branch_offset: 0,
                    header_charge_offset: 2,
                    exit_branch_offset: 4,
                    exit_charge_offset: 8,
                    backward_branch_offset: 12,
                }
            ),
            Err(Aarch64RankedU32CountdownBranchError::MisalignedCoordinate)
        );
        assert_eq!(
            encode_aarch64_rebased_ranked_u32_countdown_branches(
                Aarch64RankedU32CountdownRebasedBranchLayout {
                    preheader_branch_offset: 0,
                    header_charge_offset: 4,
                    exit_branch_offset: 0,
                    exit_charge_offset: 1 << 21,
                    backward_branch_offset: 8,
                }
            ),
            Err(Aarch64RankedU32CountdownBranchError::DistanceOutOfRange)
        );
    }
}
