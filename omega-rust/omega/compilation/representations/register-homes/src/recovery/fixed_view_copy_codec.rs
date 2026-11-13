/*
We encode and decode the self-authenticating `FixedViewCopyPlan` artifact
(`OMGFCV\0\0`, version 99), binding the source selected-instruction, live-range,
and legality roots, the segment-home or legacy source evidence, the inserted
fixed-view copies, and the authenticated transformed `SelectedInstructionPlan`
payload.

Version 98 retains installed-entry forms and both input observation and result
lifetimes alongside provider application roles, typed descriptor-table
instructions, occupied-owner versus granted-backing separation, private callback
address placements, and formal-relative provider requirement paths. Decoding
verifies both the transformed payload's content digest and canonical re-encoding
before returning the unchecked plan.
*/

pub(super) use crate::codec_bytes::decode_list;
pub(super) use crate::cursor::Cursor;
pub(super) use legalized_operations::encode_work;

use ::selected_instructions::{
    LiveRangeIdentity, LiveRangePoint, LivenessPosition, SelectedBlockId, SelectedFunction,
    SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, VirtualFixedConstraintSite, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin, selected_instruction_plan_identity,
};
use optimization_core::AcceptedObligationFactIdentity;
use optimization_unit::{
    PsiProvenance, ValueDefinitionSite, WorkSettlement, encode_value_definition_site_identity,
};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    RegisterViewId, TargetRegisterEnvironmentIdentity, encode_constraint_key,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IeeeFloatFormat, IntegerCarrier, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, ScalarType, StructuralTypeId, ValueId,
    WorkScheduleIdentity,
};
use terminal_codec::CodecError;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::codec_bytes::Envelope;
use crate::digest_encoder::content_identity;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSegmentHomePlanIdentity, FixedPrecoloredSplitRequirementPlanIdentity,
    FixedViewCopy, FixedViewCopyDecodeError, FixedViewCopyDestination, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, fixed_view_copy_identity,
};

use super::selected_blocks::{decode_block, encode_block};
use super::structural_contracts::{decode_contracts, encode_contracts};

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
// Version 99 retains opaque callable carriers and exact descriptor references.
pub(super) const VERSION: u32 = 99;
pub(super) const ENVELOPE: Envelope = Envelope::new(MAGIC, VERSION);
const ENVELOPE_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v59\0";

impl FixedViewCopyPlan {
    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        let mut content = Vec::new();
        encode_v7(&mut content, self)?;
        Ok(ENVELOPE.frame(envelope_identity(self, &content), &content))
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        let (mut cursor, identity) = ENVELOPE.open(
            encoded,
            || FixedViewCopyDecodeError::WrongMagic,
            FixedViewCopyDecodeError::UnsupportedVersion,
        )?;
        let content_offset = cursor.offset;
        let decoded = decode_v7(&mut cursor)?;
        if cursor.remaining() != 0 {
            return Err(FixedViewCopyDecodeError::TrailingBytes);
        }
        if !decoded.transformed_payload_matches {
            return Err(FixedViewCopyDecodeError::TransformedPayloadMismatch);
        }
        if selected_instruction_plan_identity(&decoded.plan.transformed)
            != decoded.expected_transformed
        {
            return Err(FixedViewCopyDecodeError::TransformedIdentityMismatch);
        }
        if envelope_identity(&decoded.plan, &encoded[content_offset..]) != identity {
            return Err(FixedViewCopyDecodeError::IdentityMismatch);
        }
        Ok(decoded.plan)
    }
}

pub(super) fn envelope_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(ENVELOPE_DOMAIN);
    hasher.raw(&fixed_view_copy_identity(plan).bytes());
    hasher.raw(&content_identity(content));
    hasher.finish()
}

pub(super) struct DecodedContent {
    pub(super) plan: FixedViewCopyPlan,
    pub(super) expected_transformed: SelectedInstructionPlanIdentity,
    pub(super) transformed_payload_matches: bool,
}

