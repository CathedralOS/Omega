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

/// One IEEE-754 operation whose result is a value of the realization's
/// format. Each denotes the exact operation rounded to nearest, ties to even:
/// Omega's canonical floating controls hold that rounding mode at every
/// native boundary, so ordinary scalar instructions realize it without a mode
/// change or a software fallback. Minimum, maximum, negation and widening are
/// exact, so each is its own nearest-even rounding.
///
/// The first six operations read two operands of the result format and the
/// rest read one. The operand of `ConvertFormat` has the other interchange
/// format; the operand of `FromInteger` is its named integer carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    FromInteger(IeeeFloatIntegerSource),
}

/// Integer carriers whose conversion to either interchange format is one
/// scalar instruction on both native architectures. Narrower carriers need a
/// preceding extension, and `u64` has no single x86-64 SSE2 conversion; their
/// `from_*` requirements stay without a native realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatIntegerSource {
    I32,
    I64,
    U32,
}

impl IeeeFloatIntegerSource {
    pub const ALL: [Self; 3] = [Self::I32, Self::I64, Self::U32];

    pub const fn is_signed(self) -> bool {
        matches!(self, Self::I32 | Self::I64)
    }

    pub const fn bits(self) -> u16 {
        match self {
            Self::I32 | Self::U32 => 32,
            Self::I64 => 64,
        }
    }

    /// The exact fixed-width carrier this source denotes.
    pub fn integer_type(self) -> semantic_vocabulary::IntegerType {
        let sign = if self.is_signed() {
            semantic_vocabulary::IntegerSign::Signed
        } else {
            semantic_vocabulary::IntegerSign::Unsigned
        };
        semantic_vocabulary::IntegerType::new(sign, self.bits())
            .expect("32- and 64-bit integer carriers are valid widths")
    }
}

impl IeeeFloatArithmeticOperation {
    pub const ALL: [Self; 12] = [
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Minimum,
        Self::Maximum,
        Self::Negate,
        Self::SquareRoot,
        Self::ConvertFormat,
        Self::FromInteger(IeeeFloatIntegerSource::I32),
        Self::FromInteger(IeeeFloatIntegerSource::I64),
        Self::FromInteger(IeeeFloatIntegerSource::U32),
    ];

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
            Self::FromInteger(IeeeFloatIntegerSource::I32) => 9,
            Self::FromInteger(IeeeFloatIntegerSource::I64) => 10,
            Self::FromInteger(IeeeFloatIntegerSource::U32) => 11,
        }
    }

    /// Two for the binary operations, one for the rest.
    pub const fn operand_count(self) -> usize {
        match self {
            Self::Add
            | Self::Subtract
            | Self::Multiply
            | Self::Divide
            | Self::Minimum
            | Self::Maximum => 2,
            Self::Negate | Self::SquareRoot | Self::ConvertFormat | Self::FromInteger(_) => 1,
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

    /// Whether `parameters` and `result` are exactly this operation's
    /// signature at `format`: `operand_count` operands of `operand_type` and
    /// one result of `format`.
    pub fn signature_matches(
        self,
        format: semantic_vocabulary::IeeeFloatFormat,
        parameters: &[semantic_vocabulary::ScalarType],
        result: semantic_vocabulary::ScalarType,
    ) -> bool {
        let operand = self.operand_type(format);
        parameters.len() == self.operand_count()
            && parameters.iter().all(|parameter| *parameter == operand)
            && result == semantic_vocabulary::ScalarType::IeeeFloat(format)
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
