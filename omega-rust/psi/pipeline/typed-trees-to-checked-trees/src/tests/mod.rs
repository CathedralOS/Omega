use super::{
    build_borrow_facts, build_domain_facts, build_flow_facts, build_operator_facts,
    build_proof_facts, build_semantic_facts, build_value_facts, lower_typed_trees,
};
use crate::flow::{StateMutationSummaryCache, call_mutated_places};
use crate::semantic::instantiate_call_contract_place;
use arena::HandleSpan;
use checked_trees::expression::{CallExpression, Expression, NamePath};
use checked_trees::machine::{Machine, TraitConformance};
use checked_trees::name::Identifier;
use checked_trees::signature::{
    SignatureContract, SignatureContractKind, StateParameter, StateSignature,
};
use checked_trees::state::State;
use checked_trees::statement::{StatementNode, TableCall};
use checked_trees::trait_definition::TraitDefinition;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{BorrowAccessKind, ContractProofFactKind, ContractProofFactOwner};
use facts::{FactPayload, FactPlace};
use source_files_to_tokens::Lexer;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use symbols::SymbolHandle;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn mutable_borrow(target: Expression) -> Expression {
    Expression::Borrow(Box::new(checked_trees::expression::BorrowExpression {
        target,
        access: language_semantics::ReferenceAccess::Mutable,
    }))
}

mod admissibility;
mod authored_selections;
mod borrow;
mod carry;
mod cleanup;
mod content;
mod contracts;
mod domain_identity;
mod dynamic_conformances;
mod flow;
mod generics;
mod multiplicity;
mod opaque_properties;
mod operators;
mod proof_embedding_totality;
mod proof_embeddings;
mod range_call_invalidation;
mod range_entry_contracts;
mod range_expression_dependencies;
mod range_lower_bounds;
mod range_state_argument_meet;
mod range_state_call_invalidation;
mod range_value_snapshots;
mod relevance;
mod semantic_dependencies;
mod termination;
mod top_level_requirements;
mod values;
