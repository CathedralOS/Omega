//! Concrete function ABI and dynamic descriptor argument shapes.

use crate::TargetStructuralParameter;
use abstract_operations::AbstractDynamicDescriptorArgument;
use calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId, ValueId};
use terminal_psi::{StructuralPathSegment, TerminalDynamicDescriptorParameter};

/// One semantic scalar value joined to its canonical target call placement.
/// Parameter rows retain declaration order in the surrounding function ABI;
/// the result uses the same record without inventing a positional index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarAbiValue {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
}

/// Exact canonical target ABI for one service-free scalar function whose
/// complete parameter and result roster consists of fixed 8/16/32/64-bit
/// integers and canonical Boolean values.
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

/// Exact canonical target ABI for one borrowed existential descriptor in a
/// function's parameter roster. `parameter` retains the complete semantic
/// interface row; `instance` and `table` bind its two pointer words to this
/// function's own call-plan placements so callers can forward the pair
/// without re-deriving either word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDynamicDescriptorParameterAbi {
    pub parameter: TerminalDynamicDescriptorParameter,
    pub instance: ValuePlacement,
    pub table: ValuePlacement,
}

/// One concrete instance address supplied to a target-level existential
/// descriptor argument. The source remains a checked projection within one
/// caller parameter; the destination is the pointer-shaped ABI placement for
/// the callee's descriptor word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDynamicDescriptorInstanceArgument {
    pub place: PlaceId,
    pub access: terminal_psi::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
}

/// The physical origin of one forwarded descriptor's instance word.
/// A projected instance is materialized from a caller-owned structural value;
/// a parameter instance passes the caller's own `{instance, table}` parameter
/// words through unchanged, so no concrete projection exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetDynamicDescriptorInstanceSource {
    /// Checked projection within one caller-owned structural value.
    Projection(TargetDynamicDescriptorInstanceArgument),
    /// Pass-through of the caller's own borrowed descriptor parameter.
    /// `parameter` retains the caller-side entry ABI placements for both
    /// descriptor words; `destination` is the callee's instance-word
    /// placement. The callee's table-word placement stays on the argument's
    /// `table_destination` field.
    Parameter {
        parameter: TargetDynamicDescriptorParameterAbi,
        destination: ValuePlacement,
    },
}

/// Target-owned ABI application for one caller-supplied existential
/// descriptor. `custody` retains the complete semantic source/target join;
/// the physical fields retain only the two pointer placements and the exact
/// instance origin needed to materialize them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDynamicDescriptorArgument {
    pub custody: AbstractDynamicDescriptorArgument,
    pub instance: TargetDynamicDescriptorInstanceSource,
    pub table_destination: ValuePlacement,
}
