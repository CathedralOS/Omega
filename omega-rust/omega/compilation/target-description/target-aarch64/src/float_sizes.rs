//! Byte lengths of selected floating-point forms. The encoder tests these
//! declarations against every supported operation and both register banks.

use selected_instructions::{
    FloatResultPolicyForm, FloatResultPolicyKind, IeeeFloatArithmeticForm,
    IeeeFloatArithmeticOperation, IeeeFloatFormat, IeeeFloatIntegerCarrier,
    IeeeFloatToIntegerDomain, MachineSizeKnowledge,
};

pub fn float_result_policy_size(form: FloatResultPolicyForm) -> MachineSizeKnowledge {
    MachineSizeKnowledge::ExactBytes(match form.policy {
        FloatResultPolicyKind::Trapping => 28,
        FloatResultPolicyKind::Saturating => 72,
    })
}

pub fn ieee_float_arithmetic_size(form: IeeeFloatArithmeticForm) -> MachineSizeKnowledge {
    use IeeeFloatArithmeticOperation::*;
    use IeeeFloatIntegerCarrier::*;
    use IeeeFloatToIntegerDomain::{Exact, Saturating, Trapping};
    use semantic_vocabulary::IeeeFloatComparisonOperation::*;

    // AArch64 instructions occupy four bytes regardless of the register.
    let (binary32, binary64) = match form.operation {
        Add | Subtract | Multiply | Divide => (16, 16),
        Minimum | Maximum | FusedMultiplyAdd | Compare(Equal | Less | Greater) => (20, 20),
        Negate | SquareRoot | ConvertFormat | FromInteger(I8 | I16 | U8 | U16) => (12, 12),
        FromInteger(I32 | I64 | U32 | U64) => (8, 8),
        TruncateToInteger { target, domain } => match (domain, target) {
            (Exact | Saturating, I32) => (12, 12),
            (Exact | Saturating, I64 | U32 | U64) => (8, 8),
            (Exact | Saturating, I8 | I16) => (44, 40),
            (Exact | Saturating, U8 | U16) => (40, 36),
            (Trapping, I8 | I16 | I32) => (68, 64),
            (Trapping, I64) => (64, 60),
            (Trapping, U8 | U16 | U32 | U64) => (60, 56),
        },
        MultiplyThenAdd => (24, 24),
        IsNan | IsInfinite | IsFinite | IsNormal | IsSubnormal => (80, 80),
        Classify => (112, 112),
        Compare(NotEqual | LessOrEqual | GreaterOrEqual) => (28, 28),
        DirectedBinary { .. } => (40, 40),
        DirectedSquareRoot { .. } => (36, 36),
    };
    MachineSizeKnowledge::ExactBytes(match form.format {
        IeeeFloatFormat::Binary32 => binary32,
        IeeeFloatFormat::Binary64 => binary64,
    })
}
