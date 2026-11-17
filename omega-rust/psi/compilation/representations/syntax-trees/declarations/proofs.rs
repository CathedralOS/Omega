use crate::item::*;

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct PropositionDefinition {
    pub name: Identifier,
    pub is_public: bool,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub transparent_formula_source_span: Option<source::SourceSpan>,
    pub body: PropositionBody,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum PropositionBody {
    Primitive,
    Witness { evidence: TypeReferenceHandle },
    Transparent { proposition: ExpressionHandle },
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct MathematicalDefinition {
    pub name: Identifier,
    pub is_public: bool,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<MathematicalParameterHandle>,
    pub result: MathematicalTypeHandle,
    pub body: MathematicalDefinitionBody,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum MathematicalDefinitionBody {
    Definition(ExpressionHandle),
    Assumption,
}

impl Default for MathematicalDefinitionBody {
    fn default() -> Self {
        Self::Definition(ExpressionHandle::invalid())
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
pub struct MathematicalParameterNode {
    pub name: Identifier,
    pub relevance: language_core::BindingRelevance,
    pub ty: MathematicalTypeHandle,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum MathematicalTypeNode {
    Ordinary(TypeReferenceHandle),
    Arrow {
        binder: Option<Identifier>,
        domain: MathematicalTypeHandle,
        codomain: MathematicalTypeHandle,
    },
    Application {
        callee: MathematicalTypeHandle,
        arguments: HandleSpan<ExpressionHandle>,
    },
}

impl Default for MathematicalTypeNode {
    fn default() -> Self {
        Self::Ordinary(TypeReferenceHandle::invalid())
    }
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
pub struct MeasureDefinition {
    pub name: HandleSpan<Identifier>,
    pub parameter: StateParameterHandle,
    pub return_type: TypeReferenceHandle,
    pub lexicographic: bool,
    pub body: HandleSpan<ExpressionHandle>,
    pub token_count: usize,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum ProofFact {
    Expression(ExpressionHandle),
    Membership(ProofMembershipFact),
}

impl Default for ProofFact {
    fn default() -> Self {
        Self::Expression(ExpressionHandle::invalid())
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    compiler_storage::Record,
    syntax_trees_derive::SyntaxRelocate,
)]
pub struct ProofMembershipFact {
    pub value: ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
    pub domain_arguments: HandleSpan<TypeReferenceHandle>,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct SatisfiesClause {
    pub trait_name: Identifier,
    pub lifetime_arguments: Vec<Identifier>,
    pub arguments: HandleSpan<TypeReferenceHandle>,
    pub requirement: Option<Identifier>,
    pub alias: Option<Identifier>,
    pub via: Option<ExternalBinding>,
    pub via_expression: ExpressionHandle,
    pub via_keyword_source_span: Option<source::SourceSpan>,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct GenericConformanceBound {
    pub binder: Identifier,
    pub trait_name: Identifier,
    pub lifetime_arguments: Vec<Identifier>,
    pub arguments: HandleSpan<TypeReferenceHandle>,
}
