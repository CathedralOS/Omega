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
use source_files_to_tokens::Lexer;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use symbols::SymbolHandle;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{
    parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};

fn mutable_borrow(target: Expression) -> Expression {
    Expression::Borrow(Box::new(checked_trees::expression::BorrowExpression {
        target,
        access: language_semantics::ReferenceAccess::Mutable,
    }))
}

/// The toolchain core service declaration, resident so raw-pipeline fixtures
/// can spell `Service<R>` against the real core declaration. These unit
/// harnesses build a bare `SourceMap` with no package scope, so `use
/// omega::language::core::service` cannot resolve; installing the source with
/// `SourceOrigin::Toolchain` gives the typed-trees service classifier the
/// exact identity it requires.
const CORE_SERVICE_OMG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/service.omg"
));

/// Type `source` with `core/service.omg` resident as a Toolchain source.
/// Fixtures exercising service-carrier semantics spell `Service<R>` fields and
/// parameters; the requirement trait they close over must be `pub`.
pub(crate) fn parse_typed_trees_with_core_service(source: &str) -> typed_trees::TypedTrees {
    let mut sources = source::SourceMap::default();
    let service_source_id = sources
        .add_with_metadata(
            std::path::PathBuf::from("source/library/core/service.omg"),
            CORE_SERVICE_OMG.to_owned(),
            std::path::PathBuf::from("source/library/core"),
            None,
            source::SourceOrigin::Toolchain,
        )
        .source_id;
    let user_source_id = sources
        .add(
            std::path::PathBuf::from("tests/main.omg"),
            source.to_owned(),
        )
        .source_id;
    let service_tokens = Lexer::new(CORE_SERVICE_OMG)
        .tokenize()
        .expect("tokenize service.omg");
    let mut syntax =
        parse_syntax_trees_with_id(service_source_id, &service_tokens).expect("parse service.omg");
    let user_tokens = Lexer::new(source).tokenize().expect("tokenize");
    parse_syntax_trees_into_with_id(&mut syntax, user_source_id, &user_tokens).expect("parse");
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

/// Bind one fused-service erasure authorization per declared boundary trait —
/// the settled-state input `build_evaluation::settle_checked_providers`
/// produces on the typed trees before checking when a Fused provider is
/// selected. Unit-plan fixtures that hold `Service<R>` carriers need this:
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
