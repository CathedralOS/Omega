//! Concrete function ABI shapes.

use crate::TargetStructuralParameter;
use calling_conventions::{CallPlan, ValuePlacement};
use semantic_vocabulary::{ScalarType, ValueId};

/// One semantic scalar value joined to its canonical target call placement.
/// Parameter rows retain declaration order in the surrounding function ABI;
/// the result uses the same record without inventing a positional index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarAbiValue {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
}

/// Exact canonical target ABI for one scalar function whose complete parameter
/// and result roster consists of fixed 8/16/32/64-bit integers, Boolean values,
/// or binary32/binary64 floats. Service reach does not affect value placement.
///
/// `call_plan` retains policy, clobbers, stack alignment, and entry control;
/// the ordered semantic rows bind its otherwise anonymous placements back to
/// terminal value identities and integer types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFunctionAbi {
    pub call_plan: CallPlan,
    pub parameters: Vec<ScalarAbiValue>,
    pub result: ScalarAbiValue,
}

/// Function-owned mixed ABI derived while Abstract scalar and structural
/// parameter declarations are both still available. Intermediate physical
/// owners validate this row mechanically; `NativeArtifact` rejoins its named
/// values and places to the canonical Terminal machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedStructuralScalarFunctionAbi {
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<ScalarAbiValue>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub result: ScalarAbiValue,
}