fn encode_prefix(
    bytes: &mut Vec<u8>,
    plan: &FixedViewCopyPlan,
    transformed: SelectedInstructionPlanIdentity,
) {
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.source_ranges.bytes());
    bytes.extend_from_slice(&plan.source_legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.push(match plan.policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => 0,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => 1,
        FixedViewCopyPolicy::ImmediateBeforeFixedUseV1 => 2,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1 => 3,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(bytes, plan.copies.len());
    for copy in &plan.copies {
        encode_copy(bytes, copy);
    }
    bytes.extend_from_slice(&transformed.bytes());
}

pub(super) fn encode_v7(bytes: &mut Vec<u8>, plan: &FixedViewCopyPlan) -> Result<(), CodecError> {
    encode_prefix(
        bytes,
        plan,
        selected_instruction_plan_identity(&plan.transformed),
    );
    encode_source_evidence(bytes, plan.source_evidence);
    encode_selected_plan(bytes, &plan.transformed)
}

struct DecodedPrefix {
    source_selected: SelectedInstructionPlanIdentity,
    source_ranges: LiveRangeIdentity,
    source_legality: AllocationLegalityIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    allocator_availability: AllocatorAvailabilityIdentity,
    policy: FixedViewCopyPolicy,
    budget: optimization_core::OptimizationWorkBudget,
    usage: optimization_core::OptimizationWorkUsage,
    copies: Vec<FixedViewCopy>,
    expected_transformed: SelectedInstructionPlanIdentity,
}

fn decode_prefix(cursor: &mut Cursor<'_>) -> Result<DecodedPrefix, FixedViewCopyDecodeError> {
    let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let source_ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
    let source_legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
    let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
    let policy = match cursor.byte()? {
        0 => FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
        1 => FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        2 => FixedViewCopyPolicy::ImmediateBeforeFixedUseV1,
        3 => FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1,
        tag => return Err(FixedViewCopyDecodeError::UnknownPolicy(tag)),
    };
    let budget = optimization_core::OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidBudget)?;
    let usage = optimization_core::OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidUsage)?;
    let copies = decode_list(cursor, decode_copy)?;
    let expected_transformed = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    Ok(DecodedPrefix {
        source_selected,
        source_ranges,
        source_legality,
        register_environment,
        allocator_availability,
        policy,
        budget,
        usage,
        copies,
        expected_transformed,
    })
}

pub(super) fn decode_v7(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedContent, FixedViewCopyDecodeError> {
    let prefix = decode_prefix(cursor)?;
    let source_evidence = decode_source_evidence(cursor)?;
    let decoded = decode_selected_plan(cursor)?;
    Ok(DecodedContent {
        plan: FixedViewCopyPlan {
            source_selected: prefix.source_selected,
            source_ranges: prefix.source_ranges,
            source_legality: prefix.source_legality,
            register_environment: prefix.register_environment,
            allocator_availability: prefix.allocator_availability,
            source_evidence,
            policy: prefix.policy,
            budget: prefix.budget,
            usage: prefix.usage,
            copies: prefix.copies,
            transformed: decoded.plan.into(),
        },
        expected_transformed: prefix.expected_transformed,
        transformed_payload_matches: decoded.payload_matches,
    })
}

pub(crate) fn encode_source_evidence(bytes: &mut Vec<u8>, evidence: FixedViewCopySourceEvidence) {
    match evidence {
        FixedViewCopySourceEvidence::LegacyLegalityTransitionsV1 => bytes.push(0),
        FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
            fixed_intervals,
            split_requirements,
            segment_homes,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&fixed_intervals.bytes());
            bytes.extend_from_slice(&split_requirements.bytes());
            bytes.extend_from_slice(&segment_homes.bytes());
        }
    }
}

pub(super) fn decode_source_evidence(
    cursor: &mut Cursor<'_>,
) -> Result<FixedViewCopySourceEvidence, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(FixedViewCopySourceEvidence::LegacyLegalityTransitionsV1),
        1 => Ok(FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
            fixed_intervals: FixedPrecoloredIntervalPlanIdentity::from_bytes(cursor.array()?),
            split_requirements: FixedPrecoloredSplitRequirementPlanIdentity::from_bytes(
                cursor.array()?,
            ),
            segment_homes: FixedPrecoloredSegmentHomePlanIdentity::from_bytes(cursor.array()?),
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownSourceEvidence(tag)),
    }
}

