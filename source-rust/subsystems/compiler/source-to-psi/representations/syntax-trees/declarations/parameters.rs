use crate::item::*;

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TypeParameter {
    pub name: Identifier,
    pub kind: TypeParameterKind,
    pub bounds: DataProperties,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum TypeParameterKind {
    #[default]
    Type,
    Implementation,
    Const {
        type_reference: crate::types::TypeReferenceHandle,
    },
    Value {
        type_reference: crate::types::TypeReferenceHandle,
    },
    /// A callable header requests evidence of this complete aliased contract.
    /// This is a declaration to check, not an executable machine selection.
    Proof {
        contract: StateSignature,
    },
    Machine {
        contract: Option<MachineParameterContract>,
    },
    Proposition {
        contract: Option<PropositionParameterSignature>,
    },
}

impl TypeParameterKind {
    pub fn is_const_or_value(&self) -> bool {
        matches!(self, Self::Const { .. } | Self::Value { .. })
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum MachineParameterContract {
    /// An authored `const F: machine`; exact typed uses must supply its type.
    Inferred,
    RequirementIdentity,
    Structural(StateSignature),
    Nominal {
        requirement: HandleSpan<Identifier>,
        trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    },
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct PropositionParameterSignature {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
}
