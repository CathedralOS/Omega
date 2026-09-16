use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterConstraintKey, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::rewrites::selected_lowering::literal_fold::identity::encode_terminal_literal_fold_content;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    RecoveryClassificationIdentity, SpillChoiceIdentity,
};

const LITERAL_FOLD_MAGIC: &[u8; 8] = b"OMGLFD\0\0";
const LITERAL_FOLD_VERSION: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiteralFoldIdentity(pub(crate) [u8; 32]);

impl LiteralFoldIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Narrow proof-preserving physical-form fold. This is not a generic constant
/// fold, instruction scheduler, rematerializer, spill policy, or opt level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiteralFoldPolicy {
    enabled_rules: u16,
}

impl LiteralFoldPolicy {
    const EXACT_ADD_BIT: u16 = 1 << 0;
    const EXACT_SUBTRACT_BIT: u16 = 1 << 1;
    const COMPARE_BIT: u16 = 1 << 2;
    const EXTENSION_BIT: u16 = 1 << 3;
    const LOAD8_INDEXED_BIT: u16 = 1 << 4;
    const COPY_BIT: u16 = 1 << 5;
    const BYTE_VIEW_ADDRESS_BIT: u16 = 1 << 6;
    const EXACT_DIVIDE_BIT: u16 = 1 << 7;
    const WRAPPING_REMAINDER_BIT: u16 = 1 << 8;
    const BITWISE_AND_ZERO_BIT: u16 = 1 << 9;
    const BITWISE_XOR_ZERO_BIT: u16 = 1 << 10;
    const WRAPPING_ADD_ZERO_BIT: u16 = 1 << 11;
    const BITWISE_AND_ONES_BIT: u16 = 1 << 12;
    const WRAPPING_REMAINDER_ZERO_BIT: u16 = 1 << 13;
    const EXACT_DIVIDE_ZERO_BIT: u16 = 1 << 14;
    const SATURATING_ADD_ZERO_BIT: u16 = 1 << 15;
    const KNOWN_BITS: u16 = Self::EXACT_ADD_BIT
        | Self::EXACT_SUBTRACT_BIT
        | Self::COMPARE_BIT
        | Self::EXTENSION_BIT
        | Self::LOAD8_INDEXED_BIT
        | Self::COPY_BIT
        | Self::BYTE_VIEW_ADDRESS_BIT
        | Self::EXACT_DIVIDE_BIT
        | Self::WRAPPING_REMAINDER_BIT
        | Self::BITWISE_AND_ZERO_BIT
        | Self::BITWISE_XOR_ZERO_BIT
        | Self::WRAPPING_ADD_ZERO_BIT
        | Self::BITWISE_AND_ONES_BIT
        | Self::WRAPPING_REMAINDER_ZERO_BIT
        | Self::EXACT_DIVIDE_ZERO_BIT
        | Self::SATURATING_ADD_ZERO_BIT;

