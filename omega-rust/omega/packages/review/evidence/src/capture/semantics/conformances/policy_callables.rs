//! Exact callable coordinates for retained conformance selections.

use super::policy_arguments::rejected;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::encoding::framed_identity;
use crate::capture::semantics::types::review_signature_type_identity_with_binders;
use crate::record::PackageReviewNominalIdentity;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

/// Caller binders under overloaded machines need the owning overload and
/// telescope ordinal. Other declaration kinds may use their nominal path only
/// when the checked parameter arena proves it denotes exactly one binder.
pub(super) fn caller_binder_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    if compilation.machines().iter().any(|machine| {
        compilation
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| parameter.symbol == symbol)
    }) {
        return callable_identity(compilation, symbol);
    }
    let nominal = nominal_identity(compilation, symbol)?;
    let mut matching = Vec::new();
    for (_, parameter) in compilation.data_type_parameters.iter() {
        if compilation.symbols.display_path(parameter.symbol, "::") == nominal.path
            && nominal_identity(compilation, parameter.symbol)? == nominal
        {
            matching.push(parameter.symbol);
        }
    }
    if matching.as_slice() != [symbol] {
        return Err(rejected(
            "a caller binder without one unambiguous declaration coordinate",
        ));
    }
    Ok(nominal)
}

pub(crate) fn callable_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let matches = compilation
        .machines()
        .iter()
        .filter_map(|machine| {
            let states = compilation.machine_states(machine);
            if machine.symbol == symbol {
                Some((machine, states.first(), None))
            } else if let Some(state) = states.iter().find(|state| state.symbol == symbol) {
                Some((machine, Some(state), None))
            } else {
                compilation
                    .machine_type_parameters(machine)
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == symbol)
                    .map(|(ordinal, _)| (machine, states.first(), Some(ordinal)))
            }
        })
        .collect::<Vec<_>>();
    let [(machine, Some(state), binder)] = matches.as_slice() else {
        return Err(rejected(
            "a callable without one exact owning machine and state",
        ));
    };
    let nominal = nominal_identity(compilation, symbol)?;
    let machine_nominal = nominal_identity(compilation, machine.symbol)?;
    if nominal.owner != machine_nominal.owner {
        return Err(rejected("a callable outside its exact machine owner"));
    }
    let overload = compilation
        .normalized_machine_overload_identity(machine)
        .ok_or_else(|| rejected("a callable without an exact overload coordinate"))?
        .identity();
    let binders = compilation
        .machine_type_parameters(machine)
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("$parameter{ordinal}")))
        .collect::<Vec<_>>();
    let type_identity = |reference| {
        review_signature_type_identity_with_binders(
            compilation,
            reference,
            &binders,
            &machine.lifetime_parameters,
        )
        .map(|identity| identity.canonical)
    };
    let mut fields = vec![machine_nominal.path, overload, nominal.path];
    fields.push(match binder {
        Some(ordinal) => format!("binder:{ordinal}"),
        None if machine.symbol == symbol => "machine".to_owned(),
        None => format!("state:{}", state.name),
    });
    for parameter in compilation.state_parameters(state) {
        fields.push(framed_identity(
            "parameter",
            &[
                type_identity(parameter.type_reference)?,
                format!(
                    "{}:{}:{}",
                    parameter.is_self, parameter.is_mutable, parameter.is_const
                ),
            ],
        ));
    }
    fields.push(if state.return_type.is_valid() {
        framed_identity("result-type", &[type_identity(state.return_type)?])
    } else {
        framed_identity("result-none", &[])
    });
    Ok(PackageReviewNominalIdentity {
        owner: nominal.owner,
        path: framed_identity("conformance-callable", &fields),
    })
}
