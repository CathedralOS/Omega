//! Contract entailment queries and pristine-template prevalidation.

use crate::program_validation::finish_diagnostics;
use crate::proof_contracts::contract_entailment;
use crate::proof_contracts::contract_entailment::validate_machine_contract_entailment;
use diagnostics::Diagnostic;

/// Recheck one already-resolved checked-body operator realization at a
/// compiler-internal evidence boundary. Callers must still establish the exact
/// selected operator and establish a retained checked baseline separately; this
/// reruns the contract-coverage judgment as the final semantic gate.
pub fn validate_checked_operator_realization_contract(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    contract_entailment::check_operator_contract_conformance(
        program,
        machine,
        operator,
        &mut diagnostics,
    );
    finish_diagnostics(diagnostics)
}

/// Prove machine-generic contracts before specialization. Static selections
/// must already have been checked against their declared callable contracts.
pub fn validate_generic_machine_contract_entailment(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        if program
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| {
                matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Machine { .. }
                )
            })
        {
            validate_machine_contract_entailment(
                program,
                machine,
                &mut diagnostics,
                &mut crate::ProvenContractExpressions::default(),
            );
        }
    }
    finish_diagnostics(diagnostics)
}
