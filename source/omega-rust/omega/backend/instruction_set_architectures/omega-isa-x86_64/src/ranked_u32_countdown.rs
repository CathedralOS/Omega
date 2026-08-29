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

/// Target-owned byte layout recovered by independently validating the exact
/// ranked countdown encoding. Its private construction prevents unvalidated
/// bytes from acquiring layout evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64RankedU32CountdownLayout {
    _private: (),
}

impl X86_64RankedU32CountdownLayout {
    pub const fn preheader_branch(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_PREHEADER_BRANCH_OFFSET,
            X86_64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT,
        )
    }

    pub const fn header_offset(self) -> usize {
        X86_64_RANKED_U32_HEADER_OFFSET
    }

    pub const fn compare(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_COMPARE_OFFSET,
            X86_64_RANKED_U32_COMPARE_BYTE_COUNT,
        )
    }

    pub const fn positive_path_offset(self) -> usize {
        X86_64_RANKED_U32_POSITIVE_PATH_OFFSET
    }

    pub const fn decrement(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_DECREMENT_OFFSET,
            X86_64_RANKED_U32_DECREMENT_BYTE_COUNT,
        )
    }

    pub const fn backward_branch(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_BACKWARD_BRANCH_OFFSET,
            X86_64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT,
        )
    }

    pub const fn exit_offset(self) -> usize {
        X86_64_RANKED_U32_EXIT_OFFSET
    }

    pub const fn return_instruction(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_RETURN_OFFSET,
            X86_64_RANKED_U32_RETURN_BYTE_COUNT,
        )
    }
}

/// Opaque evidence that one byte slice is exactly the canonical Linux
/// System-V ranked `u32` countdown and that all three branches target their
/// required sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64RankedU32CountdownEncoding {
    bytes: [u8; X86_64_RANKED_U32_COUNTDOWN_BYTE_COUNT],
    layout: X86_64RankedU32CountdownLayout,
}

impl ValidatedX86_64RankedU32CountdownEncoding {
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn layout(&self) -> X86_64RankedU32CountdownLayout {
        self.layout
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64RankedU32CountdownEncodingError {
    WrongByteCount,
    MalformedInstruction,
    PreheaderTargetMismatch,
    ExitTargetMismatch,
    BackedgeTargetMismatch,
}

impl std::fmt::Display for X86_64RankedU32CountdownEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid x86-64 ranked u32 countdown: {self:?}")
    }
}

impl std::error::Error for X86_64RankedU32CountdownEncodingError {}

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

/// Independently decode and validate the exact countdown. This deliberately
/// does not compare against [`encode_ranked_u32_countdown_in_edi`]: opcode,
/// register, width, and branch-target checks are replayed from the bytes.
pub fn validate_x86_64_ranked_u32_countdown_in_edi(
    bytes: &[u8],
) -> Result<ValidatedX86_64RankedU32CountdownEncoding, X86_64RankedU32CountdownEncodingError> {
    use X86_64RankedU32CountdownEncodingError as Error;

    let bytes: &[u8; X86_64_RANKED_U32_COUNTDOWN_BYTE_COUNT] =
        bytes.try_into().map_err(|_| Error::WrongByteCount)?;
    if bytes[0] != 0xe9
        || bytes[5..7] != [0x85, 0xff]
        || bytes[7..9] != [0x0f, 0x84]
        || bytes[13..15] != [0xff, 0xcf]
        || bytes[15] != 0xe9
        || bytes[20] != 0xc3
    {
        return Err(Error::MalformedInstruction);
    }
    let relative_target = |field: std::ops::Range<usize>, next_instruction: i64| {
        let displacement = i32::from_le_bytes(
            bytes[field]
                .try_into()
                .expect("a ranked x86-64 rel32 field is four bytes"),
        );
        next_instruction + i64::from(displacement)
    };
    if relative_target(1..5, 5) != X86_64_RANKED_U32_HEADER_OFFSET as i64 {
        return Err(Error::PreheaderTargetMismatch);
    }
    if relative_target(9..13, 13) != X86_64_RANKED_U32_EXIT_OFFSET as i64 {
        return Err(Error::ExitTargetMismatch);
    }
    if relative_target(16..20, 20) != X86_64_RANKED_U32_HEADER_OFFSET as i64 {
        return Err(Error::BackedgeTargetMismatch);
    }
    Ok(ValidatedX86_64RankedU32CountdownEncoding {
        bytes: *bytes,
        layout: X86_64RankedU32CountdownLayout { _private: () },
    })
}

/// Function-local coordinates for the three ranked control instructions after
/// native-fuel charges have been inserted. Branch targets name the first hot
/// charge at the destination site, not the relocated semantic instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64RankedU32CountdownRebasedBranchLayout {
    pub preheader_branch_offset: usize,
    pub header_charge_offset: usize,
    pub exit_branch_offset: usize,
    pub exit_charge_offset: usize,
    pub backward_branch_offset: usize,
}

