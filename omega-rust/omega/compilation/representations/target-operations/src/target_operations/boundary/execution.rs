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

/// One IEEE-754 operation on the realization's `format`. Every operation but
/// `TruncateToInteger` and `IsNan` yields a value of that format rounded to nearest,
/// ties to even: Omega's canonical floating controls hold that rounding mode
/// at every native boundary, so ordinary scalar instructions realize it
/// without a mode change or a software fallback. Minimum, maximum, negation
/// and widening are exact, so each is its own nearest-even rounding.
/// `TruncateToInteger` reads one `format` operand and yields its integer
/// carrier, rounding toward zero as the conversion requirement declares.
/// `IsNan` instead returns Boolean, with no rounding or payload result.
///
/// Multiply-then-add reads three operands and rounds twice; binary arithmetic
/// reads two operands, and unary operations read one. The operand of
/// `ConvertFormat` has the other interchange format; the
/// operand of `FromInteger` is its named integer carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IeeeFloatArithmeticOperation {
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
    /// conversion has no realization here.
    TruncateToInteger {
        target: IeeeFloatIntegerCarrier,
        domain: IeeeFloatToIntegerDomain,
    },
    /// Round the product first, then add and round again. This must never
    /// select a fused instruction, including when all operands are finite.
    MultiplyThenAdd,
    /// Classify one evaluated payload; every NaN is true, all other values false.
    IsNan,
}

/// The arithmetic domains of a float-to-integer conversion that realize as
/// one clamped truncation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatToIntegerDomain {
    Exact,
    Saturating,
}

/// Integer carriers with a native conversion to and from both interchange
/// formats on both architectures. `u64` has no SSE2 conversion in either
/// direction; its requirements stay without a native realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatIntegerCarrier {
    I32,
    I64,
    U32,
    I8,
    I16,
    U8,
    U16,
}

impl IeeeFloatIntegerCarrier {
    /// In ordinal order; carriers are appended, never renumbered.
    pub const ALL: [Self; 7] = [
        Self::I32,
        Self::I64,
        Self::U32,
        Self::I8,
        Self::I16,
        Self::U8,
        Self::U16,
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
            Self::I64 => 64,
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
    pub const ALL: [Self; 32] = {
        let mut all = [Self::Add; 32];
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
            carrier += 1;
        }
        all[30] = Self::MultiplyThenAdd;
        all[31] = Self::IsNan;
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
            Self::IsNan => 31,
            // The 32- and 64-bit carriers came first (9 through 17); the
            // narrow carriers follow (18 through 29).
            Self::FromInteger(carrier) => {
                let carrier = carrier.ordinal();
                if carrier < 3 {
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
                };
                if target < 3 {
                    12 + target * 2 + domain
                } else {
                    22 + (target - 3) * 2 + domain
                }
            }
        }
    }

    /// The semantic arity; transport and receiving checks use this same count.
    pub const fn operand_count(self) -> usize {
        match self {
            Self::MultiplyThenAdd => 3,
            Self::Add
            | Self::Subtract
            | Self::Multiply
            | Self::Divide
            | Self::Minimum
            | Self::Maximum => 2,
            Self::Negate
            | Self::IsNan
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
            Self::IsNan => semantic_vocabulary::ScalarType::Boolean,
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

/// One compiler-owned target mechanism accepted by the consuming lowerer's
/// closed catalog. This is structural target custody, not an installed
/// provider execution or a compact authority coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerBuiltinExecution {
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
    /// fields from `external_roots::ProviderExecution`; constructing a
    /// record does not grant root admission or executable authority.
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
