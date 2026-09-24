use super::lower_typed_trees;
use crate::flow::{StateMutationSummaryCache, call_mutated_places};
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
use std::sync::Arc;
use symbols::SymbolHandle;

fn mutable_borrow(target: Expression) -> Expression {
    Expression::Borrow(Box::new(checked_trees::expression::BorrowExpression {
        target,
        access: language_semantics::ReferenceAccess::Mutable,
    }))
}

/// Bind one fused-service erasure authorization per declared boundary trait —
/// the settled-state input `build_evaluation::settle_checked_providers`
/// produces on the typed trees before checking when a Fused provider is
/// selected. Unit-plan fixtures that hold `Binding<R>` carriers need this:
/// without an authorization the carrier field stays unshaped and the machine
/// fails closed. The digest is a stand-in; nothing in these harnesses compares
/// it against a realized plan.
pub(crate) fn bind_fixture_fused_service_erasures(typed: &mut typed_trees::TypedTrees) {
    let authorizations = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(
            |definition| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: definition.symbol,
                provider_plan_digest: [0x5a; 32],
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("fixture boundary traits admit fused service authorizations");
}

pub(crate) mod front_end;

mod admissibility;
mod authored_selections;
mod borrow;
mod carry;
mod cleanup;
mod content;
mod contracts;
mod domain_identity;
mod dynamic_conformances;
mod float_entry_ranges;
mod flow;
mod generics;
mod integer_entry_ranges;
mod multiplicity;
mod opaque_properties;
mod operational_tail_calls;
mod operators;
mod place_labels;
mod proof_embedding_totality;
mod proof_embeddings;
mod range_atomic_dependencies;
mod range_byte_live_lengths;
mod range_call_invalidation;
mod range_entry_contracts;
mod range_expression_dependencies;
#[path = "ranges/guard_operator_meaning.rs"]
mod range_guard_operator_meaning;
mod range_index_dependencies;
mod range_lower_bounds;
mod range_short_circuit;
mod range_state_argument_meet;
mod range_state_call_invalidation;
mod range_value_snapshots;
mod range_write_coordinates;
mod relevance;
mod semantic_dependencies;
mod termination;
mod top_level_requirements;
mod value_dispatch;
mod values;
