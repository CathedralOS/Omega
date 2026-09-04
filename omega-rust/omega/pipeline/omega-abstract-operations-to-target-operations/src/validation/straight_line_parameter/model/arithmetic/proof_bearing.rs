//! Arithmetic carriers whose operation policy retains an exact proof obligation.

use psi_core::ObligationId;

use super::{IntegerArithmeticParametersSource, ReconstructedIntegerArithmeticParameters};

macro_rules! proof_bearing_arithmetic_carriers {
    ($source:ident, $reconstructed:ident) => {
        pub(in crate::validation::straight_line_parameter) struct $source {
            pub(in crate::validation::straight_line_parameter) arithmetic:
                IntegerArithmeticParametersSource,
            pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
        }

        pub(in crate::validation::straight_line_parameter) struct $reconstructed {
            pub(in crate::validation::straight_line_parameter) arithmetic:
                ReconstructedIntegerArithmeticParameters,
            pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
        }
    };
}

proof_bearing_arithmetic_carriers!(
    ExactIntegerAddParametersSource,
    ReconstructedExactIntegerAddParameters
);
proof_bearing_arithmetic_carriers!(
    ExactIntegerSubtractParametersSource,
    ReconstructedExactIntegerSubtractParameters
);
proof_bearing_arithmetic_carriers!(
    ExactIntegerMultiplyParametersSource,
    ReconstructedExactIntegerMultiplyParameters
);
proof_bearing_arithmetic_carriers!(
    ExactIntegerDivideParametersSource,
    ReconstructedExactIntegerDivideParameters
);
proof_bearing_arithmetic_carriers!(
    ExactIntegerRemainderParametersSource,
    ReconstructedExactIntegerRemainderParameters
);
proof_bearing_arithmetic_carriers!(
    WrappingIntegerDivideParametersSource,
    ReconstructedWrappingIntegerDivideParameters
);
proof_bearing_arithmetic_carriers!(
    WrappingIntegerRemainderParametersSource,
    ReconstructedWrappingIntegerRemainderParameters
);
proof_bearing_arithmetic_carriers!(
    SaturatingIntegerDivideParametersSource,
    ReconstructedSaturatingIntegerDivideParameters
);
proof_bearing_arithmetic_carriers!(
    SaturatingIntegerRemainderParametersSource,
    ReconstructedSaturatingIntegerRemainderParameters
);