    pub const EXACT_ADD_V1: Self = Self {
        enabled_rules: Self::EXACT_ADD_BIT,
    };
    pub const EXACT_SUBTRACT_V1: Self = Self {
        enabled_rules: Self::EXACT_SUBTRACT_BIT,
    };
    pub const COMPARE_V1: Self = Self {
        enabled_rules: Self::COMPARE_BIT,
    };
    /// Exact unary extension elimination: fold a materialized incoming literal
    /// through its sole `ZeroExtend`/`SignExtend` consumer into a direct
    /// `MaterializeI64` of the extension's exact output bits.
    pub const EXTENSION_V1: Self = Self {
        enabled_rules: Self::EXTENSION_BIT,
    };
    /// Indexed byte-load offset materialization: fold a materialized incoming
    /// literal through its sole `Load8Indexed` consumer into the direct-offset
    /// `Load8` form the literal names.
    pub const LOAD8_INDEXED_V1: Self = Self {
        enabled_rules: Self::LOAD8_INDEXED_BIT,
    };
    /// Copy-constant materialization: fold a materialized incoming literal
    /// through its sole `CopyI64` consumer into a direct `MaterializeI64` of
    /// the literal at the copy's destination register.
    pub const COPY_V1: Self = Self {
        enabled_rules: Self::COPY_BIT,
    };
    /// Address-mode folding: fold a materialized incoming literal through its
    /// sole `ByteViewAddress` consumer's offset operand into the
    /// constant-offset `AddressOffset` form the literal names.
    pub const BYTE_VIEW_ADDRESS_V1: Self = Self {
        enabled_rules: Self::BYTE_VIEW_ADDRESS_BIT,
    };
    /// Exact-division identity: fold a materialized literal `1` feeding its
    /// sole `ExactDivideU64` consumer's divisor operand into a `CopyI64` of
    /// the dividend — a divide by one is the dividend itself, so the
    /// consumer's encoded fault surface is discharged by the literal.
    pub const EXACT_DIVIDE_V1: Self = Self {
        enabled_rules: Self::EXACT_DIVIDE_BIT,
    };
    /// Wrapping-remainder constant fold: fold a materialized literal `1`
    /// feeding its sole `WrappingRemainderI64` consumer's divisor operand
    /// into a `MaterializeI64` of zero at the result register — a remainder
    /// by one is always zero, so the consumer's encoded fault surface is
    /// discharged by the literal and its scratch `Def` outputs drop dead.
    pub const WRAPPING_REMAINDER_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_BIT,
    };
    /// Bitwise-and annihilator fold: fold a materialized literal `0`
    /// feeding its sole `BitwiseAndI64` consumer at either `Use` operand
    /// into a `MaterializeI64` of zero at the result register — zero is
    /// the bitwise-and annihilator, so the constant result never reads
    /// the surviving-side `Use` the fold drops.
    pub const BITWISE_AND_ZERO_V1: Self = Self {
        enabled_rules: Self::BITWISE_AND_ZERO_BIT,
    };
    /// Bitwise-xor identity fold: fold a materialized literal `0` feeding
    /// its sole `BitwiseXorI64` consumer at either `Use` operand into a
    /// `CopyI64` of the other `Use` — zero is the bitwise-xor identity
    /// element, so `x ^ 0` and `0 ^ x` are both `x` and the surviving
    /// operand's register moves to the result unchanged.
    pub const BITWISE_XOR_ZERO_V1: Self = Self {
        enabled_rules: Self::BITWISE_XOR_ZERO_BIT,
    };
    /// Wrapping-add identity fold: fold a materialized literal `0` feeding
    /// its sole `WrappingAddI64` consumer at either `Use` operand into a
    /// `CopyI64` of the other `Use` — zero is the additive identity under
    /// modulo-2^64 wrap, so `x + 0` and `0 + x` are both `x` and the
    /// surviving operand's register moves to the result unchanged. The
    /// wrapping-add consumer binds the flag-transparent add row, so the
    /// fold carries no flag clobber to retire.
    pub const WRAPPING_ADD_ZERO_V1: Self = Self {
        enabled_rules: Self::WRAPPING_ADD_ZERO_BIT,
    };
    /// Bitwise-and identity fold: fold a materialized all-ones literal
    /// (`u64::MAX`) feeding its sole `BitwiseAndI64` consumer at either
    /// `Use` operand into a `CopyI64` of the other `Use` — all-ones is the
    /// bitwise-and identity element, so `x & MAX` and `MAX & x` are both
    /// `x` and the surviving operand's register moves to the result
    /// unchanged. The all-ones grammar is disjoint from the and-zero
    /// annihilator family on the literal's value: the producer selects
    /// between the two `BitwiseAndI64` families by which exact literal the
    /// enabled rules admit.
    pub const BITWISE_AND_ONES_V1: Self = Self {
        enabled_rules: Self::BITWISE_AND_ONES_BIT,
    };
    /// Wrapping-remainder zero-dividend fold: fold a materialized literal
    /// `0` feeding its sole `WrappingRemainderI64` consumer's dividend
    /// operand into a `MaterializeI64` of zero at the result register — a
    /// remainder of a zero dividend is always zero. The literal is not
    /// what discharges the consumer's encoded fault surface: the
    /// nonzero-divisor obligation the remainder kind carries already
    /// excludes division by zero, and the zero dividend fixes a quotient
    /// that cannot overflow — so the fold retires the trap surface under
    /// the consumer's own definedness proof while dropping the divisor
    /// `Use` and dead scratch `Def` operands. The zero-dividend grammar
    /// is disjoint from the divisor-one family on the folded literal's
    /// operand position: the producer selects between the two
    /// `WrappingRemainderI64` families by which position the recorded
    /// future use names.
    pub const WRAPPING_REMAINDER_ZERO_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_ZERO_BIT,
    };
    /// Exact-divide zero-dividend fold: fold a materialized literal `0`
    /// feeding its sole `ExactDivideU64` consumer's dividend operand into
    /// a `MaterializeI64` of zero at the result register — an unsigned
    /// divide of a zero dividend is always zero. The literal is not what
    /// discharges the consumer's encoded fault surface: a quotient of
    /// zero can never overflow, and the nonzero-divisor obligation the
    /// exact-divide kind carries already excludes division by zero — so
    /// the fold retires the trap surface under the consumer's own
    /// definedness proof while dropping the divisor `Use` and the
    /// provably-zero auxiliary `Use` operands a `div` realization reads
    /// as the dividend's upper half. The zero-dividend grammar is
    /// disjoint from the divisor-one family on the folded literal's
    /// operand position: the producer selects between the two
    /// `ExactDivideU64` families by which position the recorded future
    /// use names.
    pub const EXACT_DIVIDE_ZERO_V1: Self = Self {
        enabled_rules: Self::EXACT_DIVIDE_ZERO_BIT,
    };
    /// Saturating-add identity fold: fold a materialized literal `0`
    /// feeding its sole `SaturatingAdd` u64-carrier consumer at either
    /// `Use` operand into a `CopyI64` of the other `Use` — zero is the
    /// additive identity under unsigned saturating addition, so `x +| 0`
    /// and `0 +| x` are both `x` and the surviving operand's register
    /// moves to the result unchanged. The u64 saturating-add consumer
    /// implicitly defines the target condition state on aarch64 — its
    /// `adds` realization writes `nzcv` — and clobbers `rflags` on
    /// x86-64; the fold retires both with the folded form, admitting the
    /// consumer only while every unit its record defines is dead in the
    /// function: a reader of a retired definition would observe a stale
    /// unit.
    pub const SATURATING_ADD_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_ADD_ZERO_BIT,
    };

    pub(crate) const fn empty() -> Self {
        Self { enabled_rules: 0 }
    }

    pub const fn contains(self, other: Self) -> bool {
        other.enabled_rules != 0 && self.enabled_rules & other.enabled_rules == other.enabled_rules
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            enabled_rules: self.enabled_rules | other.enabled_rules,
        }
    }

    pub const fn enables_exact_add(self) -> bool {
        self.enabled_rules & Self::EXACT_ADD_BIT != 0
    }

    pub const fn enables_exact_subtract(self) -> bool {
        self.enabled_rules & Self::EXACT_SUBTRACT_BIT != 0
    }

    pub const fn enables_compare(self) -> bool {
        self.enabled_rules & Self::COMPARE_BIT != 0
    }

    pub const fn enables_extension(self) -> bool {
        self.enabled_rules & Self::EXTENSION_BIT != 0
    }

    pub const fn enables_load8_indexed(self) -> bool {
        self.enabled_rules & Self::LOAD8_INDEXED_BIT != 0
    }

    pub const fn enables_copy(self) -> bool {
        self.enabled_rules & Self::COPY_BIT != 0
    }

    pub const fn enables_byte_view_address(self) -> bool {
        self.enabled_rules & Self::BYTE_VIEW_ADDRESS_BIT != 0
    }

    pub const fn enables_exact_divide(self) -> bool {
        self.enabled_rules & Self::EXACT_DIVIDE_BIT != 0
    }

    pub const fn enables_wrapping_remainder(self) -> bool {
        self.enabled_rules & Self::WRAPPING_REMAINDER_BIT != 0
    }

    pub const fn enables_bitwise_and_zero(self) -> bool {
        self.enabled_rules & Self::BITWISE_AND_ZERO_BIT != 0
    }

    pub const fn enables_bitwise_xor_zero(self) -> bool {
        self.enabled_rules & Self::BITWISE_XOR_ZERO_BIT != 0
    }

    pub const fn enables_wrapping_add_zero(self) -> bool {
        self.enabled_rules & Self::WRAPPING_ADD_ZERO_BIT != 0
    }

    pub const fn enables_bitwise_and_ones(self) -> bool {
        self.enabled_rules & Self::BITWISE_AND_ONES_BIT != 0
    }

    pub const fn enables_wrapping_remainder_zero(self) -> bool {
        self.enabled_rules & Self::WRAPPING_REMAINDER_ZERO_BIT != 0
    }

    pub const fn enables_exact_divide_zero(self) -> bool {
        self.enabled_rules & Self::EXACT_DIVIDE_ZERO_BIT != 0
    }

    pub const fn enables_saturating_add_zero(self) -> bool {
        self.enabled_rules & Self::SATURATING_ADD_ZERO_BIT != 0
    }

    pub const fn canonical_bits(self) -> u16 {
        self.enabled_rules
    }

    pub const fn from_canonical_bits(bits: u16) -> Option<Self> {
        if bits == 0 || bits & !Self::KNOWN_BITS != 0 {
            None
        } else {
            Some(Self {
                enabled_rules: bits,
            })
        }
    }
}

