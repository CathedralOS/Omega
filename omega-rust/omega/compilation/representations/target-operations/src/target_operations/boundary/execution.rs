//! Selected implementation roles and provider report coordinates.

/// Non-authoritative report identity for the installation-selected provider
/// plan of one bodyless boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderPlanReportIdentity(u64);

impl ProviderPlanReportIdentity {
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Non-authoritative target-operation report projection of the exact admitted
/// provider execution selected for this terminal realization.
///
/// The ledger-owned `ProviderExecutionEvidence` borrowed by lowering is the
/// authority carrier. These compact coordinates support deterministic reports
/// and serialization only and cannot recreate admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderExecutionBinding {
    provider_plan_report_identity: ProviderPlanReportIdentity,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    normalized_root_report_identity: u64,
    boundary_contract_report_fingerprint: u64,
}

/// The arithmetic whose exact result is rounded once in the selected mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IeeeFloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl IeeeFloatBinaryOperation {
    pub const ALL: [Self; 4] = [Self::Add, Self::Subtract, Self::Multiply, Self::Divide];
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Add => 0,
            Self::Subtract => 1,
            Self::Multiply => 2,
            Self::Divide => 3,
        }
    }
}

/// An explicit directed mode, independent of the caller's floating controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IeeeFloatRoundingDirection {
    TowardZero,
    TowardPositive,
    TowardNegative,
}

impl IeeeFloatRoundingDirection {
    pub const ALL: [Self; 3] = [Self::TowardZero, Self::TowardPositive, Self::TowardNegative];
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::TowardZero => 0,
            Self::TowardPositive => 1,
            Self::TowardNegative => 2,
        }
    }
}

/// One IEEE-754 operation on the realization's `format`. Every ordinary operation but
/// `TruncateToInteger` and Boolean classification yields a value of that format
/// rounded to nearest,
/// ties to even: Omega's canonical floating controls hold that rounding mode
/// at every native boundary, so ordinary scalar instructions realize it
/// without a mode change or a software fallback. Minimum, maximum, negation
/// and widening are exact, so each is its own nearest-even rounding.
/// `TruncateToInteger` reads one `format` operand and yields its integer
/// carrier, rounding toward zero as the conversion requirement declares.
/// Classification instead returns Boolean, with no rounding or payload result.
///
/// Multiply-then-add and fused multiply-add read three operands, with two
/// roundings and one rounding respectively; binary arithmetic reads two
/// operands, and unary operations read one. The operand of
/// `ConvertFormat` has the other interchange format; the
/// operand of `FromInteger` is its named integer carrier.
/// Directed binary operations retain the exact selected operation and mode.
/// Their local save/restore envelope preserves the caller's canonical controls,
/// masked exceptions and gradual-underflow invariant; mode is never ambient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IeeeFloatArithmeticOperation {
    /// Format and mode survive admission, codecs and independent byte recovery.
    DirectedBinary {
        operation: IeeeFloatBinaryOperation,
        rounding: IeeeFloatRoundingDirection,
    },
    Add,
    Subtract,
    Multiply,
    Divide,
    /// `FloatSemantics::minimum`: `left` when `left < right`, otherwise
    /// `right`. Unordered and equal operands (including `+0`/`-0`) yield the
    /// second operand, so this is neither IEEE `minimum` nor `minimumNumber`.
    Minimum,
    /// `FloatSemantics::maximum`: `left` when `left > right`, otherwise
    /// `right`, with the same second-operand rule.
    Maximum,
    /// The sign flip; it is exact and also flips the sign of a NaN.
    Negate,
    SquareRoot,
    /// Binary32 to binary64 (exact) or binary64 to binary32 (rounded).
    ConvertFormat,
    /// The total Exact-domain conversion of an integer carrier.
    FromInteger(IeeeFloatIntegerCarrier),
    /// Toward-zero conversion of a `format` operand to `target`. Saturating
    /// clamps to the carrier's range and maps NaN to zero. An Exact
    /// requirement carries its proof that the operand is finite and its
    /// truncation fits, where that clamp is the identity, so both domains
    /// realize alike; the domain stays in the operation so admission and the
    /// recorded terminal mechanism keep the selected requirement. Trapping
    /// checks that same representable truncation range and crashes on failure,
    /// including NaN; it must not silently select the saturating mechanism.
    TruncateToInteger {
        target: IeeeFloatIntegerCarrier,
        domain: IeeeFloatToIntegerDomain,
    },
    /// Round the product first, then add and round again. This must never
    /// select a fused instruction, including when all operands are finite.
    MultiplyThenAdd,
    /// One rounding of the exact product plus addend. Never substitute the
    /// separately rounded multiply-then-add operation.
    FusedMultiplyAdd,
    /// Classify one evaluated payload; every NaN is true, all other values false.
    IsNan,
    /// True for either infinity; false for finite values and every NaN.
    IsInfinite,
    /// True for finite values, including either signed zero.
    IsFinite,
    /// True only for a finite nonzero value with a nonzero exponent field.
    IsNormal,
    /// True only for a nonzero value with a zero exponent field.
    IsSubnormal,
    /// Native conventional FloatClass bytes: case ordinal in the low word,
    /// sign Boolean in byte four. The boundary result remains structural;
    /// only the selected form exposes this temporary integer carrier.
    Classify,
    /// The authored IEEE relation on two `format` operands, with a Boolean
    /// result. NaNs make every ordered relation and equality false and
    /// inequality true; signed zeros compare equal.
    Compare(semantic_vocabulary::IeeeFloatComparisonOperation),
}

