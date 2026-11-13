/*
Target-lowered calls must join semantic scalar and structural arguments, callback materializations, access-view inputs, and result custody to evaluated native ABI placements. We consolidate function ABI shapes, scalar call argument sources, boundary and direct call result roles, native callback and access-view inputs, and access-call bindings into one module.

Keeping all call-boundary contracts together makes the relationship between `CallPlan` placements, semantic value/place identities, and post-call custody transfers explicit in one place.
*/

use crate::{
    TargetOperationPlan, TargetReferenceResult, TargetStructuralHomeRequirement,
    TargetStructuralParameter, TargetUnitScalarHomeRequirement,
};
use calling_conventions::{BoundaryEntryPlan, CallPlan, NativePlace, ValuePlacement};
use semantic_vocabulary::{
    IeeeFloatValue, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use terminal_psi::{
    BoundaryStructuralResultDeclaration, StructuralOperationResult, StructuralResultDeclaration,
    TerminalAccessViewInput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetNativeCallbackArgument {
    pub registrar_invokes_callback: bool, // Producer-retained projection of the registrar's declared synchronous invocation contract.
    pub terminal_operation: OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub destination: NativePlace, // Registrar-selected destination; for a field destination, `application` carries the root pointer.
    pub field_placements: Option<calling_conventions::PrivateCallbackPlacements>,
    pub application: calling_conventions::NativeParameterApplication,
    pub registrar_boundary_entry_plan: BoundaryEntryPlan,
    pub registrar_context: calling_conventions::CallbackMaterializationContext,
    pub registrar_application_commitment: [u8; 32], // Retained compiler-origin provenance for callback application.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAccessViewInput {
    pub terminal: TerminalAccessViewInput,
    pub abi_parameter_ordinal: u32,
    pub referent_byte_size: u64,
    pub referent_alignment: u64,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlanWithAccessViewInputs {
    pub plan: TargetOperationPlan,
    pub entry_call_plan: CallPlan, // Complete entry ABI including placed access-view inputs.
    pub access_view_inputs: Vec<TargetAccessViewInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarAbiValue {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFunctionAbi {
    pub call_plan: CallPlan,
    pub parameters: Vec<ScalarAbiValue>,
    pub result: Option<ScalarAbiValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedStructuralScalarFunctionAbi {
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<ScalarAbiValue>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub result: ScalarAbiValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBoundaryResult {
    Unit,
    Structural(TargetStructuralHomeRequirement),
    Scalar(TargetUnitScalarHomeRequirement), // Scalar value produced by settlement bytes and retained as a durable home.
}

impl TargetBoundaryResult {
    pub const fn scalar_home(&self) -> Option<&TargetUnitScalarHomeRequirement> {
        match self {
            Self::Scalar(home) => Some(home),
            Self::Unit | Self::Structural(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetUnitScalarArgumentSource {
    IeeeFloatImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: IeeeFloatValue,
    },
    BlockParameter(crate::TargetScalarBlockValue),
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
    },
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    BooleanImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: bool,
    },
    Home(TargetUnitScalarHomeRequirement),
}

impl TargetUnitScalarArgumentSource {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::IeeeFloatImmediate { source_value, .. } => source_value,
            Self::BlockParameter(parameter) => parameter.value,
            Self::Parameter { source_value, .. } => source_value,
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::BooleanImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::IeeeFloatImmediate { value, .. } => ScalarType::IeeeFloat(value.format()),
            Self::BlockParameter(parameter) => parameter.scalar_type,
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::Home(home) => home.scalar_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitScalarCallArgument {
    pub parameter_index: u32,
    pub source: TargetUnitScalarArgumentSource,
    pub placement: ValuePlacement,
}

impl TargetUnitScalarCallArgument {
    pub const fn source_value(&self) -> ValueId {
        self.source.source_value()
    }

    pub const fn scalar_type(&self) -> ScalarType {
        self.source.scalar_type()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralResultContract {
    Machine(StructuralResultDeclaration),
    Requirement(BoundaryStructuralResultDeclaration),
    Callable(BoundaryStructuralResultDeclaration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetCallResult {
    Unit,
    Scalar(TargetUnitScalarHomeRequirement),
    Structural {
        result: StructuralOperationResult,
        callee_result: TargetStructuralResultContract,
        result_home: Option<TargetStructuralHomeRequirement>,
        reference_results: Vec<TargetReferenceResult>,
    },
}

impl TargetCallResult {
    pub const fn scalar_home(&self) -> Option<&TargetUnitScalarHomeRequirement> {
        match self {
            Self::Scalar(home) => Some(home),
            Self::Unit | Self::Structural { .. } => None,
        }
    }
}

pub fn selector_carrier_type() -> semantic_vocabulary::ScalarType {
    semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64 carrier"),
    )
}

pub type NativeCallTarget = abstract_operations::AbstractCallTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAccessCallBinding {
    pub input: terminal_psi::TerminalAccessViewInput,
    pub argument_index: u32,
    pub association: TargetAccessCallAssociation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAccessCallAssociation {
    Captured {
        schema: semantic_vocabulary::StructuralTypeId,
        carrier: semantic_vocabulary::StructuralTypeId,
    },
    CallerFormal(terminal_psi::TerminalAccessViewInput),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAccessInterpretation {
    Captured(abstract_operations::AccessReferenceBacking),
    Formal(terminal_psi::TerminalAccessViewInput),
}