pub(super) fn encode_copy(bytes: &mut Vec<u8>, copy: &FixedViewCopy) {
    crate::codec_bytes::push_encoded(bytes, copy.function.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.machine.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.source_virtual_register.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.source_value.get().to_le_bytes());
    encode_value_definition_site_identity(bytes, copy.source_definition_site);
    crate::codec_bytes::push_encoded(bytes, copy.from_view.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.to_view.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.insertion_block.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.before_instruction.0.to_le_bytes());
    length(bytes, copy.destinations.len());
    for destination in &copy.destinations {
        crate::encode_fixed_constraint_site_identity(bytes, destination.site);
        crate::codec_bytes::push_encoded(bytes, destination.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, destination.view.0.to_le_bytes());
    }
    crate::codec_bytes::push_encoded(bytes, copy.copy_instruction.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.result_virtual_register.0.to_le_bytes());
    encode_constraint_key(bytes, copy.copy_constraint);
}

pub(super) fn decode_copy(
    cursor: &mut Cursor<'_>,
) -> Result<FixedViewCopy, FixedViewCopyDecodeError> {
    let function = cursor.u32()?;
    let machine = decode_id(cursor, MachineId::new)?;
    let source_virtual_register = VirtualRegisterId(cursor.u32()?);
    let source_value = decode_id(cursor, ValueId::new)?;
    let source_definition_site = decode_definition_site(cursor)?;
    let from_view = RegisterViewId(cursor.u16()?);
    let to_view = RegisterViewId(cursor.u16()?);
    let insertion_block = SelectedBlockId(cursor.u32()?);
    let before_instruction = SelectedInstructionId(cursor.u32()?);
    let destinations = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(FixedViewCopyDestination {
            site: decode_fixed_site(cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            view: RegisterViewId(cursor.u16()?),
        })
    })?;
    Ok(FixedViewCopy {
        function,
        machine,
        source_virtual_register,
        source_value,
        source_definition_site,
        from_view,
        to_view,
        insertion_block,
        before_instruction,
        destinations,
        copy_instruction: SelectedInstructionId(cursor.u32()?),
        result_virtual_register: VirtualRegisterId(cursor.u32()?),
        copy_constraint: decode_constraint_key(cursor)?,
    })
}

pub(super) fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    crate::codec_bytes::option(bytes, value, |bytes, value| {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    });
}

pub(super) fn decode_option_u64(
    cursor: &mut Cursor<'_>,
) -> Result<Option<u64>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.u64()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

pub(super) fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    crate::codec_bytes::option(bytes, value, |bytes, value| {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    });
}

pub(super) fn decode_option_u16(
    cursor: &mut Cursor<'_>,
) -> Result<Option<u16>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.u16()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

pub(super) fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    encode_le_list(bytes, values, |value| value.to_le_bytes());
}

fn encode_le_list<T, const N: usize>(
    bytes: &mut Vec<u8>,
    values: impl ExactSizeIterator<Item = T>,
    to_le: impl Fn(T) -> [u8; N],
) {
    length(bytes, values.len());
    for value in values {
        crate::codec_bytes::push_encoded(bytes, to_le(value));
    }
}

pub(super) fn decode_ids<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<Vec<T>, FixedViewCopyDecodeError> {
    decode_list(cursor, |cursor| decode_id(cursor, constructor))
}

pub(super) fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    encode_le_list(bytes, values, |value| value.to_le_bytes());
}

pub(super) fn decode_u16s(cursor: &mut Cursor<'_>) -> Result<Vec<u16>, FixedViewCopyDecodeError> {
    decode_list(cursor, |cursor| Ok(cursor.u16()?))
}

pub(super) fn decode_id<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<T, FixedViewCopyDecodeError> {
    let raw = cursor.u64()?;
    constructor(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))
}

pub(super) fn decode_accepted_fact(
    cursor: &mut Cursor<'_>,
) -> Result<AcceptedObligationFactIdentity, FixedViewCopyDecodeError> {
    Ok(AcceptedObligationFactIdentity::from_bytes(cursor.array()?))
}