/// The selected requirement's invalid-input behavior, retained through emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatToIntegerDomain {
    Exact,
    Saturating,
    Trapping,
}

/// Integer carriers converted to and from both IEEE interchange formats.
/// A target may need an instruction sequence rather than a single conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatIntegerCarrier {
    I32,
    I64,
    U32,
    I8,
    I16,
    U8,
    U16,
    U64,
}

impl IeeeFloatIntegerCarrier {
    /// In ordinal order; carriers are appended, never renumbered.
    pub const ALL: [Self; 8] = [
        Self::I32,
        Self::I64,
        Self::U32,
        Self::I8,
        Self::I16,
        Self::U8,
        Self::U16,
        Self::U64,
    ];

    pub const fn ordinal(self) -> u8 {
        match self {
            Self::I32 => 0,
            Self::I64 => 1,
            Self::U32 => 2,
            Self::I8 => 3,
            Self::I16 => 4,
            Self::U8 => 5,
            Self::U16 => 6,
            Self::U64 => 7,
        }
    }

    pub const fn is_signed(self) -> bool {
        matches!(self, Self::I32 | Self::I64 | Self::I8 | Self::I16)
    }

    pub const fn bits(self) -> u16 {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::U64 => 64,
        }
    }

    /// The exact fixed-width carrier this denotes.
    pub fn integer_type(self) -> semantic_vocabulary::IntegerType {
        let sign = if self.is_signed() {
            semantic_vocabulary::IntegerSign::Signed
        } else {
            semantic_vocabulary::IntegerSign::Unsigned
        };
        semantic_vocabulary::IntegerType::new(sign, self.bits())
            .expect("8- through 64-bit integer carriers are valid widths")
    }
}

/// Operations order by their stable ordinal, which is also the order of
/// [`IeeeFloatArithmeticOperation::ALL`]: machine-effect catalogs require
/// their declarations in this order, and narrow carriers appended later than
/// `TruncateToInteger` must not sort between older operations.
impl PartialOrd for IeeeFloatArithmeticOperation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IeeeFloatArithmeticOperation {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ordinal().cmp(&other.ordinal())
    }
}

impl IeeeFloatArithmeticOperation {
    pub const ALL: [Self; 67] = {
        let mut all = [Self::Add; 67];
        let fixed = [
            Self::Add,
            Self::Subtract,
            Self::Multiply,
            Self::Divide,
            Self::Minimum,
            Self::Maximum,
            Self::Negate,
            Self::SquareRoot,
            Self::ConvertFormat,
        ];
        let mut index = 0;
        while index < fixed.len() {
            all[index] = fixed[index];
            index += 1;
        }
        let mut carrier = 0;
        while carrier < IeeeFloatIntegerCarrier::ALL.len() {
            let target = IeeeFloatIntegerCarrier::ALL[carrier];
            let conversion = Self::FromInteger(target);
            all[conversion.ordinal() as usize] = conversion;
            let exact = Self::truncate(target, IeeeFloatToIntegerDomain::Exact);
            all[exact.ordinal() as usize] = exact;
            let saturating = Self::truncate(target, IeeeFloatToIntegerDomain::Saturating);
            all[saturating.ordinal() as usize] = saturating;
            let trapping = Self::truncate(target, IeeeFloatToIntegerDomain::Trapping);
            all[trapping.ordinal() as usize] = trapping;
            carrier += 1;
        }
        all[30] = Self::MultiplyThenAdd;
        all[31] = Self::IsNan;
        all[32] = Self::IsInfinite;
        all[44] = Self::FusedMultiplyAdd;
        all[57] = Self::IsFinite;
        all[58] = Self::IsNormal;
        all[59] = Self::IsSubnormal;
        all[60] = Self::Classify;
        let mut comparison = 0;
        while comparison < semantic_vocabulary::IeeeFloatComparisonOperation::ALL.len() {
            let compare =
                Self::Compare(semantic_vocabulary::IeeeFloatComparisonOperation::ALL[comparison]);
            all[compare.ordinal() as usize] = compare;
            comparison += 1;
        }
        let mut operation = 0;
        while operation < IeeeFloatBinaryOperation::ALL.len() {
            let mut rounding = 0;
            while rounding < IeeeFloatRoundingDirection::ALL.len() {
                let directed = Self::DirectedBinary {
                    operation: IeeeFloatBinaryOperation::ALL[operation],
                    rounding: IeeeFloatRoundingDirection::ALL[rounding],
                };
                all[directed.ordinal() as usize] = directed;
                rounding += 1;
            }
            operation += 1;
        }
        all
    };

