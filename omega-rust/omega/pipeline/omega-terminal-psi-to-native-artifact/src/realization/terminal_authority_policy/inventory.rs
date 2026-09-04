//! Optimizer module role: inventory leaf. Closed compiler-intrinsic mechanism coordinates.

use std::sync::OnceLock;

use omega_effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
};
use psi_numerics::{arithmetic::ArithmeticDomain, literals::FloatFormat};
use psi_symbols::BuiltinFunction;

pub(super) const CLOSED_POLICY_ROW_COUNT: u32 = 497;
const FLOAT_FORMATS: [FloatFormat; 2] = [FloatFormat::F32, FloatFormat::F64];
const ARITHMETIC_DOMAINS: [ArithmeticDomain; 4] = [
    ArithmeticDomain::Exact,
    ArithmeticDomain::Wrapping,
    ArithmeticDomain::Saturating,
    ArithmeticDomain::Trapping,
];

pub(super) fn committed_policy_mechanisms() -> &'static [CompilerIntrinsicExecutionIdentity] {
    static MECHANISMS: OnceLock<Vec<CompilerIntrinsicExecutionIdentity>> = OnceLock::new();
    MECHANISMS.get_or_init(closed_policy_mechanisms)
}

pub(super) fn closed_policy_mechanisms() -> Vec<CompilerIntrinsicExecutionIdentity> {
    let mut mechanisms = Vec::with_capacity(CLOSED_POLICY_ROW_COUNT as usize);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32);
    mechanisms.push(CompilerIntrinsicExecutionIdentity::LinuxReadByte);
    mechanisms.extend(
        BuiltinFunction::ALL
            .into_iter()
            .map(CompilerIntrinsicExecutionIdentity::BuiltinFunction),
    );
    for operation in CompilerPrimitiveFloatBinaryOperation::ALL {
        for format in FLOAT_FORMATS {
            mechanisms.push(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                operation,
                format,
            });
        }
    }
    mechanisms.extend(
        FLOAT_FORMATS
            .into_iter()
            .map(CompilerIntrinsicExecutionIdentity::NamedFloatNegation),
    );
    for source in CompilerNumericType::ALL {
        for target in CompilerNumericType::ALL {
            for domain in ARITHMETIC_DOMAINS {
                mechanisms.push(CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                    source,
                    target,
                    domain,
                });
            }
        }
    }
    assert_eq!(mechanisms.len(), CLOSED_POLICY_ROW_COUNT as usize);
    mechanisms
}
