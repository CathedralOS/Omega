mod checks;
mod invariants;
mod labels;
mod lookup;

use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, NamePath};
use omega_checked_trees::name::Identifier;
use omega_checked_trees::statement::{
    StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode,
};
use omega_checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, BorrowFacts, BorrowRootKind,
    BorrowWritableRootFact, CheckFacts, ContractCallFact, ContractExitFact, ContractProofFact,
    ContractProofFactKind, ContractProofFactOwner, ContractProofFactRef, DomainDependencyFact,
    DomainDependencyPathFact, DomainFacts, FlowCallFact, FlowConstraintKind, FlowConstraintRef,
    FlowBorrowActivationFact, FlowExitFact, FlowFacts, FlowInvalidationFact, FlowInvalidationSource,
    FlowBorrowWeakeningFact, FlowBorrowWeakeningReason,
    FlowSemanticContextRef, FlowStateFact, FlowStatementFact, InvariantFact, InvariantFacts, CheckedTrees, ProofFactKind,
    ProofFacts, ProofObligationFact, ProofObligationOwner, StateBorrowFact,
};
use omega_core::arena::{Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FactRef, ProgramPoint};
use std::collections::BTreeSet;

use crate::labels::{semantic_contract_fact_kind, semantic_proof_obligation_kind, symbol_name};

pub fn lower_typed_trees(
    program: omega_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
    omega_validation::validate_program(&program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;
    let effects = omega_effects::infer_effects(&program);
    omega_validation::validate_effect_plan(&program, &effects)?;
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let invariants = build_invariant_facts(&program);
    let mut semantic = build_semantic_facts(&program, &proof);
    let domains = build_domain_facts(&program, &semantic);
    let flow = build_flow_facts(&program, &borrow, &proof, &mut semantic, &domains, &effects);
    let facts = CheckFacts {
        semantic,
        proof,
        borrow,
        invariants,
        domains,
        effects,
        flow,
    };
    checks::check_checked_facts(&program, &facts)?;

    Ok(CheckedTrees {
        typed: program,
        facts,
    })
}


mod semantic;
mod semantic_calls;
mod semantic_places;

pub use semantic::lower_typed_program;
use semantic::{
    build_semantic_facts, call_site_argument_expressions, find_call_site, find_state,
    find_state_in_machine, CallSite,
};


mod proof;

use invariants::build_invariant_facts;
use proof::{build_proof_facts, contract_target_from_state_symbol};
mod flow;

use flow::{build_domain_facts, build_flow_facts};

mod borrow;

use borrow::build_borrow_facts;

#[cfg(test)]
mod tests;