/// Canonical recipe and output commitment. The transformed selected CFG stays
/// private to the validated carrier and is independently reconstructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralFoldPlan {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub spill_choices: SpillChoiceIdentity,
    pub recovery_classifications: RecoveryClassificationIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    /// The validated machine-effect catalog the pair descriptors were
    /// admitted against: producer, consumer, and rewritten declarations are
    /// resolved in this catalog by both the producer and the independent
    /// replay.
    pub machine_effect_catalog: MachineEffectCatalogIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: LiteralFoldPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionLiteralFold>,
    pub transformed_selected: SelectedInstructionPlanIdentity,
}

impl LiteralFoldPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_literal_fold_content(self);
        let identity = crate::literal_fold_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(LITERAL_FOLD_MAGIC);
        encoded.extend_from_slice(&LITERAL_FOLD_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, LiteralFoldDecodeError> {
        let mut cursor = LiteralFoldCursor::new(encoded);
        if cursor.take(LITERAL_FOLD_MAGIC.len())? != LITERAL_FOLD_MAGIC {
            return Err(LiteralFoldDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != LITERAL_FOLD_VERSION {
            return Err(LiteralFoldDecodeError::UnsupportedVersion(version));
        }
        let identity = LiteralFoldIdentity::from_bytes(cursor.array()?);
        let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let recovery_classifications = RecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_fuel = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(raw_fuel)
            .ok_or(LiteralFoldDecodeError::InvalidFuelSchedule(raw_fuel))?;
        let policy_bits = u16::from_le_bytes(cursor.array()?);
        let policy = LiteralFoldPolicy::from_canonical_bits(policy_bits)
            .ok_or(LiteralFoldDecodeError::UnknownPolicy(policy_bits))?;
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| LiteralFoldDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| LiteralFoldDecodeError::InvalidUsage)?;
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine)
                .ok_or(LiteralFoldDecodeError::InvalidMachineId(raw_machine))?;
            let action = match cursor.byte()? {
                0 => None,
                1 => Some(LiteralFoldAction {
                    block: SelectedBlockId(u32::from_le_bytes(cursor.array()?)),
                    pressure_point: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                    literal_instruction: SelectedInstructionId(u32::from_le_bytes(cursor.array()?)),
                    victim: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    consumer_instruction: SelectedInstructionId(u32::from_le_bytes(
                        cursor.array()?,
                    )),
                    surviving: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    result: match cursor.byte()? {
                        0 => None,
                        1 => Some(VirtualRegisterId(u32::from_le_bytes(cursor.array()?))),
                        tag => return Err(LiteralFoldDecodeError::UnknownOption(tag)),
                    },
                    immediate: u64::from_le_bytes(cursor.array()?),
                    immediate_constraint: decode_constraint_key(&mut cursor)?,
                }),
                tag => return Err(LiteralFoldDecodeError::UnknownOption(tag)),
            };
            functions.push(FunctionLiteralFold { machine, action });
        }
        let transformed_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        if cursor.remaining() != 0 {
            return Err(LiteralFoldDecodeError::TrailingBytes);
        }
        let plan = Self {
            source_selected,
            spill_choices,
            recovery_classifications,
            ranges,
            legality,
            register_environment,
            allocator_availability,
            machine_effect_catalog,
            optimization_unit,
            fuel_schedule,
            policy,
            budget,
            usage,
            functions,
            transformed_selected,
        };
        if crate::literal_fold_identity(&plan) != identity {
            return Err(LiteralFoldDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiteralFold {
    pub machine: MachineId,
    pub action: Option<LiteralFoldAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldAction {
    pub block: SelectedBlockId,
    pub pressure_point: LiveRangePoint,
    pub literal_instruction: SelectedInstructionId,
    pub victim: VirtualRegisterId,
    pub consumer_instruction: SelectedInstructionId,
    /// The source register every `Use` position of the rewritten constraint
    /// row binds — the operand that survives the fold. Under a right-literal
    /// grammar it is the consumer's operand 0, under a left-literal grammar
    /// operand 1, under the `Use`-free unary grammar it records the folded
    /// input register, and under the constant-result grammars it records
    /// the dropped non-victim `Use` — the operand-0 dividend under the
    /// right grammar, the operand-1 `Use` under the left annihilator and
    /// left auxiliary-`Use` grammars — for custody; the rewritten row
    /// binds no `Use` position at all.
    pub surviving: VirtualRegisterId,
    /// The folded consumer's scalar result. Flag-defining consumers such as
    /// `CompareI64` carry no `Def` operand and record `None`.
    pub result: Option<VirtualRegisterId>,
    pub immediate: u64,
    pub immediate_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldValidationReceipt {
    pub(crate) identity: LiteralFoldIdentity,
    pub(crate) source_selected: SelectedInstructionPlanIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) recovery_classifications: RecoveryClassificationIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) machine_effect_catalog: MachineEffectCatalogIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) transformed_selected: SelectedInstructionPlanIdentity,
    pub(crate) policy: LiteralFoldPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) applied_count: usize,
}

impl LiteralFoldValidationReceipt {
    pub const fn identity(self) -> LiteralFoldIdentity {
        self.identity
    }
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn spill_choices(self) -> SpillChoiceIdentity {
        self.spill_choices
    }
    pub const fn recovery_classifications(self) -> RecoveryClassificationIdentity {
        self.recovery_classifications
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    /// The validated machine-effect catalog identity the fold's producer and
    /// independent replay both bound.
    pub const fn machine_effect_catalog(self) -> MachineEffectCatalogIdentity {
        self.machine_effect_catalog
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn policy(self) -> LiteralFoldPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn applied_count(self) -> usize {
        self.applied_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiteralFold {
    pub(crate) plan: LiteralFoldPlan,
    pub(crate) transformed: std::sync::Arc<SelectedInstructionPlan>,
    pub(crate) receipt: LiteralFoldValidationReceipt,
}

impl ValidatedLiteralFold {
    pub const fn plan(&self) -> &LiteralFoldPlan {
        &self.plan
    }
    /// Share current immutable data; the returned artifact grants no new authority.
    pub fn shared_transformed(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        std::sync::Arc::clone(&self.transformed)
    }

    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }
    pub const fn receipt(&self) -> LiteralFoldValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralFoldError {
    RootMismatch,
    UnsupportedPolicy,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    ClassificationNotAdmitted {
        function: usize,
    },
    UnsupportedVictimRole {
        function: usize,
    },
    UnsupportedImmediate {
        function: usize,
    },
    FutureUseMismatch {
        function: usize,
    },
    LiteralMismatch {
        function: usize,
    },
    ConsumerMismatch {
        function: usize,
    },
    /// The producer, consumer, or rewritten catalog declaration does not
    /// satisfy the pair's declared machine-effect surface.
    EffectSurfaceMismatch {
        function: usize,
    },
    /// The bound machine-effect catalog does not declare a policy-enabled
    /// rewritten form under its constraint key, or that declaration violates
    /// the pair's declared effect isolation.
    EffectCatalogMismatch,
    ImmediateConstraintMismatch,
    IdentifierUnderflow {
        function: usize,
    },
    DecisionMismatch {
        function: usize,
    },
    UsageMismatch,
    TransformedPlanMismatch,
    TransformedIdentityMismatch,
}

impl std::fmt::Display for LiteralFoldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "terminal literal fold failed: {self:?}")
    }
}

impl std::error::Error for LiteralFoldError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralFoldDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u16),
    UnknownOption(u8),
    UnknownConstraintFamily(u8),
    InvalidFuelSchedule(u32),
    InvalidMachineId(u64),
    InvalidBudget,
    InvalidUsage,
    LengthOverflow,
    TrailingBytes,
    IdentityMismatch,
}

impl std::fmt::Display for LiteralFoldDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid terminal literal fold: {self:?}")
    }
}

impl std::error::Error for LiteralFoldDecodeError {}

fn decode_constraint_key(
    cursor: &mut LiteralFoldCursor<'_>,
) -> Result<RegisterConstraintKey, LiteralFoldDecodeError> {
    let family = match cursor.byte()? {
        0 => register_model::RegisterConstraintFamily::Call,
        1 => register_model::RegisterConstraintFamily::Return,
        2 => register_model::RegisterConstraintFamily::SystemCall,
        3 => register_model::RegisterConstraintFamily::InlineAssembly,
        4 => register_model::RegisterConstraintFamily::Instruction,
        tag => return Err(LiteralFoldDecodeError::UnknownConstraintFamily(tag)),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: u32::from_le_bytes(cursor.array()?),
    })
}

struct LiteralFoldCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> LiteralFoldCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], LiteralFoldDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(LiteralFoldDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(LiteralFoldDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], LiteralFoldDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| LiteralFoldDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, LiteralFoldDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, LiteralFoldDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| LiteralFoldDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}
