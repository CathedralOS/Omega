//! Concrete function ABI and dynamic descriptor parameter shapes.

use crate::TargetStructuralParameter;
use abstract_operations::AbstractDynamicDescriptorArgument;
use calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use semantic_vocabulary::{IntegerType, PlaceId, ScalarType, StructuralTypeId, ValueId};
use terminal_psi::{StructuralPathSegment, TerminalDynamicDescriptorParameter};

/// One semantic scalar value joined to its canonical target call placement.
/// Parameter rows retain declaration order in the surrounding function ABI;
/// the result uses the same record without inventing a positional index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedIntegerScalarAbiValue {
    pub value: ValueId,
    pub scalar_type: IntegerType,
    pub placement: ValuePlacement,
}

/// One target-native scalar parameter of an attached Unit function.
///
/// Unlike the fixed-integer scalar-function ABI, Unit control may consume a
/// canonical Boolean directly. Keeping the semantic scalar type here avoids
/// laundering that Boolean through an integer-only carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitScalarAbiValue {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
}

/// One semantic scalar result joined to its canonical target call placement.
/// Mixed structural/scalar functions currently admit fixed integers and
/// Boolean results; the exact scalar family remains explicit in this row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedStructuralScalarAbiResult {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
}

/// Exact canonical target ABI for one service-free scalar function whose
/// complete parameter and result roster consists of fixed 8/16/32/64-bit
/// integers.
///
/// `call_plan` retains policy, clobbers, stack alignment, and entry control;
/// the ordered semantic rows bind its otherwise anonymous placements back to
/// terminal value identities and integer types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedIntegerScalarFunctionAbi {
    pub call_plan: CallPlan,
    pub parameters: Vec<FixedIntegerScalarAbiValue>,
    pub result: FixedIntegerScalarAbiValue,
}

/// Function-owned mixed ABI derived while Abstract scalar and structural
/// parameter declarations are both still available. Intermediate physical
/// owners validate this row mechanically; `NativeArtifact` rejoins its named
/// values and places to the canonical Terminal machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedStructuralScalarFunctionAbi {
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<FixedIntegerScalarAbiValue>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub result: MixedStructuralScalarAbiResult,
}

/// Target-owned physical ABI for one portable existential parameter.
///
/// Terminal Psi owns only the semantic interface. The receiving lowerer maps
/// that interface to two ordinary pointer-shaped parameters and retains their
/// exact placements here so assignment cannot recover them by convention.
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

/// Target-owned ABI application for one caller-supplied existential
/// descriptor. `custody` retains the complete semantic source/target join;
/// the physical fields retain only the two pointer placements and the exact
/// concrete instance projection needed to materialize them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDynamicDescriptorArgument {
    pub custody: AbstractDynamicDescriptorArgument,
    pub instance: TargetDynamicDescriptorInstanceArgument,
    pub table_destination: ValuePlacement,
}
