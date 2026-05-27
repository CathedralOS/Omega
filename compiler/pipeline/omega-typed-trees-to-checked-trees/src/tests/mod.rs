use super::{
    build_borrow_facts, build_domain_facts, build_flow_facts, build_proof_facts,
    build_semantic_facts, lower_typed_trees,
};
use crate::flow::{StateMutationSummaryCache, call_mutated_places};
use crate::semantic::instantiate_call_contract_place;
use omega_checked_trees::expression::{CallExpression, Expression, NamePath};
use omega_checked_trees::machine::{Machine, TraitConformance};
use omega_checked_trees::name::Identifier;
use omega_checked_trees::signature::{
    SignatureContract, SignatureContractKind, StateParameter, StateSignature,
};
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{StatementNode, TableCall};
use omega_checked_trees::trait_definition::TraitDefinition;
use omega_checked_trees::types::TypeReferenceNode;
use omega_checked_trees::{BorrowAccessKind, ContractProofFactKind, ContractProofFactOwner};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactPayload, FactPlace};
use omega_source_files_to_tokens::Lexer;
use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use omega_tokens_to_syntax_trees::parse_syntax_trees;
use std::sync::Arc;

mod borrow;
mod contracts;
mod flow;
mod termination;
