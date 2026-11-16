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

    optimization_core::structural_variant_accessor!(StructuralResultDeclaration; Self::Unit | Self::Scalar(_));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueBinding {
    pub parameter: ValueId,
    pub argument: ValueId,
    pub scalar_type: ScalarType,
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
