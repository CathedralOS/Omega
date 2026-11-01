//! Parameter qualifications are requirements in the selected call's type scope.
//!
//! The declaration contract already owns authored qualifications. Selecting a
//! qualified type for a formal adds obligations on the actual argument; it does
//! not add assumptions to the caller or mutate the generic declaration.

use facts::FactPayload;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use validation::{
    CallSite, call_site_argument_expressions, call_type_arguments, parameter_domain_constraints,
};

pub(crate) fn call_parameter_domains(
    program: &dyn ::validation::CheckingInput,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    site: &CallSite<'_>,
) -> Vec<(ExpressionHandle, FactPayload)> {
    let Some(caller) = program.machine_by_symbol(caller_machine) else {
        return Vec::new();
    };
    let Some(callee) = program.machine_by_symbol(target_machine) else {
        return Vec::new();
    };
    let Some(state) = program.state_in_machine(caller, caller_state) else {
        return Vec::new();
    };
    let Some(entry) = program.state_in_machine(callee, target_state) else {
        return Vec::new();
    };
    let selected = match site {
        CallSite::Statement(call) => &call.machine_arguments[..],
        CallSite::Expression { call, .. } => &call.machine_arguments[..],
        CallSite::TransitionNamed { .. } => &[],
    };
    let arguments = call_site_argument_expressions(program, site);
    let parameters = program.state_parameters(entry);
    let Some(bindings) = call_type_arguments(
        program,
        caller,
        Some(state),
        callee,
        entry,
        selected,
        arguments,
    ) else {
        // The ordinary call validator rejects an invalid static application.
        return Vec::new();
    };
    let mut requirements = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let declared = parameter_domain_constraints(program, parameter.type_reference, &[]);
        for domain in parameter_domain_constraints(program, parameter.type_reference, &bindings) {
            // Existing declaration contracts instantiate their own domain
            // arguments. Do not copy an open callee-domain identity into the
            // caller; only newly selected caller qualifications belong here.
            if declared.contains(&domain) {
                continue;
            }
            let (domain_symbol, name, _, semantic_domain) = domain;
            let payload = if domain_symbol.is_valid() {
                FactPayload::DomainMembership {
                    value: *argument,
                    domain: Default::default(),
                    domain_symbol,
                    semantic_domain,
                }
            } else if let Some(permission) = language_semantics::CarryPermission::from_name(&name) {
                FactPayload::CarryPermission {
                    value: *argument,
                    permission,
                }
            } else {
                continue;
            };
            requirements.push((*argument, payload));
        }
    }
    requirements
}
