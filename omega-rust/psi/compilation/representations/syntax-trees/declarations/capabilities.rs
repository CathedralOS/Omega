use crate::item::*;

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct OperatorDefinition {
    pub is_public: bool,
    pub is_boundary: bool,
    pub name: HandleSpan<Identifier>,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<CapabilityContract>,
    pub spelling: Option<OperatorSpelling>,
    pub token_count: usize,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct CapabilityDefinition {
    pub name: Identifier,
    pub members: HandleSpan<CapabilityMember>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum CapabilityMember {
    Field(CapabilityField),
    State(CapabilityState),
}

impl Default for CapabilityMember {
    fn default() -> Self {
        Self::Field(CapabilityField::default())
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct CapabilityField {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct CapabilityState {
    pub signature: StateSignature,
    pub contracts: HandleSpan<CapabilityContract>,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct CapabilityContract {
    pub kind: CapabilityContractKind,
    pub keyword_source_span: Option<source::SourceSpan>,
    pub binding: Option<Identifier>,
    pub facts: HandleSpan<ProofFact>,
    pub token_count: usize,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum CapabilityContractKind {
    Ensures,
    EnsuresForResultCase {
        result_case: HandleSpan<Identifier>,
    },
    #[default]
    Requires,
    Crashes {
        cause: CrashCause,
    },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record, syntax_trees_derive::SyntaxRelocate,
)]
pub enum CrashCause {
    Trap,
    Abort,
}
