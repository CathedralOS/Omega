use super::{Served, Stop, unserved};
use crate::{TypedTrees, data::TypeParameterKind, trait_definition::{AppliedTraitScope, TraitApplicationArguments}, types::TypeReferenceHandle};
use symbols::SymbolHandle;

/// The receiving trait application a nominal callback is judged against.
pub(super) struct Receiving<'arguments> {
    pub(super) parameter: SymbolHandle,
    pub(super) requirement: SymbolHandle,
    pub(super) receiving_trait: SymbolHandle,
    pub(super) arguments: &'arguments [TypeReferenceHandle],
}

/// A nominal parameter selects its requirement on the exact receiving trait
/// application. A machine with the same signature can satisfy another trait.
/// The receiving application is read as this call instantiates it, so
/// `ElementStep<Element>` asks for `ElementStep<Command>` when the same call
/// binds `Element` to `Command`.
pub(super) fn require_nominal_selection(
    program: &TypedTrees,
    receiving: Receiving<'_>,
    chosen: SymbolHandle,
    supplying_owner: SymbolHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
) -> Served<()> {
    let Receiving {
        parameter,
        requirement,
        receiving_trait: receiving,
        arguments,
    } = receiving;
    let required = receiving_scope(program, parameter, receiving, arguments)?
        .under_invocation(
            program,
            program.symbols.get(parameter).parent,
            supplying_owner,
            bindings,
            &[],
        )
        .ok_or(unserved(
            "a nominal callback binder needs its call's static arguments",
        ))?;
    let matches = |scope: &AppliedTraitScope<'_>| scope.same_application(program, &required);
    if program.symbols.get(chosen).kind == symbols::SymbolKind::MachineParameter {
        let owner = program.symbols.get(chosen).parent;
        let formals = program.declaration_type_parameters(owner);
        let formal = formals.iter().find(|formal| formal.symbol == chosen);
        let Some(TypeParameterKind::Machine {
            contract:
                crate::source_state::data::MachineParameterContract::Nominal {
                    receiving_trait,
                    receiving_arguments,
                    requirement: supplied,
                    ..
                },
        }) = formal.map(|formal| &formal.kind)
        else {
            return Err(Stop::Rejected(format!(
                "forwarded callback `{}` has no binding to the required trait",
                program.symbols.name(chosen),
            )));
        };
        let scope = receiving_scope(program, chosen, *receiving_trait, receiving_arguments)?;
        if *supplied != requirement || !matches(&scope) {
            return Err(Stop::Rejected(format!(
                "forwarded callback `{}` has another requirement binding",
                program.symbols.name(chosen),
            )));
        }
        return Ok(());
    }
    let callback = program
        .machine_by_symbol(chosen)
        .or_else(|| {
            program
                .unique_machine_by_entry_state_symbol(chosen)
                .map(|(machine, _)| machine)
        })
        .ok_or(unserved("a nominal callback lost its selected machine"))?;
    let selected_entry = program
        .machine_states(callback)
        .first()
        .ok_or(unserved("a nominal callback has no entry state"))?;
    let owned = program
        .owned_conformance_realization(callback.symbol)
        .filter(|(_, row)| {
            row.requirement == requirement && row.realization_state == selected_entry.symbol
        })
        .and_then(|(conformance, _)| AppliedTraitScope::from_conformance(program, conformance));
    let direct = program
        .machine_trait_conformances(callback)
        .iter()
        .filter(|row| row.requirement_symbol == requirement)
        .filter_map(|row| AppliedTraitScope::from_machine_conformance(program, callback, row));
    if owned.into_iter().chain(direct).filter(matches).count() != 1 {
        return Err(Stop::Rejected(format!(
            "callback `{}` must satisfy the exact requirement of `{}` on its selected trait application",
            program.symbols.name(callback.symbol),
            program.symbols.name(parameter),
        )));
    }
    Ok(())
}

fn receiving_scope<'program>(
    program: &'program TypedTrees,
    parameter: SymbolHandle,
    receiving: SymbolHandle,
    arguments: &[TypeReferenceHandle],
) -> Served<AppliedTraitScope<'program>> {
    let declaration = program
        .trait_definition_by_symbol(receiving)
        .ok_or(unserved(
            "a nominal callback binder lost its receiving trait",
        ))?;
    AppliedTraitScope::new(
        program,
        declaration,
        TraitApplicationArguments {
            lexical_owner: program.symbols.get(parameter).parent,
            enclosing_data_applications: &[],
            arguments,
            lifetimes: &[],
        },
    )
    .ok_or(unserved(
        "a nominal callback binder needs its complete receiving application",
    ))
}
