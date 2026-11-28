// Scalar parameter, result and successor-binding identities shared by the
// native program forms.

use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::StructuralResultDeclaration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractResult {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

impl From<terminal_psi::ValueDeclaration> for AbstractResult {
    fn from(declaration: terminal_psi::ValueDeclaration) -> Self {
        Self {
            value: declaration.id,
            scalar_type: declaration.scalar_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractFunctionResult {
    Unit,
    Scalar(AbstractResult),
    Structural(StructuralResultDeclaration),
}

impl AbstractFunctionResult {
    pub const fn scalar(&self) -> Option<AbstractResult> {
        match self {
            Self::Unit => None,
            Self::Scalar(result) => Some(*result),
            Self::Structural(_) => None,
        }
    }

    record_storage::enum_variant_accessor!(structural, Structural; StructuralResultDeclaration; Self::Unit | Self::Scalar(_));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueBinding {
    pub parameter: ValueId,
    pub argument: ValueId,
    pub scalar_type: ScalarType,
}

/// The scalar definition which must occupy a physical view on one assembly edge.
/// Adjacent labels retain their individual register versions after native block fusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssemblyRegisterVersion {
    pub edge: semantic_vocabulary::EdgeId,
    pub view: terminal_psi::AsmRegisterView,
    pub operation: semantic_vocabulary::OperationId,
    pub value: ValueId,
    pub materialization: AssemblyRegisterMaterialization,
}

/// The selected instruction contract whose result establishes the edge value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssemblyRegisterMaterialization {
    IntegerImmediate(semantic_vocabulary::IntegerValue),
}

pub fn binding_arguments(bindings: &[ValueBinding]) -> impl Iterator<Item = ValueId> + '_ {
    bindings.iter().map(|binding| binding.argument)
}

/// One structural argument bound to a destination block's parameter place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStructuralBinding {
    pub parameter: semantic_vocabulary::PlaceId,
    pub argument: terminal_psi::StructuralArgument,
}