pub(super) fn length(bytes: &mut Vec<u8>, value: usize) {
    crate::codec_bytes::length(bytes, value, "fixed-view-copy artifact");
}

pub(super) fn decode_definition_site(
    cursor: &mut Cursor<'_>,
) -> Result<ValueDefinitionSite, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => ValueDefinitionSite::FunctionParameter(cursor.u32()?),
        1 => ValueDefinitionSite::BlockParameter {
            block: decode_id(cursor, BlockId::new)?,
            position: cursor.u32()?,
        },
        2 => ValueDefinitionSite::Node {
            block: decode_id(cursor, BlockId::new)?,
            node: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownDefinitionSite(tag)),
    })
}

pub(super) fn decode_fixed_site(
    cursor: &mut Cursor<'_>,
) -> Result<VirtualFixedConstraintSite, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => VirtualFixedConstraintSite::Entry,
        1 => VirtualFixedConstraintSite::Operand {
            position: LivenessPosition(cursor.u32()?),
            point: LiveRangePoint(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            operand: cursor.u16()?,
            access: decode_access(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownFixedSite(tag)),
    })
}

pub(super) fn encode_target(bytes: &mut Vec<u8>, target: target::NativeTarget) {
    crate::codec_bytes::encode_target(bytes, target);
}

pub(super) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<target::NativeTarget, FixedViewCopyDecodeError> {
    let architecture = match cursor.byte()? {
        0 => target::Architecture::X86_64,
        1 => target::Architecture::Aarch64,
        tag => return Err(FixedViewCopyDecodeError::UnknownArchitecture(tag)),
    };
    let object_format = match cursor.byte()? {
        0 => target::ObjectFormat::Elf,
        1 => target::ObjectFormat::MachO,
        2 => target::ObjectFormat::Coff,
        tag => return Err(FixedViewCopyDecodeError::UnknownObjectFormat(tag)),
    };
    Ok(target::NativeTarget {
        architecture,
        object_format,
        pointer_size: cursor.length()?,
        pointer_alignment: cursor.length()?,
    })
}

pub(super) fn encode_scalar(bytes: &mut Vec<u8>, scalar: ScalarType) {
    match scalar {
        ScalarType::Callable(contract) => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, contract.get().to_le_bytes());
        }
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                IntegerCarrier::Fixed => 0,
                IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            crate::codec_bytes::push_encoded(bytes, integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                IeeeFloatFormat::Binary32 => 0,
                IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

pub(super) fn decode_scalar(
    cursor: &mut Cursor<'_>,
) -> Result<ScalarType, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        3 => Ok(ScalarType::Callable(decode_id(
            cursor,
            semantic_vocabulary::CallableContractId::new,
        )?)),
        0 => Ok(ScalarType::Boolean),
        1 => {
            let carrier = cursor.byte()?;
            let sign = match cursor.byte()? {
                0 => IntegerSign::Signed,
                1 => IntegerSign::Unsigned,
                tag => return Err(FixedViewCopyDecodeError::UnknownIntegerSign(tag)),
            };
            let bits = cursor.u16()?;
            let integer = match carrier {
                0 => IntegerType::new(sign, bits),
                1 if sign == IntegerSign::Unsigned => IntegerType::address(bits),
                tag => return Err(FixedViewCopyDecodeError::UnknownIntegerCarrier(tag)),
            }
            .map_err(|_| FixedViewCopyDecodeError::InvalidIntegerType)?;
            Ok(ScalarType::Integer(integer))
        }
        2 => match cursor.byte()? {
            0 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
            1 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
            tag => Err(FixedViewCopyDecodeError::UnknownScalarType(tag)),
        },
        tag => Err(FixedViewCopyDecodeError::UnknownScalarType(tag)),
    }
}

pub(super) fn decode_integer(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(IntegerValue::Signed(cursor.i128()?)),
        1 => Ok(IntegerValue::Unsigned(cursor.u128()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownIntegerValue(tag)),
    }
}

