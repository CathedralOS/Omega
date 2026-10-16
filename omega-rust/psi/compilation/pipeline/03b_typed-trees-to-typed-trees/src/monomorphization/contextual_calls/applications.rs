//! Retain provider tuples independently of the enclosing invocation's tuple.
//! Both symbolic and closed selections refine the same applied source contract.

use super::super::{Candidate, Diagnostic, SymbolHandle, TypeReferenceHandle, TypedTrees};
use typed_trees::{
    signature::StateSignature, state::State, static_applications::StaticMachineApplicationHandle,
};

pub(super) fn retain_view(
    program: &mut TypedTrees,
    provider: &Candidate<'_>,
    invocation: StaticMachineApplicationHandle,
    ordinal: usize,
    selected: SymbolHandle,
    demand: SymbolHandle,
    expected: &StateSignature,
    caller_types: &[(SymbolHandle, TypeReferenceHandle)],
) -> Result<(), Vec<Diagnostic>> {
    let caller_lifetimes = program
        .expression_table
        .static_machine_application_lifetimes(invocation)
        .to_vec();
    program
        .prepare_applied_machine_callable(
            invocation,
            ordinal,
            provider.binding_application,
            selected,
            demand,
            expected,
            caller_types,
            &caller_lifetimes,
        )
        .map_err(|message| vec![Diagnostic::error(message)])
}

pub(super) fn retain_provider(
    program: &mut TypedTrees,
    provider: &Candidate<'_>,
    state: &State,
    invocation: StaticMachineApplicationHandle,
    lifetimes: &[typed_trees::static_applications::StaticMachineLifetimeBinding],
) -> StaticMachineApplicationHandle {
    let owner = program
        .expression_table
        .static_machine_application(invocation)
        .lexical_owner;
    let retained = program.expression_table.retain_static_machine_application(
        state.symbol,
        state.name.clone(),
        &[],
    );
    let record = program
        .expression_table
        .static_machine_application_mut(retained);
    record.lexical_owner = owner;
    record.binding_declaration = provider.template.template_symbol;
    let bindings = super::super::applications::candidate_bindings(program, provider);
    program
        .expression_table
        .set_static_machine_application_bindings(retained, &bindings);
    program
        .expression_table
        .set_static_machine_application_lifetimes(retained, lifetimes);
    retained
}
