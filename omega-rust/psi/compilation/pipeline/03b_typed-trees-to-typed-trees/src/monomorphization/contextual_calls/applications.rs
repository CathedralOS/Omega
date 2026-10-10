//! Retain provider tuples independently of the enclosing invocation's tuple.
//! Both symbolic and closed selections refine the same applied source contract.

use super::super::{Candidate, Diagnostic, SymbolHandle, TypeReferenceHandle, TypedTrees};
use typed_trees::{
    signature::StateSignature, state::State, static_applications::StaticMachineApplicationHandle,
};

pub(super) fn retain_view(
    program: &mut TypedTrees,
    provider: &Candidate<'_>,
    state: &State,
    invocation: StaticMachineApplicationHandle,
    ordinal: usize,
    selected: SymbolHandle,
    expected: &StateSignature,
    caller_types: &[(SymbolHandle, TypeReferenceHandle)],
) -> Result<(), Vec<Diagnostic>> {
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
    let bindings = super::super::applications::candidate_bindings(provider);
    program
        .expression_table
        .set_static_machine_application_bindings(retained, &bindings);
    program
        .prepare_applied_machine_callable(
            invocation,
            ordinal,
            retained,
            selected,
            expected,
            caller_types,
        )
        .map_err(|message| vec![Diagnostic::error(message)])
}