pub(super) fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, FixedViewCopyDecodeError> {
    let family = match cursor.byte()? {
        0 => RegisterConstraintFamily::Call,
        1 => RegisterConstraintFamily::Return,
        2 => RegisterConstraintFamily::SystemCall,
        3 => RegisterConstraintFamily::InlineAssembly,
        4 => RegisterConstraintFamily::Instruction,
        tag => return Err(FixedViewCopyDecodeError::UnknownConstraintFamily(tag)),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}

pub(super) fn decode_access(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterOperandAccess, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(RegisterOperandAccess::Use),
        1 => Ok(RegisterOperandAccess::Def),
        2 => Ok(RegisterOperandAccess::UseDef),
        tag => Err(FixedViewCopyDecodeError::UnknownOperandAccess(tag)),
    }
}

pub(super) fn decode_bool(cursor: &mut Cursor<'_>) -> Result<bool, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(FixedViewCopyDecodeError::UnknownBoolean(tag)),
    }
}

pub(crate) struct DecodedSelectedPlan {
    pub(crate) plan: SelectedInstructionPlan,
    pub(crate) payload_matches: bool,
}

pub(super) fn encode_selected_plan(
    bytes: &mut Vec<u8>,
    plan: &SelectedInstructionPlan,
) -> Result<(), CodecError> {
    let payload = canonical_selected_plan_bytes(plan)?;
    bytes.extend_from_slice(&content_identity(&payload));
    length(bytes, payload.len());
    bytes.extend_from_slice(&payload);
    Ok(())
}

pub(super) fn decode_selected_plan(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedSelectedPlan, FixedViewCopyDecodeError> {
    let expected_digest: [u8; 32] = cursor.array()?;
    let payload_length = cursor.length()?;
    let payload = cursor.take(payload_length)?;
    let mut payload_cursor = Cursor::new(payload);
    let plan = decode_ordinary_plan(&mut payload_cursor)?;
    if payload_cursor.remaining() != 0 {
        return Err(FixedViewCopyDecodeError::TrailingBytes);
    }
    let digest_matches = content_identity(payload) == expected_digest;
    let canonical_matches = canonical_selected_plan_bytes(&plan)
        .map_err(|_| FixedViewCopyDecodeError::InvalidScalarRefinement)?
        .as_slice()
        == payload;
    Ok(DecodedSelectedPlan {
        plan,
        payload_matches: digest_matches && canonical_matches,
    })
}

fn canonical_selected_plan_bytes(plan: &SelectedInstructionPlan) -> Result<Vec<u8>, CodecError> {
    let mut payload = Vec::new();
    encode_ordinary_plan(&mut payload, plan)?;
    Ok(payload)
}

pub(super) fn encode_ordinary_plan(
    bytes: &mut Vec<u8>,
    plan: &SelectedInstructionPlan,
) -> Result<(), CodecError> {
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(bytes, plan.psi.vocabulary_marker.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, plan.work_schedule.marker().to_le_bytes());
    encode_target(bytes, plan.target);
    crate::codec_bytes::push_encoded(bytes, plan.entry.get().to_le_bytes());
    length(bytes, plan.functions.len());
    for function in &plan.functions {
        encode_function(bytes, function)?;
    }
    Ok(())
}

pub(super) fn decode_ordinary_plan(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionPlan, FixedViewCopyDecodeError> {
    let fingerprint = SemanticFingerprint::from_bytes(cursor.array()?);
    let marker_raw = cursor.u16()?;
    let marker = VocabularyMarker::new(marker_raw)
        .ok_or(FixedViewCopyDecodeError::InvalidVocabulary(marker_raw))?;
    let work_raw = cursor.u32()?;
    let work_schedule = WorkScheduleIdentity::new(work_raw)
        .ok_or(FixedViewCopyDecodeError::InvalidWorkSchedule(work_raw))?;
    let target = decode_target(cursor)?;
    let entry = decode_id(cursor, MachineId::new)?;
    let functions = decode_list(cursor, decode_function)?;
    Ok(SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: marker,
            program_fingerprint: fingerprint,
        },
        work_schedule,
        target,
        entry,
        functions: functions.into(),
    })
}

