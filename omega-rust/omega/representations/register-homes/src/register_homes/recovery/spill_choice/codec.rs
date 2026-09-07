//! Spill-choice v2 framing and unchecked decoding.

use super::identity::encode_terminal_spill_choice_content;
use super::*;

const SPILL_CHOICE_MAGIC: &[u8; 8] = b"OMGSPC\0\0";
const SPILL_CHOICE_VERSION: u32 = 2;

impl SpillChoicePlan {
    /// Canonical transport only. Decoding does not grant recovery-victim or
    /// allocation authority; the independent validator must replay it against
    /// the retained validated roots and target register environment.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_spill_choice_content(self);
        let identity = crate::spill_choice_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(SPILL_CHOICE_MAGIC);
        encoded.extend_from_slice(&SPILL_CHOICE_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SpillChoiceDecodeError> {
        let mut cursor = SpillChoiceCursor::new(encoded);
        if cursor.take(SPILL_CHOICE_MAGIC.len())? != SPILL_CHOICE_MAGIC {
            return Err(SpillChoiceDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != SPILL_CHOICE_VERSION {
            return Err(SpillChoiceDecodeError::UnsupportedVersion(version));
        }
        let identity = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let policy = match cursor.byte()? {
            0 => SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            tag => return Err(SpillChoiceDecodeError::UnknownPolicy(tag)),
        };
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| SpillChoiceDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| SpillChoiceDecodeError::InvalidUsage)?;
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine)
                .ok_or(SpillChoiceDecodeError::InvalidMachineId(raw_machine))?;
            let choice = match cursor.byte()? {
                0 => None,
                1 => {
                    let block = SelectedBlockId(u32::from_le_bytes(cursor.array()?));
                    let point = LiveRangePoint(u32::from_le_bytes(cursor.array()?));
                    let incoming = VirtualRegisterId(u32::from_le_bytes(cursor.array()?));
                    let incoming_class = RegisterClassId(u16::from_le_bytes(cursor.array()?));
                    let candidate_count = cursor.length()?;
                    let mut incoming_common_candidates =
                        Vec::with_capacity(candidate_count.min(cursor.remaining()));
                    for _ in 0..candidate_count {
                        incoming_common_candidates
                            .push(RegisterViewId(u16::from_le_bytes(cursor.array()?)));
                    }
                    let resident_count = cursor.length()?;
                    let mut active_residents =
                        Vec::with_capacity(resident_count.min(cursor.remaining()));
                    for _ in 0..resident_count {
                        active_residents.push(PressureResident {
                            virtual_register: VirtualRegisterId(u32::from_le_bytes(
                                cursor.array()?,
                            )),
                            class: RegisterClassId(u16::from_le_bytes(cursor.array()?)),
                            start: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                            exclusive_end: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                            view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
                        });
                    }
                    let contender_count = cursor.length()?;
                    let mut contenders =
                        Vec::with_capacity(contender_count.min(cursor.remaining()));
                    for _ in 0..contender_count {
                        let virtual_register =
                            VirtualRegisterId(u32::from_le_bytes(cursor.array()?));
                        let exclusive_end = LiveRangePoint(u32::from_le_bytes(cursor.array()?));
                        let reclaimed_view = match cursor.byte()? {
                            0 => None,
                            1 => Some(RegisterViewId(u16::from_le_bytes(cursor.array()?))),
                            tag => return Err(SpillChoiceDecodeError::UnknownOption(tag)),
                        };
                        contenders.push(PressureContender {
                            virtual_register,
                            exclusive_end,
                            reclaimed_view,
                        });
                    }
                    let selected_victim = VirtualRegisterId(u32::from_le_bytes(cursor.array()?));
                    Some(SpillChoice {
                        block,
                        point,
                        incoming,
                        incoming_class,
                        incoming_common_candidates,
                        active_residents,
                        contenders,
                        selected_victim,
                    })
                }
                tag => return Err(SpillChoiceDecodeError::UnknownOption(tag)),
            };
            functions.push(FunctionSpillChoices { machine, choice });
        }
        if cursor.remaining() != 0 {
            return Err(SpillChoiceDecodeError::TrailingBytes);
        }
        let plan = Self {
            legality,
            ranges,
            register_environment,
            allocator_availability,
            policy,
            budget,
            usage,
            functions,
        };
        if crate::spill_choice_identity(&plan) != identity {
            return Err(SpillChoiceDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

struct SpillChoiceCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> SpillChoiceCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }
    fn take(&mut self, length: usize) -> Result<&'encoded [u8], SpillChoiceDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(SpillChoiceDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(SpillChoiceDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], SpillChoiceDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| SpillChoiceDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, SpillChoiceDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, SpillChoiceDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| SpillChoiceDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
