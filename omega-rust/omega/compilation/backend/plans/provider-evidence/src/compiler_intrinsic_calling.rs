//! Native entry envelopes of the closed compiler-intrinsic catalog.
//!
//! These shapes belong to the mechanism, not an arbitrary source ABI graph.
//! Hosted input returns the catalog's conventional empty/byte sum in its
//! caller-owned result home. Source issuance independently authenticates the
//! nominal declaration; receiving admission independently checks the complete
//! Terminal case/payload contract. Neither may infer a public sum ABI from
//! private layout or redefine this envelope from retained placements.

use crate::CompilerIntrinsicExecutionIdentity;
use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, ValueShape,
    evaluate_conventional_sum_layout, evaluate_ordinary_boundary_entry_plan,
};

/// The existing catalog's complete native envelope. Inline numeric execution
/// has no call entry; unsupported hosted targets have no native envelope.
pub fn compiler_intrinsic_boundary_entry_plan(
    execution: CompilerIntrinsicExecutionIdentity,
    target: target::NativeTarget,
) -> Result<Option<BoundaryEntryPlan>, String> {
    let integer = ValueShape::integer(4, 4);
    let signature = match execution {
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
        | CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => CallSignature {
            parameters: vec![integer],
            result: None,
        },
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            let layout = evaluate_conventional_sum_layout(&[], &[vec![], vec![integer]])
                .map_err(|error| format!("hosted byte result layout is unavailable: {error:?}"))?;
            CallSignature {
                parameters: Vec::new(),
                result: Some(layout.shape),
            }
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(_)
        | CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { .. }
        | CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison { .. }
        | CompilerIntrinsicExecutionIdentity::NamedFloatNegation(_)
        | CompilerIntrinsicExecutionIdentity::NamedFloatConversion { .. } => return Ok(None),
    };
    if target::HostedIntrinsicBundle::from_native_target(target).is_none() {
        return Err("hosted intrinsic has no catalog on this native target".to_owned());
    }
    evaluate_ordinary_boundary_entry_plan(CallingPolicy::native_for_target(target), &signature)
        .map(|validated| Some(validated.plan().clone()))
        .map_err(|error| error.to_string())
}