pub(super) fn encode_function(
    bytes: &mut Vec<u8>,
    function: &SelectedFunction,
) -> Result<(), CodecError> {
    crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
    encode_option_u64(bytes, function.attachment.map(|value| value.get()));
    encode_option_u64(
        bytes,
        function.nominal_cleanup_subject.map(|value| value.get()),
    );
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|value| value.get()),
    );
    crate::codec_bytes::push_encoded(bytes, function.entry_block.0.to_le_bytes());
    length(bytes, function.virtual_registers.len());
    for register in &function.virtual_registers {
        encode_register(bytes, register);
    }
    length(bytes, function.blocks.len());
    for block in &function.blocks {
        encode_block(bytes, block);
    }
    encode_contracts(bytes, function)
}

pub(super) fn decode_function(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedFunction, FixedViewCopyDecodeError> {
    let machine = decode_id(cursor, MachineId::new)?;
    let attachment = match decode_option_u64(cursor)? {
        None => None,
        Some(raw) => Some(
            StructuralTypeId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
        ),
    };
    let nominal_cleanup_subject = match decode_option_u64(cursor)? {
        None => None,
        Some(raw) => Some(
            StructuralTypeId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
        ),
    };
    let operations = decode_ids(cursor, OperationId::new)?;
    let edges = decode_ids(cursor, EdgeId::new)?;
    let entry_block = SelectedBlockId(cursor.u32()?);
    let virtual_registers = decode_list(cursor, decode_register)?;
    let blocks = decode_list(cursor, decode_block)?;
    let mut function = SelectedFunction {
        machine,
        attachment,
        nominal_cleanup_subject,
        provenance: target_operations::TerminalPsiProvenance { operations, edges },
        entry_block,
        virtual_registers,
        blocks: blocks.into(),
        structural: None,
        outgoing_arguments: Vec::new(),
        local_storage_slots: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
    };
    decode_contracts(cursor, &mut function)?;
    if !function.ids_are_positions() {
        return Err(FixedViewCopyDecodeError::IdentityNotPosition);
    }
    Ok(function)
}

pub(super) fn encode_register(bytes: &mut Vec<u8>, register: &VirtualRegister) {
    crate::codec_bytes::push_encoded(bytes, register.id.0.to_le_bytes());
    encode_scalar(bytes, register.scalar_type);
    crate::codec_bytes::push_encoded(bytes, register.class.0.to_le_bytes());
    register.origin.encode_identity(bytes);
    match register.definition_site {
        None => bytes.push(0),
        Some(site) => {
            bytes.push(1);
            encode_value_definition_site_identity(bytes, site);
        }
    }
    encode_option_u16(bytes, register.entry_fixed_view.map(|view| view.0));
}

pub(super) fn decode_register(
    cursor: &mut Cursor<'_>,
) -> Result<VirtualRegister, FixedViewCopyDecodeError> {
    let id = VirtualRegisterId(cursor.u32()?);
    let scalar_type = decode_scalar(cursor)?;
    let class = RegisterClassId(cursor.u16()?);
    let origin = match cursor.byte()? {
        8 => VirtualRegisterOrigin::StructuralObservation {
            instruction: SelectedInstructionId(cursor.u32()?),
            place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            byte_offset: cursor.u32()?,
        },
        9 => VirtualRegisterOrigin::InstructionScratch {
            instruction: SelectedInstructionId(cursor.u32()?),
            operand: cursor.u16()?,
        },
        7 => VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(cursor.u32()?),
            source_value: decode_id(cursor, ValueId::new)?,
        },
        6 => VirtualRegisterOrigin::SpillAddress {
            instruction: SelectedInstructionId(cursor.u32()?),
            register: VirtualRegisterId(cursor.u32()?),
        },
        4 => VirtualRegisterOrigin::StructuralParameter {
            place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            parameter_index: cursor.length()?,
        },
        5 => VirtualRegisterOrigin::AbiTransport {
            instruction: SelectedInstructionId(cursor.u32()?),
            place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            byte_offset: cursor.u32()?,
        },
        3 => VirtualRegisterOrigin::BlockParameter {
            source_value: decode_id(cursor, ValueId::new)?,
            block: SelectedBlockId(cursor.u32()?),
            parameter_index: cursor.length()?,
        },
        0 => VirtualRegisterOrigin::EntryParameter {
            source_value: decode_id(cursor, ValueId::new)?,
            parameter_index: cursor.length()?,
        },
        1 => VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(cursor.u32()?),
            source_value: decode_id(cursor, ValueId::new)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownRegisterOrigin(tag)),
    };
    Ok(VirtualRegister {
        id,
        scalar_type,
        class,
        origin,
        definition_site: crate::cursor::optional(
            cursor,
            decode_definition_site,
            FixedViewCopyDecodeError::UnknownOption,
        )?,
        entry_fixed_view: decode_option_u16(cursor)?.map(RegisterViewId),
    })
}

