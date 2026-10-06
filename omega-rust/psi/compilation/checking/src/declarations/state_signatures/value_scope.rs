//! Arrival contracts observe the target state's explicit value frontier.
//! Declaration names in domain/proposition/type positions are not value reads.
//! A machine's `requires` clause is its entry state's arrival contract: stage
//! 02 resolves it in the entry parameter frontier, so it answers to the same
//! scope as the entry state's own contracts. Range and proof premises read it
//! by symbol, which an out-of-scope spelling does not carry.

use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use diagnostics::Diagnostic;
use typed_trees::{
    TypedTrees, domain::ProofFact, machine::Machine, signature::SignatureContractKind,
};

pub(super) fn validate(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Retain declaration-roster diagnostics even though value bindings use
    // their explicit state identities rather than an attached namespace.
    MachineSymbols::build(program, machine, diagnostics);
    for (ordinal, state) in program.machine_states(machine).iter().enumerate() {
        let scope = crate::value_custody::locals::StateValueScope {
            program,
            machine,
            state,
            symbols,
            prior_statements: &[],
            context: "arrival contract",
        };
        let type_scope = crate::value_custody::locals::StateValueScope {
            context: "type bound",
            ..scope
        };
        for parameter in program.state_parameters(state) {
            type_scope.type_reference(parameter.type_reference, diagnostics);
        }
        type_scope.type_reference(state.return_type, diagnostics);
        let machine_requires = program
            .machine_contracts(machine)
            .iter()
            .filter(|contract| ordinal == 0 && contract.kind == SignatureContractKind::Requires);
        for contract in program
            .state_contracts(state)
            .iter()
            .chain(machine_requires)
        {
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