    const fn truncate(target: IeeeFloatIntegerCarrier, domain: IeeeFloatToIntegerDomain) -> Self {
        Self::TruncateToInteger { target, domain }
    }

    /// The operation's position in [`Self::ALL`]. Identity encodings and
    /// selected-form tags derive from this one table; operations are
    /// appended, never renumbered.
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Add => 0,
            Self::Subtract => 1,
            Self::Multiply => 2,
            Self::Divide => 3,
            Self::Minimum => 4,
            Self::Maximum => 5,
            Self::Negate => 6,
            Self::SquareRoot => 7,
            Self::ConvertFormat => 8,
            Self::MultiplyThenAdd => 30,
            Self::FusedMultiplyAdd => 44,
            Self::DirectedBinary {
                operation,
                rounding,
            } => 45 + operation.ordinal() * 3 + rounding.ordinal(),
            Self::IsNan => 31,
            Self::IsInfinite => 32,
            Self::IsFinite => 57,
            Self::IsNormal => 58,
            Self::IsSubnormal => 59,
            Self::Classify => 60,
            Self::Compare(comparison) => 61 + comparison.ordinal(),
            // The 32- and 64-bit carriers came first (9 through 17); the
            // narrow carriers follow (18 through 29).
            Self::FromInteger(carrier) => {
                let carrier = carrier.ordinal();
                if carrier == 7 {
                    33
                } else if carrier < 3 {
                    9 + carrier
                } else {
                    15 + carrier
                }
            }
            Self::TruncateToInteger { target, domain } => {
                let target = target.ordinal();
                let domain = match domain {
                    IeeeFloatToIntegerDomain::Exact => 0,
                    IeeeFloatToIntegerDomain::Saturating => 1,
                    IeeeFloatToIntegerDomain::Trapping => return 36 + target,
                };
                // Existing identities 0..=32 remain stable. U64 operations
                // and trapping conversions extend, rather than renumber, them.
                if target == 7 {
                    34 + domain
                } else if target < 3 {
                    12 + target * 2 + domain
                } else {
                    22 + (target - 3) * 2 + domain
                }
            }
        }
    }

    /// The operation owns its mode; an ambient nearest-even control cannot
    /// realize or authenticate an explicitly directed operation.
    pub const fn rounding_direction(self) -> Option<IeeeFloatRoundingDirection> {
        match self {
            Self::DirectedBinary { rounding, .. } => Some(rounding),
            _ => None,
        }
    }

    /// The semantic arity; transport and receiving checks use this same count.
    pub const fn operand_count(self) -> usize {
        match self {
            Self::MultiplyThenAdd | Self::FusedMultiplyAdd => 3,
            Self::Add
            | Self::Subtract
            | Self::Multiply
            | Self::Divide
            | Self::Minimum
            | Self::Maximum
            | Self::DirectedBinary { .. }
            | Self::Compare(_) => 2,
            Self::Negate
            | Self::IsNan
            | Self::IsInfinite
            | Self::IsFinite
            | Self::IsNormal
            | Self::IsSubnormal
            | Self::Classify
            | Self::SquareRoot
            | Self::ConvertFormat
            | Self::FromInteger(_)
            | Self::TruncateToInteger { .. } => 1,
        }
    }

    /// The scalar type of every operand when the result has `format`.
    pub fn operand_type(
        self,
        format: semantic_vocabulary::IeeeFloatFormat,
    ) -> semantic_vocabulary::ScalarType {
        use semantic_vocabulary::{IeeeFloatFormat, ScalarType};
        match self {
            Self::ConvertFormat => ScalarType::IeeeFloat(match format {
                IeeeFloatFormat::Binary32 => IeeeFloatFormat::Binary64,
                IeeeFloatFormat::Binary64 => IeeeFloatFormat::Binary32,
            }),
            Self::FromInteger(source) => ScalarType::Integer(source.integer_type()),
            _ => ScalarType::IeeeFloat(format),
        }
    }

    /// The scalar type of the result: the integer carrier of
    /// `TruncateToInteger`, Boolean for classification, otherwise `format`.
    pub fn result_type(
        self,
        format: semantic_vocabulary::IeeeFloatFormat,
    ) -> semantic_vocabulary::ScalarType {
        match self {
            Self::IsNan
            | Self::IsInfinite
            | Self::IsFinite
            | Self::IsNormal
            | Self::IsSubnormal
            | Self::Compare(_) => semantic_vocabulary::ScalarType::Boolean,
            Self::Classify => semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .expect("u64 is valid"),
            ),
            Self::TruncateToInteger { target, .. } => {
                semantic_vocabulary::ScalarType::Integer(target.integer_type())
            }
            _ => semantic_vocabulary::ScalarType::IeeeFloat(format),
        }
    }

    /// Whether `parameters` and `result` are exactly this operation's
    /// signature at `format`: `operand_count` operands of `operand_type` and
    /// one result of `result_type`.
    pub fn signature_matches(
        self,
        format: semantic_vocabulary::IeeeFloatFormat,
        parameters: &[semantic_vocabulary::ScalarType],
        result: semantic_vocabulary::ScalarType,
    ) -> bool {
        let operand = self.operand_type(format);
        parameters.len() == self.operand_count()
            && parameters.iter().all(|parameter| *parameter == operand)
            && result == self.result_type(format)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IeeeFloatArithmeticOperation as Operation, IeeeFloatIntegerCarrier as Carrier,
        IeeeFloatToIntegerDomain as Domain,
    };

    #[test]
    fn float_operation_extensions_preserve_unique_stable_ordinals() {
        for (ordinal, operation) in Operation::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(operation.ordinal()), ordinal, "{operation:?}");
        }
        assert_eq!(Operation::MultiplyThenAdd.ordinal(), 30);
        assert_eq!(Operation::FusedMultiplyAdd.ordinal(), 44);
        assert_eq!(Operation::IsNan.ordinal(), 31);
        assert_eq!(Operation::IsInfinite.ordinal(), 32);
        assert_eq!(Operation::FromInteger(Carrier::U64).ordinal(), 33);
        for carrier in Carrier::ALL {
            assert_eq!(
                Operation::TruncateToInteger {
                    target: carrier,
                    domain: Domain::Trapping
                }
                .ordinal(),
                36 + carrier.ordinal()
            );
        }
    }
}

