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
    /// Return only a larger encoding issued for this instruction.
    pub fn next_wider(
        &self,
        current: &EncodedBranchAlternative,
    ) -> Option<&EncodedBranchAlternative> {
        let next = if self
            .shortened
            .as_ref()
            .is_some_and(|form| std::ptr::eq(form, current))
        {
            &self.baseline
        } else if std::ptr::eq(&self.baseline, current) {
            self.widened.as_ref()?
        } else {
            return None;
        };
        (next.bytes.len() > current.bytes.len()).then_some(next)
    }

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedControlIndex {
    rows: Vec<(
        SelectedInstructionId,
        std::sync::Arc<EncodedControlAlternatives>,
    )>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedControlIndexError {
    FunctionRosterMismatch,
    DuplicateInstruction(SelectedInstructionId),
}

impl EncodedControlIndex {
    pub fn for_layout(
        encoding: &crate::SelectedFormEncoding,
        layout: &crate::ResolvedMachineLayout,
    ) -> Result<Vec<Self>, EncodedControlIndexError> {
        Self::partition(
            encoding.rows(),
            layout.functions().iter().map(|function| {
                function
                    .blocks
                    .iter()
                    .map(|block| block.instructions.len())
                    .sum()
            }),
        )
    }

    /// Instruction IDs are local to their function. The encoding keeps the
    /// function roster and row counts even when layout changes block order.
    pub fn partition(
        rows: &[crate::SelectedFormEncodingRow],
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

    pub fn new(rows: &[crate::SelectedFormEncodingRow]) -> Result<Self, SelectedInstructionId> {
        let mut controls = rows
            .iter()
            .filter_map(|row| match &row.state {
                crate::SelectedFormEncodingState::DeferredControl { alternatives, .. } => {
                    Some((row.instruction, std::sync::Arc::clone(alternatives)))
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

    /// Replay reads the producer's rows in function order and checks every carried alternative.
    pub fn matches_function_rows(&self, rows: &[crate::SelectedFormEncodingRow]) -> bool {
        let mut controls = 0;
        for row in rows {
            if let crate::SelectedFormEncodingState::DeferredControl { alternatives, .. } =
                &row.state
            {
                controls += 1;
                if self.get(row.instruction).is_none_or(|stored| {
                    !std::ptr::eq(stored, alternatives.as_ref()) && stored != alternatives.as_ref()
                }) {
                    return false;
                }
            }
        }
        controls == self.rows.len()
    }

    pub fn get(&self, instruction: SelectedInstructionId) -> Option<&EncodedControlAlternatives> {
        self.rows
            .binary_search_by_key(&instruction, |(id, _)| *id)
            .ok()
            .map(|index| self.rows[index].1.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchPatchError {
    DisplacementOverflow,
    MisalignedTarget,
    FieldOutsideEncoding,
    NonzeroTemplateField,
}

impl BranchDisplacementField {
    fn geometry(self) -> (usize, u32, u32, i64) {
        match self {
            BranchDisplacementField::Relative8 => (1, 0, 8, 1),
            BranchDisplacementField::Relative32 => (4, 0, 32, 1),
            BranchDisplacementField::Aarch64Conditional19 => (4, 5, 19, 4),
            BranchDisplacementField::Aarch64Branch26 => (4, 0, 26, 4),
        }
    }
}

impl EncodedBranchAlternative {
    /// The encoder admitted the fixed bits and operands of this exact self. Read
    /// its signed displacement from the final instruction, including the second
    /// word of a widened ARM64 conditional. Bytes outside that field must match.
    pub fn read_target(
        &self,
        bytes: &[u8],
        instruction_offset: u64,
        architecture: target::Architecture,
    ) -> Option<u64> {
        use BranchDisplacementField as Field;
        use target::Architecture;
        let start = usize::try_from(self.field_offset).ok()?;
        let width = match self.field {
            Field::Relative8 => 1,
            _ => 4,
        };
        let end = start.checked_add(width)?;
        if bytes.len() != self.bytes.len()
            || bytes.get(..start)? != self.bytes.get(..start)?
            || bytes.get(end..)? != self.bytes.get(end..)?
        {
            return None;
        }
        let field = bytes.get(start..end)?;
        let template = self.bytes.get(start..end)?;
        let displacement = match (architecture, self.field) {
            (Architecture::X86_64, Field::Relative8) if template == [0] => {
                i64::from(i8::from_le_bytes(field.try_into().ok()?))
            }
            (Architecture::X86_64, Field::Relative32) if template == [0; 4] => {
                i64::from(i32::from_le_bytes(field.try_into().ok()?))
            }
            (Architecture::Aarch64, Field::Aarch64Conditional19 | Field::Aarch64Branch26) => {
                let word = u32::from_le_bytes(field.try_into().ok()?);
                let fixed = u32::from_le_bytes(template.try_into().ok()?);
                let (mask, shift, bits) = match self.field {
                    Field::Aarch64Conditional19 => (0x00ff_ffe0, 5, 19),
                    Field::Aarch64Branch26 => (0x03ff_ffff, 0, 26),
                    _ => return None,
                };
                if word & !mask != fixed {
                    return None;
                }
                let immediate = i64::from((word & mask) >> shift);
                let sign = 1_i64 << (bits - 1);
                ((immediate ^ sign) - sign) * 4
            }
            _ => return None,
        };
        let reference = instruction_offset.checked_add(u64::from(self.reference_offset))?;
        reference.checked_add_signed(displacement)
    }

    fn field_value(&self, displacement: i64) -> Result<i64, BranchPatchError> {
        let delta = displacement
            .checked_sub(i64::from(self.reference_offset))
            .ok_or(BranchPatchError::DisplacementOverflow)?;
        let (_, _, bits, scale) = self.field.geometry();
        if delta % scale != 0 {
            return Err(BranchPatchError::MisalignedTarget);
        }
        let value = delta / scale;
        let limit = 1_i64 << (bits - 1);
        if !(-limit..limit).contains(&value) {
            return Err(BranchPatchError::DisplacementOverflow);
        }
        Ok(value)
    }

    /// Check the issued field’s reach and alignment without copying its bytes.
    pub fn reaches(&self, displacement: i64) -> Result<bool, BranchPatchError> {
        match self.field_value(displacement) {
            Ok(_) => Ok(true),
            Err(BranchPatchError::DisplacementOverflow) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// `displacement` is target minus instruction start, for every architecture.
    pub fn patch(&self, displacement: i64) -> Result<Vec<u8>, BranchPatchError> {
        let (width, shift, bits, _) = self.field.geometry();
        let value = self.field_value(displacement)?;
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
            assert_eq!(row.reaches(minimum), Ok(true));
            assert_eq!(row.reaches(maximum), Ok(true));
            assert_eq!(row.reaches(minimum - scale), Ok(false));
            assert_eq!(row.reaches(maximum + scale), Ok(false));
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
                    row.reaches(reference + 1),
                    Err(BranchPatchError::MisalignedTarget)
                );
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
