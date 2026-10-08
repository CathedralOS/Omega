//! Statement-call landing follows retained target symbols, never overload names.

use super::FloatLandingPair;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::statement::TableCall;

pub(super) fn collect(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    call: &TableCall,
    pairs: &mut Vec<FloatLandingPair>,
) {
    if !call.target_symbol.is_valid() {
        return;
    }
    let Some(parameters) = parameters(program, machine, call) else {
        return;
    };
    let parameters = parameters.iter();
    let arguments = program.statement_table.expression_handles(call.arguments);
    for (parameter, argument) in parameters.zip(arguments) {
        if parameter.type_reference.is_valid() {
            pairs.push((*argument, parameter.type_reference));
        }
    }
}

fn parameters<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    call: &TableCall,
) -> Option<&'program [StateParameter]> {
    if let Some(signature) = program.machine_parameter_signature_in(machine, call.target_symbol) {
        return Some(program.state_signature_parameters(signature));
    }
    for candidate in program.machines() {
        if let Some(state) = program.state_in_machine(candidate, call.target_symbol) {
            return Some(program.state_parameters(state));
        }
    }
    for trait_definition in program.traits() {
        if let Some(signature) =
            program.trait_machine_signature_by_symbol(trait_definition, call.target_symbol)
        {
            return Some(program.state_signature_parameters(signature));
        }
    }
    program
        .all_operators()
        .find(|operator| operator.symbol == call.target_symbol)
        .map(|operator| program.operator_parameters(operator))
}