/// Target-owned branch fragments for the charge-interleaved countdown body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64RankedU32CountdownRebasedBranches {
    preheader: [u8; 5],
    exit: [u8; 6],
    backward: [u8; 5],
}

impl X86_64RankedU32CountdownRebasedBranches {
    pub const fn preheader(self) -> [u8; 5] {
        self.preheader
    }

    pub const fn exit(self) -> [u8; 6] {
        self.exit
    }

    pub const fn backward(self) -> [u8; 5] {
        self.backward
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64RankedU32CountdownBranchError {
    CoordinateOverflow,
    DistanceOutOfRange,
    TruncatedInstruction,
    MalformedInstruction,
    TargetMismatch,
}

impl std::fmt::Display for X86_64RankedU32CountdownBranchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid rebased x86-64 ranked u32 countdown branch: {self:?}"
        )
    }
}

impl std::error::Error for X86_64RankedU32CountdownBranchError {}

/// Encode only the three control fragments whose rel32 fields move when hot
/// charges are interleaved with the otherwise immutable semantic body.
pub fn encode_x86_64_rebased_ranked_u32_countdown_branches(
    layout: X86_64RankedU32CountdownRebasedBranchLayout,
) -> Result<X86_64RankedU32CountdownRebasedBranches, X86_64RankedU32CountdownBranchError> {
    let preheader_end = layout
        .preheader_branch_offset
        .checked_add(5)
        .ok_or(X86_64RankedU32CountdownBranchError::CoordinateOverflow)?;
    let exit_end = layout
        .exit_branch_offset
        .checked_add(6)
        .ok_or(X86_64RankedU32CountdownBranchError::CoordinateOverflow)?;
    let backward_end = layout
        .backward_branch_offset
        .checked_add(5)
        .ok_or(X86_64RankedU32CountdownBranchError::CoordinateOverflow)?;
    let mut preheader = [0_u8; 5];
    preheader[0] = 0xe9;
    preheader[1..].copy_from_slice(&x86_rel32(layout.header_charge_offset, preheader_end)?);
    let mut exit = [0_u8; 6];
    exit[..2].copy_from_slice(&[0x0f, 0x84]);
    exit[2..].copy_from_slice(&x86_rel32(layout.exit_charge_offset, exit_end)?);
    let mut backward = [0_u8; 5];
    backward[0] = 0xe9;
    backward[1..].copy_from_slice(&x86_rel32(layout.header_charge_offset, backward_end)?);
    Ok(X86_64RankedU32CountdownRebasedBranches {
        preheader,
        exit,
        backward,
    })
}

/// Independently decode the three supplied metered branches and prove that
/// they enter the required hot-charge groups. This does not call the fragment
/// encoder above.
pub fn validate_x86_64_rebased_ranked_u32_countdown_branches(
    bytes: &[u8],
    layout: X86_64RankedU32CountdownRebasedBranchLayout,
) -> Result<(), X86_64RankedU32CountdownBranchError> {
    let preheader = bytes
        .get(layout.preheader_branch_offset..layout.preheader_branch_offset.saturating_add(5))
        .ok_or(X86_64RankedU32CountdownBranchError::TruncatedInstruction)?;
    let exit = bytes
        .get(layout.exit_branch_offset..layout.exit_branch_offset.saturating_add(6))
        .ok_or(X86_64RankedU32CountdownBranchError::TruncatedInstruction)?;
    let backward = bytes
        .get(layout.backward_branch_offset..layout.backward_branch_offset.saturating_add(5))
        .ok_or(X86_64RankedU32CountdownBranchError::TruncatedInstruction)?;
    if preheader[0] != 0xe9 || exit[..2] != [0x0f, 0x84] || backward[0] != 0xe9 {
        return Err(X86_64RankedU32CountdownBranchError::MalformedInstruction);
    }
    let decoded_target = |instruction: &[u8], field_start: usize, instruction_end: usize| {
        let displacement = i32::from_le_bytes(
            instruction[field_start..field_start + 4]
                .try_into()
                .expect("validated x86-64 rel32 field has four bytes"),
        );
        i128::try_from(instruction_end)
            .ok()
            .and_then(|end| end.checked_add(i128::from(displacement)))
            .and_then(|target| usize::try_from(target).ok())
    };
    if decoded_target(
        preheader,
        1,
        layout.preheader_branch_offset.saturating_add(5),
    ) != Some(layout.header_charge_offset)
        || decoded_target(exit, 2, layout.exit_branch_offset.saturating_add(6))
            != Some(layout.exit_charge_offset)
        || decoded_target(backward, 1, layout.backward_branch_offset.saturating_add(5))
            != Some(layout.header_charge_offset)
    {
        return Err(X86_64RankedU32CountdownBranchError::TargetMismatch);
    }
    Ok(())
}

