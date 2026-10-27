//! Spill-choice v2 framing and unchecked decoding.

use super::identity::encode_terminal_spill_choice_content;
use super::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionSpillChoices,
    LiveRangeIdentity, LiveRangePoint, MachineId, OptimizationWorkBudget, OptimizationWorkUsage,
    PressureContender, PressureResident, RegisterClassId, RegisterViewId, SelectedBlockId,
    SpillChoice, SpillChoiceDecodeError, SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy,
    TargetRegisterEnvironmentIdentity, VirtualRegisterId,
};
use crate::codec_bytes::Envelope;

const SPILL_CHOICE_ENVELOPE: Envelope = Envelope::new(b"OMGSPC\0\0", 2);

impl SpillChoicePlan {
    /// Canonical transport only. Decoding does not grant recovery-victim or
    /// allocation authority; the independent validator must replay it against
    /// the retained validated roots and target register environment.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_spill_choice_content(self);
        let identity = crate::spill_choice_identity(self);
        SPILL_CHOICE_ENVELOPE.frame(identity.bytes(), &content)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SpillChoiceDecodeError> {
        let (mut cursor, identity) = SPILL_CHOICE_ENVELOPE.open(
            encoded,
            || SpillChoiceDecodeError::WrongMagic,
            SpillChoiceDecodeError::UnsupportedVersion,
        )?;
        let identity = SpillChoiceIdentity::from_bytes(identity);
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
            let raw_machine = cursor.u64()?;
            let machine = MachineId::new(raw_machine)
                .ok_or(SpillChoiceDecodeError::InvalidMachineId(raw_machine))?;
            let choice = match cursor.byte()? {
                0 => None,
                1 => {
                    let block = SelectedBlockId(cursor.u32()?);
                    let point = LiveRangePoint(cursor.u32()?);
                    let incoming = VirtualRegisterId(cursor.u32()?);
                    let incoming_class = RegisterClassId(cursor.u16()?);
                    let incoming_common_candidates =
                        crate::codec_bytes::decode_list::<_, SpillChoiceDecodeError>(
                            &mut cursor,
                            |cursor| Ok(RegisterViewId(cursor.u16()?)),
                        )?;
                    let resident_count = cursor.length()?;
                    let mut active_residents =
                        Vec::with_capacity(resident_count.min(cursor.remaining()));
                    for _ in 0..resident_count {
                        active_residents.push(PressureResident {
                            virtual_register: VirtualRegisterId(cursor.u32()?),
                            class: RegisterClassId(cursor.u16()?),
                            start: LiveRangePoint(cursor.u32()?),
                            exclusive_end: LiveRangePoint(cursor.u32()?),
                            view: RegisterViewId(cursor.u16()?),
                        });
                    }
                    let contender_count = cursor.length()?;
                    let mut contenders =
                        Vec::with_capacity(contender_count.min(cursor.remaining()));
                    for _ in 0..contender_count {
                        let virtual_register = VirtualRegisterId(cursor.u32()?);
                        let exclusive_end = LiveRangePoint(cursor.u32()?);
                        let reclaimed_view = match cursor.byte()? {
                            0 => None,
                            1 => Some(RegisterViewId(cursor.u16()?)),
                            tag => return Err(SpillChoiceDecodeError::UnknownOption(tag)),
                        };
                        contenders.push(PressureContender {
                            virtual_register,
                            exclusive_end,
                            reclaimed_view,
                        });
                    }
                    let selected_victim = VirtualRegisterId(cursor.u32()?);
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
