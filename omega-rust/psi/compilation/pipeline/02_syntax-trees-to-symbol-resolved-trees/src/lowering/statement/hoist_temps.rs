//! The one constructor for synthetic `let __hoist_N` temps.
//!
//! Every hoist pushes `let __hoist_N = <initial>;` ahead of the rewritten
//! statement and reads the temp back through a fresh one-member `Name`.
//! `TypeReference::Unit` is the inference sentinel: the symbol-resolved ->
//! typed lowering types such a temp from its initializer
//! (`infer_hoist_temp_type`). Any other declared type is the temp's
//! annotation.

use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{LocalData, LocalDataStorage, Statement};
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;

/// Emits `let __hoist_N = <initial>;` with an inferred type and returns a
/// `Name` referencing it.
pub(crate) fn hoist_into_temp(
    lowerer: &mut Lowerer,
    initial: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    mint_hoist_temp(lowerer, hoisted, TypeReference::Unit, None, |_, _| initial)
}

/// Returns a fresh `Name` referencing a hoist temp, first emitting
/// `let __hoist_N: <declared> = <initial>;` unless `shared_subject` already
/// has a temp.
///
/// `shared_subject` is the arena index of a match subject's syntax handle,
/// which the parser shares across every arm of one match. The first arm mints
/// the temp and records it; sibling arms reference that same temp, so
/// exhaustiveness still groups them, and their own lowered subjects are left
/// orphaned. `initial` runs only when minting and before the temp's name is
/// drawn, so any temp it hoists for its own operands numbers first and
/// precedes this one in `hoisted`.
pub(crate) fn mint_hoist_temp(
    lowerer: &mut Lowerer,
    hoisted: &mut Vec<Statement>,
    declared: TypeReference,
    shared_subject: Option<u32>,
    initial: impl FnOnce(&mut Lowerer, &mut Vec<Statement>) -> ExpressionHandle,
) -> ExpressionHandle {
    let shared_temp = shared_subject.and_then(|subject| lowerer.match_subject_temp(subject));
    let name = match shared_temp {
        Some(existing) => DiagnosticName::generated(existing),
        None => {
            let initial_value = initial(lowerer, hoisted);
            let fresh = lowerer.next_hoist_name();
            if let Some(subject) = shared_subject {
                lowerer.record_match_subject_temp(subject, fresh.clone());
            }
            let name = DiagnosticName::generated(fresh);
            let type_is_inferred = matches!(declared, TypeReference::Unit);
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    type_reference: declared,
                    initial_value,
                    is_mutable: false,
                    type_is_inferred,
                    relevance: language_core::BindingRelevance::Relevant,
                },
            }));
            name
        }
    };
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut members = HandleSpan::empty();
    expressions.push_name_path_member(&mut members, name);
    let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
    expressions.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        is_self_value: false,
        head_symbol: SymbolHandle::invalid(),
        symbol: SymbolHandle::invalid(),
    }))
}
