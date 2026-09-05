//! Declaration projection follows the checked precondition's entry scope.

use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::facts::exactly_one;
use omega_compiler::CheckedCompilation;
use psi_checked_trees::{ContractProofFactKind, ContractProofFactOwner};
use psi_diagnostics::Diagnostic;
use psi_facts::{FactOrigin, ProgramPoint};

/// The source keyword selects a self formal, never a same-spelled nominal.
/// A resolved attachment root must be the exact containing machine whose
/// checked parameter roster owns that formal.
pub(crate) fn checked_self_parameter_symbol(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    path: &psi_typed_trees::expression::TableNamePath,
) -> Option<psi_symbols::SymbolHandle> {
    if context.domain_symbol.is_some()
        || context.data_symbol.is_some()
        || !compilation
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|name| name.as_str() == "self")
    {
        return None;
    }
    let mut parameters = context
        .parameters
        .iter()
        .filter(|parameter| parameter.is_self);
    let parameter = parameters.next()?;
    if parameters.next().is_some() || !parameter.symbol.is_valid() {
        return None;
    }
    let machine_owner = match context.owner {
        ContractProofFactOwner::Machine { machine_symbol } => Some((machine_symbol, None)),
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => Some((machine_symbol, Some(state_symbol))),
        _ => None,
    };
    let attachment = if let Some((machine_symbol, state_symbol)) = machine_owner {
        let mut machines = compilation
            .machines()
            .iter()
            .filter(|machine| machine.symbol == machine_symbol);
        let machine = machines.next()?;
        if machines.next().is_some() {
            return None;
        }
        let states = compilation.machine_states(machine);
        let state = match state_symbol {
            None => states.first()?,
            Some(symbol) => {
                let mut matches = states.iter().filter(|state| state.symbol == symbol);
                let state = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
                state
            }
        };
        if !compilation
            .state_parameters(state)
            .iter()
            .any(|candidate| candidate == parameter)
        {
            return None;
        }
        Some(machine_symbol)
    } else {
        None
    };
    (!path.head_symbol.is_valid()
        || path.head_symbol == parameter.symbol
        || Some(path.head_symbol) == attachment)
        .then_some(parameter.symbol)
}

pub(super) fn contract_point(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
) -> Result<(ProgramPoint, Option<FactOrigin>), Vec<Diagnostic>> {
    let ContractProofFactOwner::Machine { machine_symbol } = context.owner else {
        return Ok((context.point, None));
    };
    if context.point != (ProgramPoint::Machine { machine_symbol }) {
        return Err(vec![Diagnostic::error(
            "reviewed machine contract has a different declaration point",
        )]);
    }
    let checked = exactly_one(
        compilation
            .facts
            .proof
            .contract_facts
            .iter()
            .filter_map(|(_, checked)| {
                (checked.owner == context.owner && checked.fact == fact).then_some(checked)
            }),
        "contract member path",
        "checked declaration fact",
    )?;
    let origin = Some(FactOrigin::MachineContract { machine_symbol });
    if checked.kind != ContractProofFactKind::Requires {
        return Ok((context.point, origin));
    }
    let machine = exactly_one(
        compilation
            .machines()
            .iter()
            .filter(|machine| machine.symbol == machine_symbol),
        "contract member path",
        "entry owner",
    )?;
    let entry = compilation.machine_states(machine).first().ok_or_else(|| {
        vec![Diagnostic::error(
            "reviewed precondition has no canonical entry state",
        )]
    })?;
    Ok((
        ProgramPoint::State {
            machine_symbol,
            state_symbol: entry.symbol,
        },
        origin,
    ))
}
