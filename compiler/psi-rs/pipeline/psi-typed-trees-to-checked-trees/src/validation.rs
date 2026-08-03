use psi_diagnostics::Diagnostic;
use psi_effects::OperationalPlan;
use psi_proof::obligations::ProofPlan;
use psi_typed_trees::TypedTrees;

pub(crate) struct ValidatedTypedProgram<'program> {
    pub(crate) proof_plan: ProofPlan<'program>,
    pub(crate) operations: OperationalPlan,
}

pub(crate) fn validate_typed_program(
    program: &TypedTrees,
) -> Result<ValidatedTypedProgram<'_>, Vec<Diagnostic>> {
    psi_validation::validate_program_after_generic_contract_entailment(program)?;

    let proof_plan = psi_proof::obligations::build_proof_plan(program);
    psi_proof::checker::check_proof_plan(&proof_plan)?;

    let operations = psi_effects::infer_operational_may(program);
    psi_validation::validate_behavior_plan(program, &operations)?;
    crate::call_acknowledgements::validate_call_acknowledgements(program, &operations)?;

    Ok(ValidatedTypedProgram {
        proof_plan,
        operations,
    })
}
