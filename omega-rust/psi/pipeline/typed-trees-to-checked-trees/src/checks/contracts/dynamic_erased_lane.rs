//! Refusal of erased formals on dynamic-dispatch requirements.
//!
//! A call through a `&dyn` descriptor binding cannot carry an erased-argument
//! lane: the checked dispatch lane keeps `self` arity only, and a requirement
//! whose signature declares erased formals can never receive its proof-only
//! actuals through the descriptor. Refusing here names the missing lane
//! instead of surfacing later as an unadmitted call plan.

use crate::labels::call_target_label;
use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use diagnostics::Diagnostic;

pub(super) fn check_dynamic_erased_formal_lane(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !call_flow.has_receiver || !call_flow.receiver_symbol.is_valid() {
        return;
    }
    let conformances = &facts.dynamic_conformances;
    let receiver_is_dynamic = conformances
        .binding_facts()
        .selections
        .iter()
        .any(|selection| {
            selection.machine == state_flow.machine_symbol
                && selection.state == state_flow.state_symbol
                && selection.binding == call_flow.receiver_symbol
                && selection.statement_index < call_flow.statement_index
        })
        || conformances.storages.iter().any(|storage| {
            storage.machine == state_flow.machine_symbol
                && storage.state == state_flow.state_symbol
                && storage.destination_field == call_flow.receiver_symbol
                && storage.statement_index < call_flow.statement_index
        });
    if !receiver_is_dynamic {
        return;
    }
    let Some(signature) = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .find(|signature| signature.symbol == call_flow.target_symbol)
    else {
        return;
    };
    let Some(erased) = program
        .state_signature_parameters(signature)
        .iter()
        .find(|parameter| parameter.relevance.is_erased())
    else {
        return;
    };
    let diagnostic = Diagnostic::error(format!(
        "dynamic dispatch to `{}` cannot supply erased formal `{}`; dynamic call lanes carry no erased arguments",
        call_target_label(program, call_flow.target_symbol),
        erased.name,
    ));
    diagnostics.push(if call_flow.authored_expression.is_valid() {
        diagnostic.with_source_span(
            program
                .expression_table
                .source_span(call_flow.authored_expression),
        )
    } else {
        diagnostic
    });
}
