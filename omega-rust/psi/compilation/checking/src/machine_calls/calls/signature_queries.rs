//! Exact call signatures and invocation-owned argument destination answers.

use std::sync::OnceLock;
use symbols::SymbolHandle;
use typed_trees::statement::TableCall;
use typed_trees::types::TypeReferenceHandle;

/// Every source state has one signature row. Argument count is a caller's
/// eligibility check, not a second signature identity or a memo-table key.
pub(crate) struct CallDestinationQueries {
    rows: Vec<(SymbolHandle, OnceLock<Option<Vec<TypeReferenceHandle>>>)>,
}

impl CallDestinationQueries {
    fn of(program: &dyn crate::CheckingInput) -> Self {
        let mut symbols: Vec<_> = program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .map(|state| state.symbol)
            .collect();
        symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        symbols.dedup();
        Self {
            rows: symbols
                .into_iter()
                .map(|symbol| (symbol, OnceLock::new()))
                .collect(),
        }
    }
}

pub(crate) fn call_argument_destinations(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
    argument_count: usize,
) -> Option<Vec<TypeReferenceHandle>> {
    let compute = || call_argument_signature(program, target);
    let signature = match program.checking_memos() {
        None => compute(),
        Some(memos) => {
            let queries = memos
                .call_destinations
                .get_or_init(|| CallDestinationQueries::of(program));
            match queries.rows.binary_search_by_key(
                &(target.arena_index(), target.generation()),
                |(symbol, _)| (symbol.arena_index(), symbol.generation()),
            ) {
                Ok(position) => queries.rows[position].1.get_or_init(compute).clone(),
                Err(_) => compute(),
            }
        }
    }?;
    (signature.len() == argument_count).then_some(signature)
}

fn call_argument_signature(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<Vec<TypeReferenceHandle>> {
    let (machine, state) = super::machine_state_by_symbol(program, target)?;
    if !machine.symbol.is_valid()
        || !program.machine_type_parameters(machine).is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !machine.conformance_bounds.is_empty()
        || program
            .machines()
            .iter()
            .filter(|candidate| candidate.symbol == machine.symbol)
            .count()
            != 1
    {
        return None;
    }
    Some(
        program
            .state_parameters(state)
            .iter()
            .map(|parameter| {
                if parameter.is_const || !parameter.symbol.is_valid() {
                    TypeReferenceHandle::invalid()
                } else {
                    parameter.type_reference
                }
            })
            .collect(),
    )
}

/// The single machine state `symbol` selects, when exactly one state carries
/// it.
pub(crate) fn unique_state_by_symbol<'p>(
    program: &'p dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<&'p typed_trees::state::State> {
    let matches = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.symbol == symbol)
        .collect::<Vec<_>>();
    let [state] = matches.as_slice() else {
        return None;
    };
    Some(state)
}

/// A call's receiver spelled as member-name strings: the statement table's
/// name-path members owned outright for the signature selectors that compare
/// them against authored path spellings.
pub(crate) fn call_receiver_member_names(
    program: &dyn crate::CheckingInput,
    call: &TableCall,
) -> Vec<String> {
    program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect()
}
