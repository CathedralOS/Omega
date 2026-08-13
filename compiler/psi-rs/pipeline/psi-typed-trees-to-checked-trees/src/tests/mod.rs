use super::{
    build_borrow_facts, build_domain_facts, build_flow_facts, build_operator_facts,
    build_proof_facts, build_semantic_facts, build_value_facts, lower_typed_trees,
};
use crate::flow::{StateMutationSummaryCache, call_mutated_places};
use crate::semantic::instantiate_call_contract_place;
use psi_arena::HandleSpan;
use psi_checked_trees::expression::{CallExpression, Expression, NamePath};
use psi_checked_trees::machine::{Machine, TraitConformance};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::signature::{
    SignatureContract, SignatureContractKind, StateParameter, StateSignature,
};
use psi_checked_trees::state::State;
use psi_checked_trees::statement::{StatementNode, TableCall};
use psi_checked_trees::trait_definition::TraitDefinition;
use psi_checked_trees::types::TypeReferenceNode;
use psi_checked_trees::{BorrowAccessKind, ContractProofFactKind, ContractProofFactOwner};
use psi_facts::{FactPayload, FactPlace};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_symbols::SymbolHandle;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use std::sync::Arc;

mod admissibility;
mod borrow;
mod carry;
mod cleanup;
mod content;
mod contracts;
mod domain_identity;
mod flow;
mod generics;
mod multiplicity;
mod operators;
mod relevance;
mod termination;
mod values;