pub(super) fn encode_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
    encode_ids(bytes, provenance.operations.iter().map(|value| value.get()));
    encode_ids(bytes, provenance.values.iter().map(|value| value.get()));
    encode_ids(bytes, provenance.edges.iter().map(|value| value.get()));
    encode_ids(
        bytes,
        provenance.obligations.iter().map(|value| value.get()),
    );
    encode_work(bytes, &provenance.work);
}

pub(super) fn decode_provenance(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionProvenance, FixedViewCopyDecodeError> {
    Ok(SelectedInstructionProvenance {
        operations: decode_ids(cursor, OperationId::new)?,
        values: decode_ids(cursor, ValueId::new)?,
        edges: decode_ids(cursor, EdgeId::new)?,
        obligations: decode_ids(cursor, ObligationId::new)?,
        work: decode_work(cursor)?,
    })
}

pub(super) fn decode_work(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<WorkSettlement>, FixedViewCopyDecodeError> {
    decode_list(cursor, |cursor| {
        let tag = cursor.byte()?;
        let raw = cursor.u64()?;
        let site = match tag {
            0 => PsiProvenance::Operation(
                OperationId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
            ),
            1 => PsiProvenance::Edge(
                EdgeId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
            ),
            tag => return Err(FixedViewCopyDecodeError::UnknownWorkSite(tag)),
        };
        Ok(WorkSettlement {
            site,
            units: cursor.u64()?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Cursor, RegisterClassId, SelectedInstructionId, VirtualRegister, VirtualRegisterId,
        VirtualRegisterOrigin, decode_register, encode_register,
    };

    #[test]
    fn structural_observation_origin_round_trips_without_scalar_definition_site() {
        let source = VirtualRegister {
            id: VirtualRegisterId(3),
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
            class: RegisterClassId(0),
            origin: VirtualRegisterOrigin::StructuralObservation {
                instruction: SelectedInstructionId(5),
                place: semantic_vocabulary::PlaceId::new(7).unwrap(),
                byte_offset: 4,
            },
            definition_site: None,
            entry_fixed_view: None,
        };
        let mut encoded = Vec::new();
        encode_register(&mut encoded, &source);
        assert_eq!(decode_register(&mut Cursor::new(&encoded)).unwrap(), source);
        for end in 0..encoded.len() {
            assert!(decode_register(&mut Cursor::new(&encoded[..end])).is_err());
        }
        for mutation in 0..4 {
            let mut changed = source.clone();
            changed.origin = match mutation {
                0 => VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(6),
                    place: semantic_vocabulary::PlaceId::new(7).unwrap(),
                    byte_offset: 4,
                },
                1 => VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(5),
                    place: semantic_vocabulary::PlaceId::new(8).unwrap(),
                    byte_offset: 4,
                },
                2 => VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(5),
                    place: semantic_vocabulary::PlaceId::new(7).unwrap(),
                    byte_offset: 0,
                },
                _ => VirtualRegisterOrigin::AbiTransport {
                    instruction: SelectedInstructionId(5),
                    place: semantic_vocabulary::PlaceId::new(7).unwrap(),
                    byte_offset: 4,
                },
            };
            let mut changed_bytes = Vec::new();
            encode_register(&mut changed_bytes, &changed);
            assert_ne!(encoded, changed_bytes);
            assert_eq!(
                decode_register(&mut Cursor::new(&changed_bytes)).unwrap(),
                changed
            );
        }
    }
}
