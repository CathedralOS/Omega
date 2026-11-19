//! Byte lengths of selected floating-point forms. The encoder tests these
//! declarations against every supported operation and both register banks.

use selected_instructions::{
    FloatResultPolicyForm, FloatResultPolicyKind, IeeeFloatArithmeticForm,
    IeeeFloatArithmeticOperation, IeeeFloatFormat, IeeeFloatIntegerCarrier,
    IeeeFloatToIntegerDomain, MachineSizeKnowledge,
};

pub fn float_result_policy_size(form: FloatResultPolicyForm) -> MachineSizeKnowledge {
    MachineSizeKnowledge::ExactBytes(match form.policy {
        FloatResultPolicyKind::Trapping => 38,
        FloatResultPolicyKind::Saturating => 103,
    })
}

pub fn ieee_float_arithmetic_size(form: IeeeFloatArithmeticForm) -> MachineSizeKnowledge {
    use IeeeFloatArithmeticOperation::*;
    use IeeeFloatIntegerCarrier::*;

    // REX prefixes vary with the allocated register bank. Each pair gives
    // the low- and high-bank byte counts for one floating-point format.
    let (binary32, binary64) = match form.operation {
        Add | Subtract | Multiply | Divide | Minimum | Maximum => ((16, 19), (19, 19)),
        Negate => ((20, 22), (23, 23)),
        SquareRoot => ((12, 14), (14, 14)),
        ConvertFormat => ((13, 14), (13, 14)),
        FromInteger(I32) => ((8, 10), (9, 10)),
        FromInteger(I64) => ((9, 10), (10, 10)),
        FromInteger(U32) => ((11, 13), (12, 13)),
        FromInteger(U64) => ((65, 66), (66, 66)),
        FromInteger(I8 | I16 | U8 | U16) => ((26, 28), (23, 24)),
        TruncateToInteger { target, domain } => {
            let ((min32, max32), (min64, max64)) = match target {
                I32 => ((60, 64), (57, 60)),
                I64 => ((60, 61), (57, 57)),
                U32 => ((53, 54), (50, 50)),
                U64 => ((169, 170), (166, 166)),
                I8 | I16 => ((66, 68), (63, 64)),
                U8 | U16 => ((52, 54), (49, 50)),
            };
            let check = match domain {
                IeeeFloatToIntegerDomain::Trapping => 72,
                IeeeFloatToIntegerDomain::Exact | IeeeFloatToIntegerDomain::Saturating => 0,
            };
            (
                (min32 + check, max32 + check),
                (min64 + check, max64 + check),
            )
        }
        MultiplyThenAdd => ((24, 28), (28, 28)),
        FusedMultiplyAdd => ((21, 25), (25, 25)),
        IsNan | IsInfinite | IsFinite | IsNormal | IsSubnormal => ((149, 150), (150, 150)),
        Classify => ((186, 187), (187, 187)),
        Compare(_) => ((21, 24), (24, 24)),
        DirectedBinary { .. } => ((47, 50), (50, 50)),
        DirectedSquareRoot { .. } => ((43, 45), (45, 45)),
    };
    let (minimum_bytes, maximum_bytes) = match form.format {
        IeeeFloatFormat::Binary32 => binary32,
        IeeeFloatFormat::Binary64 => binary64,
    };
    MachineSizeKnowledge::EncoderResolved {
        minimum_bytes,
        maximum_bytes: Some(maximum_bytes),
    }
}
