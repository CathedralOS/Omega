use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedBlockId, VirtualRegisterId};
use psi_core::MachineId;

use crate::spill_choice_identity::encode_terminal_spill_choice_content;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
};

const SPILL_CHOICE_MAGIC: &[u8; 8] = b"OMGSPC\0\0";
const SPILL_CHOICE_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpillChoiceIdentity(pub(crate) [u8; 32]);

impl SpillChoiceIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable structural policy for the first locally witnessed pressure point.
/// This is not an optimization level or a target cost model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillChoicePolicy {
    SingleBlockFarthestEndThenHighestVregV1,
}

/// Deterministic recovery-victim evidence. Despite the historical “spill”
/// name, this artifact grants no spill/reload, rematerialization, stack-slot,
/// frame, instruction-emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillChoicePlan {
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub policy: SpillChoicePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionSpillChoices>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSpillChoices {
    pub machine: MachineId,
    /// The first pressure point only. `None` proves this bounded greedy walk
    /// encountered no pressure; it does not prove globally optimal coloring.
    pub choice: Option<SpillChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillChoice {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub incoming: VirtualRegisterId,
    pub incoming_class: RegisterClassId,
    pub incoming_common_candidates: Vec<RegisterViewId>,
    pub active_residents: Vec<PressureResident>,
    pub contenders: Vec<PressureContender>,
    pub selected_victim: VirtualRegisterId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureResident {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureContender {
    pub virtual_register: VirtualRegisterId,
    pub exclusive_end: LiveRangePoint,
    /// `None` denotes keeping an incoming value out of the current homes.
    /// `Some(view)` is the lowest legal incoming view recovered by evicting
    /// the named active resident. It is evidence, not permission to evict.
    pub reclaimed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillChoiceValidationReceipt {
    pub(crate) identity: SpillChoiceIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) policy: SpillChoicePolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) choice_count: usize,
    pub(crate) contender_count: usize,
}

impl SpillChoiceValidationReceipt {
    pub const fn identity(self) -> SpillChoiceIdentity {
        self.identity
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn policy(self) -> SpillChoicePolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn choice_count(self) -> usize {
        self.choice_count
    }
    pub const fn contender_count(self) -> usize {
        self.contender_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSpillChoices {
    pub(crate) plan: SpillChoicePlan,
    pub(crate) receipt: SpillChoiceValidationReceipt,
}

impl ValidatedSpillChoices {
    pub const fn plan(&self) -> &SpillChoicePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> SpillChoiceValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillChoiceError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTiedOperands {
        function: usize,
    },
    UnsupportedEarlyClobber {
        function: usize,
    },
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    UnresolvedEntryTransitions {
        function: usize,
        register: u32,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    NoCommonCandidate {
        function: usize,
        register: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        register: u32,
        view: u16,
    },
    UnsupportedPressureShape {
        function: usize,
        register: u32,
    },
    ChoiceMismatch {
        function: usize,
    },
    UsageMismatch,
}

impl std::fmt::Display for SpillChoiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal recovery-victim choice failed: {self:?}"
        )
    }
}

impl std::error::Error for SpillChoiceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpillChoiceDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownOption(u8),
    InvalidBudget,
    InvalidUsage,
    InvalidMachineId(u64),
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for SpillChoiceDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal spill-choice encoding: {self:?}"
        )
    }
}

impl std::error::Error for SpillChoiceDecodeError {}

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
