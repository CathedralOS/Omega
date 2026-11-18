//! Encoded control alternatives and their displacement fields. Layout chooses
//! a supplied alternative and patches its field; it cannot invent instructions.

use crate::SelectedFormDecodedFootprint;
use selected_instructions::{MachineAlternativeKey, SelectedInstructionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchDisplacementField {
    Relative8,
    Relative32,
    Aarch64Conditional19,
    Aarch64Branch26,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedBranchAlternative {
    pub alternative: MachineAlternativeKey,
    pub bytes: Vec<u8>,
    pub field: BranchDisplacementField,
    pub field_offset: u32,
    /// Position relative to the instruction start from which the field counts.
    pub reference_offset: u32,
    pub footprint: SelectedFormDecodedFootprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedControlAlternatives {
    pub baseline: EncodedBranchAlternative,
    pub widened: Option<EncodedBranchAlternative>,
    pub shortened: Option<EncodedBranchAlternative>,
}

impl EncodedControlAlternatives {
    pub fn matching(
        &self,
        alternative: MachineAlternativeKey,
        byte_count: usize,
    ) -> Option<&EncodedBranchAlternative> {
        let mut matches = std::iter::once(&self.baseline)
            .chain(self.widened.iter())
            .chain(self.shortened.iter())
            .filter(|row| row.alternative == alternative && row.bytes.len() == byte_count);
        let row = matches.next()?;
        matches.next().is_none().then_some(row)
    }
}

/// Index only the encoded control rows needed by layout, without allocating
/// storage for every instruction in the program.
pub struct EncodedControlIndex<'a> {
    rows: Vec<(SelectedInstructionId, &'a EncodedControlAlternatives)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedControlIndexError {
    FunctionRosterMismatch,
    DuplicateInstruction(SelectedInstructionId),
}

impl<'a> EncodedControlIndex<'a> {
    /// Instruction IDs are local to their function. The encoding keeps the
    /// function roster and row counts even when layout changes block order.
    pub fn partition(
        rows: &'a [crate::SelectedFormEncodingRow],
        counts: impl IntoIterator<Item = usize>,
    ) -> Result<Vec<Self>, EncodedControlIndexError> {
        let mut remainder = rows;
        let mut functions = Vec::new();
        for count in counts {
            if count > remainder.len() {
                return Err(EncodedControlIndexError::FunctionRosterMismatch);
            }
            let (current, rest) = remainder.split_at(count);
            functions
                .push(Self::new(current).map_err(EncodedControlIndexError::DuplicateInstruction)?);
            remainder = rest;
        }
        if !remainder.is_empty() {
            return Err(EncodedControlIndexError::FunctionRosterMismatch);
        }
        Ok(functions)
    }

    pub fn new(rows: &'a [crate::SelectedFormEncodingRow]) -> Result<Self, SelectedInstructionId> {
        let mut controls = rows
            .iter()
            .filter_map(|row| match &row.state {
                crate::SelectedFormEncodingState::DeferredControl { alternatives, .. } => {
                    Some((row.instruction, alternatives.as_ref()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        controls.sort_unstable_by_key(|(instruction, _)| *instruction);
        if let Some(pair) = controls.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(pair[0].0);
        }
        Ok(Self { rows: controls })
    }

    pub fn get(
        &self,
        instruction: SelectedInstructionId,
    ) -> Option<&'a EncodedControlAlternatives> {
        self.rows
            .binary_search_by_key(&instruction, |(id, _)| *id)
            .ok()
            .map(|index| self.rows[index].1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchPatchError {
    DisplacementOverflow,
    MisalignedTarget,
    FieldOutsideEncoding,
    NonzeroTemplateField,
}

impl EncodedBranchAlternative {
    /// `displacement` is target minus instruction start, for every architecture.
    pub fn patch(&self, displacement: i64) -> Result<Vec<u8>, BranchPatchError> {
        let delta = displacement
            .checked_sub(i64::from(self.reference_offset))
            .ok_or(BranchPatchError::DisplacementOverflow)?;
        let (width, shift, bits, scale) = match self.field {
            BranchDisplacementField::Relative8 => (1, 0, 8, 1),
            BranchDisplacementField::Relative32 => (4, 0, 32, 1),
            BranchDisplacementField::Aarch64Conditional19 => (4, 5, 19, 4),
            BranchDisplacementField::Aarch64Branch26 => (4, 0, 26, 4),
        };
        if delta % scale != 0 {
            return Err(BranchPatchError::MisalignedTarget);
        }
        let value = delta / scale;
        let limit = 1_i64 << (bits - 1);
        if !(-limit..limit).contains(&value) {
            return Err(BranchPatchError::DisplacementOverflow);
        }
        let start = usize::try_from(self.field_offset)
            .map_err(|_| BranchPatchError::FieldOutsideEncoding)?;
        let end = start
            .checked_add(width)
            .ok_or(BranchPatchError::FieldOutsideEncoding)?;
        let mut bytes = self.bytes.clone();
        let field = bytes
            .get_mut(start..end)
            .ok_or(BranchPatchError::FieldOutsideEncoding)?;
        let mut encoded = 0_u64;
        for (position, byte) in field.iter().enumerate() {
            encoded |= u64::from(*byte) << (position * 8);
        }
        let mask = ((1_u64 << bits) - 1) << shift;
        if encoded & mask != 0 {
            return Err(BranchPatchError::NonzeroTemplateField);
        }
        encoded |= ((value as u64) << shift) & mask;
        field.copy_from_slice(&encoded.to_le_bytes()[..width]);
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{MachineAlternativeFamily, MachineEncodedEffects};

    fn template(
        field: BranchDisplacementField,
        bytes: Vec<u8>,
        offset: u32,
        reference: u32,
    ) -> EncodedBranchAlternative {
        EncodedBranchAlternative {
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::Jump,
                variant: 0,
            },
            bytes,
            field,
            field_offset: offset,
            reference_offset: reference,
            footprint: SelectedFormDecodedFootprint {
                register_reads: vec![],
                register_writes: vec![],
                implicit_defs: vec![],
                implicit_clobbers: vec![],
                encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![]),
            },
        }
    }

    #[test]
    fn signed_displacement_limits_are_checked_after_reference_and_scale() {
        use BranchDisplacementField::*;
        for (row, bits, scale) in [
            (template(Relative8, vec![0x75, 0], 1, 2), 8, 1),
            (template(Relative32, vec![0xe9, 0, 0, 0, 0], 1, 5), 32, 1),
            (
                template(
                    Aarch64Conditional19,
                    0x54000001_u32.to_le_bytes().to_vec(),
                    0,
                    0,
                ),
                19,
                4,
            ),
            (
                template(Aarch64Branch26, 0x14000000_u32.to_le_bytes().to_vec(), 0, 4),
                26,
                4,
            ),
        ] {
            let reference = i64::from(row.reference_offset);
            let minimum = -(1_i64 << (bits - 1)) * scale + reference;
            let maximum = ((1_i64 << (bits - 1)) - 1) * scale + reference;
            assert!(row.patch(minimum).is_ok());
            assert!(row.patch(maximum).is_ok());
            assert_eq!(
                row.patch(minimum - scale),
                Err(BranchPatchError::DisplacementOverflow)
            );
            assert_eq!(
                row.patch(maximum + scale),
                Err(BranchPatchError::DisplacementOverflow)
            );
            if scale == 4 {
                assert_eq!(
                    row.patch(reference + 1),
                    Err(BranchPatchError::MisalignedTarget)
                );
            }
        }
    }

    #[test]
    fn patching_preserves_opcodes_and_uses_the_supplied_reference() {
        let short = template(BranchDisplacementField::Relative8, vec![0x75, 0], 1, 2);
        assert_eq!(short.patch(0).unwrap(), [0x75, 0xfe]);
        let conditional = template(
            BranchDisplacementField::Aarch64Conditional19,
            0x54000001_u32.to_le_bytes().to_vec(),
            0,
            0,
        );
        assert_eq!(conditional.patch(-4).unwrap(), 0x54ffffe1_u32.to_le_bytes());
        let mut words = 0x54000040_u32.to_le_bytes().to_vec();
        words.extend_from_slice(&0x14000000_u32.to_le_bytes());
        let widened = template(
            BranchDisplacementField::Aarch64Branch26,
            words.clone(),
            4,
            4,
        );
        let patched = widened.patch(12).unwrap();
        assert_eq!(&patched[..4], &words[..4]);
        assert_eq!(&patched[4..], &0x14000002_u32.to_le_bytes());
        assert_eq!(widened.bytes, words);
    }

    #[test]
    fn malformed_templates_and_arithmetic_overflow_are_refused() {
        let mut row = template(
            BranchDisplacementField::Relative32,
            vec![0xe9, 0, 0, 0, 0],
            1,
            5,
        );
        assert_eq!(
            row.patch(i64::MIN),
            Err(BranchPatchError::DisplacementOverflow)
        );
        row.bytes[1] = 1;
        assert_eq!(row.patch(5), Err(BranchPatchError::NonzeroTemplateField));
        row.bytes[1] = 0;
        row.field_offset = 2;
        assert_eq!(row.patch(5), Err(BranchPatchError::FieldOutsideEncoding));
    }
}
