use crate::item::*;

/// Work produced after parsing. A fresh declaration reads the empty state
/// without allocating a record.
#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct ConstPreparation {
    pub materialized_initializer: Option<ExpressionHandle>,
    pub pending_target_observation: bool,
    pub normalization: Option<ConstInitializerNormalization>,
    pub qualification: Option<ConstQualification>,
    pub pending_qualification: Option<ConstDestinationObligation>,
}

impl ConstPreparation {
    pub(super) const EMPTY: Self = Self {
        materialized_initializer: None,
        pending_target_observation: false,
        normalization: None,
        qualification: None,
        pending_qualification: None,
    };
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct ConstQualification {
    pub destination: TypeReferenceHandle,
    pub initializer: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct ConstDestinationObligation {
    pub declaration: source_text::SourceSpan,
    pub destination: TypeReferenceHandle,
    pub initializer: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    record_storage::Record,
    syntax_trees_derive::SyntaxRelocate,
)]
pub struct ConstInitializerCustody {
    pub selections: Vec<crate::types::ConstArgumentOrigin>,
    pub builtin_operators: Vec<source_text::SourceSpan>,
    pub call_selections: Vec<(source_text::SourceSpan, source_text::SourceSpan)>,
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct ConstInitializerNormalization {
    pub authored_custody: ConstInitializerCustody,
    pub declaration_custody: ConstInitializerCustody,
    pub authored_expression: ExpressionHandle,
    pub result_value: language_core::constant_value::EvaluatedConstant,
    pub selections: Vec<crate::types::ConstArgumentOrigin>,
    pub builtin_operators: Vec<source_text::SourceSpan>,
    pub call_selections: Vec<(source_text::SourceSpan, source_text::SourceSpan)>,
}