fn x86_rel32(
    target: usize,
    instruction_end: usize,
) -> Result<[u8; 4], X86_64RankedU32CountdownBranchError> {
    let distance = i128::try_from(target)
        .ok()
        .and_then(|target| {
            i128::try_from(instruction_end)
                .ok()
                .and_then(|end| target.checked_sub(end))
        })
        .ok_or(X86_64RankedU32CountdownBranchError::CoordinateOverflow)?;
    i32::try_from(distance)
        .map(i32::to_le_bytes)
        .map_err(|_| X86_64RankedU32CountdownBranchError::DistanceOutOfRange)
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
        let validated = validate_x86_64_ranked_u32_countdown_in_edi(&bytes).unwrap();
        assert_eq!(validated.bytes(), bytes);
        assert_eq!(validated.layout().header_offset(), 5);
    }

    #[test]
    fn decoder_rejects_instruction_and_register_mutations() {
        for index in [0, 5, 6, 7, 8, 13, 14, 15, 20] {
            let mut bytes = encode_ranked_u32_countdown_in_edi();
            bytes[index] ^= 1;
            assert_eq!(
                validate_x86_64_ranked_u32_countdown_in_edi(&bytes),
                Err(X86_64RankedU32CountdownEncodingError::MalformedInstruction)
            );
        }
    }

    #[test]
    fn decoder_rejects_each_wrong_branch_target() {
        let mut preheader = encode_ranked_u32_countdown_in_edi();
        preheader[1..5].copy_from_slice(&1_i32.to_le_bytes());
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&preheader),
            Err(X86_64RankedU32CountdownEncodingError::PreheaderTargetMismatch)
        );

        let mut exit = encode_ranked_u32_countdown_in_edi();
        exit[9..13].copy_from_slice(&6_i32.to_le_bytes());
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&exit),
            Err(X86_64RankedU32CountdownEncodingError::ExitTargetMismatch)
        );

        let mut backedge_to_preheader = encode_ranked_u32_countdown_in_edi();
        backedge_to_preheader[16..20].copy_from_slice(&(-20_i32).to_le_bytes());
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&backedge_to_preheader),
            Err(X86_64RankedU32CountdownEncodingError::BackedgeTargetMismatch)
        );
    }

    #[test]
    fn decoder_rejects_truncated_and_trailing_bytes() {
        let bytes = encode_ranked_u32_countdown_in_edi();
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&bytes[..20]),
            Err(X86_64RankedU32CountdownEncodingError::WrongByteCount)
        );
        let mut trailing = bytes.to_vec();
        trailing.push(0x90);
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&trailing),
            Err(X86_64RankedU32CountdownEncodingError::WrongByteCount)
        );
    }

    #[test]
    fn rebased_branch_fragments_target_charge_group_entrances() {
        let layout = X86_64RankedU32CountdownRebasedBranchLayout {
            preheader_branch_offset: 36,
            header_charge_offset: 41,
            exit_branch_offset: 115,
            exit_charge_offset: 272,
            backward_branch_offset: 267,
        };
        let fragments = encode_x86_64_rebased_ranked_u32_countdown_branches(layout).unwrap();
        assert_eq!(fragments.preheader(), [0xe9, 0, 0, 0, 0]);
        assert_eq!(&fragments.exit()[2..], &151_i32.to_le_bytes());
        assert_eq!(&fragments.backward()[1..], &(-231_i32).to_le_bytes());
        let mut bytes = vec![0_u8; 345];
        bytes[36..41].copy_from_slice(&fragments.preheader());
        bytes[115..121].copy_from_slice(&fragments.exit());
        bytes[267..272].copy_from_slice(&fragments.backward());
        validate_x86_64_rebased_ranked_u32_countdown_branches(&bytes, layout).unwrap();

        bytes[117] ^= 1;
        assert_eq!(
            validate_x86_64_rebased_ranked_u32_countdown_branches(&bytes, layout),
            Err(X86_64RankedU32CountdownBranchError::TargetMismatch)
        );
        bytes[117] ^= 1;
        bytes[115] ^= 1;
        assert_eq!(
            validate_x86_64_rebased_ranked_u32_countdown_branches(&bytes, layout),
            Err(X86_64RankedU32CountdownBranchError::MalformedInstruction)
        );
        assert_eq!(
            validate_x86_64_rebased_ranked_u32_countdown_branches(&bytes[..270], layout),
            Err(X86_64RankedU32CountdownBranchError::TruncatedInstruction)
        );
    }

    #[test]
    fn rebased_branch_encoder_rejects_rel32_overflow() {
        assert_eq!(
            encode_x86_64_rebased_ranked_u32_countdown_branches(
                X86_64RankedU32CountdownRebasedBranchLayout {
                    preheader_branch_offset: 0,
                    header_charge_offset: usize::MAX,
                    exit_branch_offset: 10,
                    exit_charge_offset: 20,
                    backward_branch_offset: 30,
                }
            ),
            Err(X86_64RankedU32CountdownBranchError::DistanceOutOfRange)
        );
    }
}
