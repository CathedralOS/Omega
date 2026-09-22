//! Authored transparent propositions supply proof-derived ordering premises.
//!
//! `requires Distinct(i, j)` where `Distinct` is declared with a formula
//! (`proposition Distinct(left: u64, right: u64) = left != right;`) decomposes
//! the instantiated formula under the machine scope's builtin-meaning gate.
//! Each declared proposition parameter binds the normalized bound of its
//! actual argument, so `i != j` arrives as the same disequality premise an
//! inline `requires i != j` would supply. Primitive and witness propositions
//! own no formula and contribute nothing; an application whose argument
//! cannot normalize to an immutable integer bound supplies no premise either.
//! The recorded source stays the exact requires row, so replay re-derives the
//! same premises from the typed program instead of trusting recorded bounds.

use super::super::indexes::{NormalizedBound, normalized_bound};
use super::{PremiseScope, StatedOrderingPremise, decompose_premise_expression};
use arena::Handle;
use checked_trees::{BorrowCompatibilityPremiseSource, ContractProofFact};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::proposition::{PropositionApplication, PropositionBody, PropositionFormula};
use typed_trees::signature::StateParameter;
use typed_trees::state::State;

/// Decompose a proposition-spelled requires row when the proposition owns a
/// transparent Boolean formula. Every other body kind contributes nothing.
pub(super) fn append_proposition_premises(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    fact: Handle<ContractProofFact>,
    application: &PropositionApplication,
    premises: &mut Vec<StatedOrderingPremise>,
) {
    let Some(definition) = program
        .propositions()
        .iter()
        .find(|definition| definition.symbol == application.proposition)
    else {
        return;
    };
    let PropositionBody::Transparent { proposition } = &definition.body else {
        return;
    };
    let PropositionFormula::BooleanExpression(formula) = proposition else {
        return;
    };
    let parameters = program.proposition_parameters(definition);
    let arguments = program
        .expression_table
        .expression_handles(application.arguments);
    if parameters.len() != arguments.len() {
        return;
    }
    decompose_premise_expression(
        program,
        PremiseScope::Proposition {
            machine,
            state,
            parameters,
            arguments,
        },
        *formula,
        false,
        BorrowCompatibilityPremiseSource::Requires(fact),
        &None,
        premises,
    );
}

/// Substitute proposition parameters inside a normalized formula bound with
/// the actual argument's normalized bound, composing constant offsets. A
/// symbol not declared as a proposition parameter, or an actual that is not a
/// single symbol or integer bound, makes the bound unprovable rather than
/// silently approximate.
pub(super) fn substitute_bound(
    program: &TypedTrees,
    bound: NormalizedBound,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
) -> Option<NormalizedBound> {
    let argument_bound = |symbol: symbols::SymbolHandle| {
        let index = parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)?;
        normalized_bound(program, arguments[index])
    };
    match bound {
        NormalizedBound::Integer(_) => Some(bound),
        NormalizedBound::Symbol { symbol, offset } => match argument_bound(symbol)? {
            NormalizedBound::Symbol {
                symbol: actual,
                offset: actual_offset,
            } => Some(NormalizedBound::Symbol {
                symbol: actual,
                offset: offset + actual_offset,
            }),
            NormalizedBound::Integer(value) => Some(NormalizedBound::Integer(offset + value)),
            NormalizedBound::SymbolSum { .. } | NormalizedBound::Storage { .. } => None,
        },
        // Storage bounds are scope-local coordinates; substitution through a
        // proposition's immutable argument bounds cannot preserve the pinned
        // occurrence the storage name requires.
        NormalizedBound::Storage { .. } => None,
        NormalizedBound::SymbolSum {
            first,
            second,
            offset,
        } => {
            let substitute = |symbol| match argument_bound(symbol)? {
                NormalizedBound::Symbol {
                    symbol: actual,
                    offset: actual_offset,
                } => Some((actual, actual_offset)),
                _ => None,
            };
            let (first, first_offset) = substitute(first)?;
            let (second, second_offset) = substitute(second)?;
            // Two aliases of one argument mean coefficient two, which is
            // outside the current normalized vocabulary.
            if first == second {
                return None;
            }
            let (first, second) = if (first.arena_index(), first.generation())
                < (second.arena_index(), second.generation())
            {
                (first, second)
            } else {
                (second, first)
            };
            Some(NormalizedBound::SymbolSum {
                first,
                second,
                offset: offset + first_offset + second_offset,
            })
        }
    }
}