/// One compiler-owned target mechanism accepted by the consuming lowerer's
/// closed catalog. This is structural target custody, not an installed
/// provider execution or a compact authority coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerBuiltinExecution {
    /// Closed sealed operation identity; every occurrence separately owes RAM custody.
    PlacedScalarAccess {
        contract: [u8; 32],
        placement_commitment: [u8; 32],
        access_slot: u32,
        operation: terminal_psi::TerminalPlacedAccessorKind,
    },
    HostedExitProcessI32,
    HostedReadByte,
    HostedWriteByteI32,
    /// A selected primitive float arithmetic provider. It realizes inline as
    /// one scalar IEEE operation in the caller rather than as a call.
    NearestIeeeFloatArithmetic {
        operation: IeeeFloatArithmeticOperation,
        format: semantic_vocabulary::IeeeFloatFormat,
    },
}

/// Closed execution roles for a realized Terminal boundary.
///
/// Installed and foreign implementations retain their admitted provider
/// execution. Compiler-owned target builtins instead retain the exact local
/// catalog identity accepted by the consuming lowerer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryExecutionBinding {
    AdmittedProvider(ProviderExecutionBinding),
    CompilerBuiltin(CompilerBuiltinExecution),
}

impl From<ProviderExecutionBinding> for BoundaryExecutionBinding {
    fn from(binding: ProviderExecutionBinding) -> Self {
        Self::AdmittedProvider(binding)
    }
}

impl ProviderExecutionBinding {
    /// Non-authoritative data projection. Production lowering obtains these
    /// fields from selected-provider and installation evidence; constructing
    /// a record does not grant entry admission or executable authority.
    pub fn from_execution_record(
        provider_plan_report_identity: ProviderPlanReportIdentity,
        provider_execution_report_identity: u64,
        provider_execution_report_fingerprint: u64,
        normalized_root_report_identity: u64,
        boundary_contract_report_fingerprint: u64,
    ) -> Option<Self> {
        [
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        ]
        .iter()
        .all(|identity| *identity != 0)
        .then_some(Self {
            provider_plan_report_identity,
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        })
    }

    pub const fn provider_plan_report_identity(self) -> ProviderPlanReportIdentity {
        self.provider_plan_report_identity
    }

    pub const fn provider_execution_report_identity(self) -> u64 {
        self.provider_execution_report_identity
    }

    pub const fn provider_execution_report_fingerprint(self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    pub const fn normalized_root_report_identity(self) -> u64 {
        self.normalized_root_report_identity
    }

    pub const fn boundary_contract_report_fingerprint(self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}
