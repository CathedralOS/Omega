use crate::data::DataProperties;
use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

/// A typed proof-formula declaration. It remains outside the executable
/// machine graph and owns no runtime result or body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub binders: HandleSpan<PropositionBinder>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    pub body: PropositionBody,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionBinder {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub kind: PropositionBinderKind,
    pub bounds: DataProperties,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PropositionBinderKind {
    #[default]
    Type,
    Const {
        type_reference: TypeReferenceHandle,
    },
    Machine,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PropositionBody {
    #[default]
    Primitive,
    Witness {
        evidence: TypeReferenceHandle,
    },
    /// Source/debug expansion only; normalized proof facts inline this before
    /// semantic identity is minted.
    Transparent {
        proposition: PropositionFormula,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropositionFormula {
    Application(PropositionApplication),
    BooleanExpression(ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropositionApplication {
    pub proposition: SymbolHandle,
    pub name: Identifier,
    pub binder_arguments: Box<[PropositionBinderArgument]>,
    pub arguments: HandleSpan<ExpressionHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropositionBinderArgument {
    pub path: Box<[Identifier]>,
    pub symbol: SymbolHandle,
}
