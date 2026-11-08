// Boundary execution bindings, compiler builtin roles, and the append-only IEEE-754 float arithmetic operation catalog.
// `IeeeFloatArithmeticOperation` orders and encodes all 70 nearest-even and directed floating-point operations by stable append-only ordinals so machine-effect catalogs, selected-form wire tags, and admission checks never drift when narrow integer carriers or directed square-root modes are added.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderExecutionBinding {
    provider_plan_report_identity: ProviderPlanReportIdentity,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    normalized_root_report_identity: u64,
    boundary_contract_report_fingerprint: u64,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IeeeFloatArithmeticOperation {
    DirectedBinary {
        operation: IeeeFloatBinaryOperation,
        rounding: IeeeFloatRoundingDirection,
    },
    DirectedSquareRoot {
        rounding: IeeeFloatRoundingDirection,
    },
    Add,
    Subtract,
    Multiply,
    Divide,
    Minimum, // `FloatSemantics::minimum`: `left` when `left < right`, otherwise `right` (including unordered and equal `+0`/`-0`).
    Maximum, // `FloatSemantics::maximum`: `left` when `left > right`, otherwise `right`.
    Negate,
    SquareRoot,
    ConvertFormat,
    FromInteger(IeeeFloatIntegerCarrier),
    TruncateToInteger {
        target: IeeeFloatIntegerCarrier,
        domain: IeeeFloatToIntegerDomain,
    },
    MultiplyThenAdd, // Separate product and addition roundings; never select a fused instruction.
    FusedMultiplyAdd, // Single rounding of exact product plus addend.
    IsNan,
    IsInfinite,
    IsFinite,
    IsNormal,
    IsSubnormal,
    Classify, // Conventional `FloatClass` bytes: case ordinal in low word, sign Boolean in byte four.
    Compare(semantic_vocabulary::IeeeFloatComparisonOperation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatToIntegerDomain {
    Exact,
    Saturating,
    Trapping,
}

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
    pub const ALL: [Self; 70] = {
        let mut all = [Self::Add; 70];
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
        let mut square_root_rounding = 0;
        while square_root_rounding < IeeeFloatRoundingDirection::ALL.len() {
            let directed = Self::DirectedSquareRoot {
                rounding: IeeeFloatRoundingDirection::ALL[square_root_rounding],
            };
            all[directed.ordinal() as usize] = directed;
            square_root_rounding += 1;
        }
        all
    };

    const fn truncate(target: IeeeFloatIntegerCarrier, domain: IeeeFloatToIntegerDomain) -> Self {
        Self::TruncateToInteger { target, domain }
    }

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
            Self::DirectedSquareRoot { rounding } => 67 + rounding.ordinal(),
            Self::IsNan => 31,
            Self::IsInfinite => 32,
            Self::IsFinite => 57,
            Self::IsNormal => 58,
            Self::IsSubnormal => 59,
            Self::Classify => 60,
            Self::Compare(comparison) => 61 + comparison.ordinal(),
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

    pub const fn rounding_direction(self) -> Option<IeeeFloatRoundingDirection> {
        match self {
            Self::DirectedBinary { rounding, .. } | Self::DirectedSquareRoot { rounding } => {
                Some(rounding)
            }
            _ => None,
        }
    }

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
            | Self::DirectedSquareRoot { .. }
            | Self::ConvertFormat
            | Self::FromInteger(_)
            | Self::TruncateToInteger { .. } => 1,
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerBuiltinExecution {
    EmptyRunRelease,
    StaticInterrupt {
        boundary: semantic_vocabulary::BoundaryMachineId,
        operation: language_core::static_interrupts::StaticInterruptOperation,
    },
    AccessLifecycle {
        boundary: semantic_vocabulary::BoundaryMachineId,
        operation: terminal_psi::TerminalAccessLifecycleOperation,
    },
    AccessorScalarAccess {
        contract: [u8; 32],
        placement_commitment: [u8; 32],
        access_slot: u32,
        operation: terminal_psi::TerminalAccessorKind,
    },
    HostedExitProcessI32,
    HostedReadByte,
    ProgramImageRetainedSuffixStart,
    ProgramStorageEntry,
    HostedWriteByteI32,
    NearestIeeeFloatArithmetic {
        operation: IeeeFloatArithmeticOperation,
        format: semantic_vocabulary::IeeeFloatFormat,
    },
}

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
