//!
//! This file carries the operator definition and symbol lookup.
//! `named_calls.rs` resolves named calls and their candidates,
//! `satisfied_operators.rs` resolves satisfied boundary and checked
//! operators, `spellings.rs` resolves operator spellings against operands,
//! `trait_operators.rs` matches trait operator applications,
//! `type_matching.rs` matches type references under a policy and
//! `operand_signatures.rs` derives contract paths and operand signatures.

mod applications;
mod indexing;
mod named_calls;
mod operand_signatures;
mod primitive_float;
mod satisfied_operators;
mod spellings;
mod trait_operators;
mod type_matching;

pub use applications::{
    ClosedOperatorApplicationArgument, ClosedOperatorRealizationApplication,
    SymbolicOperatorTypeApplicationArgument, closed_indexed_operator_application_for_operands,
    closed_operator_application_for_operands, closed_operator_realization_application,
    symbolic_operator_type_application_for_operands,
};
pub use indexing::resolve_indexed_spelling_for_operands;
pub use named_calls::{
    named_expression_call_candidates, named_statement_call_candidates, resolve_named_call,
    resolve_named_expression_call,
};
pub use operand_signatures::{
    operator_contract_path, operator_operand_signature, operator_requires_clauses,
};
pub use primitive_float::primitive_float_binary_semantics;
pub use satisfied_operators::{
    boundary_operator_requirement_identity, resolve_satisfied_boundary_operator,
    resolve_satisfied_boundary_operator_for_conformance, resolve_satisfied_checked_operator,
    resolve_satisfied_checked_operator_for_conformance,
    resolve_specialized_checked_operator_application,
};
pub use spellings::{
    candidates_for_spelling, has_builtin_spelled_expression_meaning, resolve_spelling,
    resolve_spelling_for_operands,
};
pub use trait_operators::{
    SelectedTraitOperatorMeaning, selected_trait_operator_meanings,
    trait_operator_matches_application, trait_operator_operand_signature,
};

use arena::HandleSpan;
use language_core::operator_spelling::OperatorSpelling;
use language_semantics::const_value::CanonicalConstIdentity;
use symbols::SymbolHandle;

use crate::TypedTrees;
use crate::domain::DomainDefinition;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperatorConstBinding {
    symbol: SymbolHandle,
    value: CanonicalConstIdentity,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub is_public: bool,
    pub is_boundary: bool,
    pub symbol: SymbolHandle,
    pub name: HandleSpan<crate::name::Identifier>,
    pub lifetime_parameters: Vec<crate::name::Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<crate::signature::SignatureContract>,
    /// Optional `spelling` clause carried from syntax (Wave 0 decision #3).
    pub spelling: Option<OperatorSpelling>,
    pub token_count: usize,
}

/// A spelled operator meaning visible at a use site: a root operator, or a
/// domain operator together with its owning domain.
#[derive(Debug, Clone, Copy)]
pub struct SpelledOperator<'program> {
    pub operator: &'program OperatorDefinition,
    pub domain: Option<&'program DomainDefinition>,
}

/// Find one exact independently nameable operator declaration, whether it is
/// rooted directly, stored beneath a domain home, or the signature view of a
/// token-bearing machine (whose symbol is the machine's own). Visibility
/// belongs to the operator itself; callers must not infer it from the
/// carrier/domain path.
pub fn declaration_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&OperatorDefinition> {
    program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        )
        .chain(program.machine_token_bindings())
        .find(|operator| operator.symbol == symbol)
}
