use crate::data::DataProperties;
use crate::expression::ExpressionHandle;
use crate::name::DiagnosticName;
use crate::types::TypeReference;
use arena::HandleSpan;
use symbols::SymbolHandle;

/// A proof-formula declaration after lexical symbol assignment. This is a
/// distinct root category: it has no result type, executable signature,
/// effects, termination plan, or runtime body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub is_public: bool,
    pub binders: HandleSpan<PropositionBinder>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    /// Exact authored expression occurrence for a transparent formula. This
    /// remains separate from normalized proposition identity.
    pub transparent_formula_source_span: Option<source::SourceSpan>,
    pub body: PropositionBody,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionBinder {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub kind: PropositionBinderKind,
    pub bounds: DataProperties,
}

/// Proof-static binder kinds admitted by the first proposition-family slice.
/// In particular, `Machine` is an opaque machine identity index and is not a
/// callable machine parameter with an operation contract.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PropositionBinderKind {
    #[default]
    Type,
    Const {
        type_reference: TypeReference,
    },
    Machine,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PropositionBody {
    #[default]
    Primitive,
    Witness {
        evidence: TypeReference,
    },
    /// Retained only for source/debug expansion. It acquires no independent
    /// terminal proposition identity.
    Transparent {
        proposition: ExpressionHandle,
    },
}
