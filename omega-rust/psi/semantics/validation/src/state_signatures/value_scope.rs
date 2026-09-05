//! Arrival contracts observe the target state's explicit value frontier.
//! Declaration names in domain/proposition/type positions are not value reads.

use crate::symbols::{MachineSymbols, TopLevelSymbols};
use diagnostics::Diagnostic;
use typed_trees::{TypedTrees, domain::ProofFact, machine::Machine};

pub(super) fn validate(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let machine_symbols = MachineSymbols::build(program, machine, diagnostics);
    for state in program.machine_states(machine) {
        let scope = crate::locals::StateValueScope {
            program,
            machine,
            state,
            machine_symbols: &machine_symbols,
            symbols,
            prior_statements: &[],
            context: "arrival contract",
        };
        let type_scope = crate::locals::StateValueScope {
            context: "type bound",
            ..scope
        };
        for parameter in program.state_parameters(state) {
            type_scope.type_reference(parameter.type_reference, diagnostics);
        }
        type_scope.type_reference(state.return_type, diagnostics);
        for contract in program.state_contracts(state) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => scope.expression(*expression, diagnostics),
                    ProofFact::Membership(membership) => {
                        scope.expression(membership.value, diagnostics)
                    }
                    ProofFact::Proposition(application) => {
                        for argument in program
                            .expression_table
                            .expression_handles(application.arguments)
                        {
                            scope.expression(*argument, diagnostics);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
