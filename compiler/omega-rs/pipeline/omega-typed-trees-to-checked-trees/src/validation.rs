use omega_core::diagnostics::Diagnostic;
use omega_effects::OperationalPlan;
use omega_proof::obligations::ProofPlan;
use omega_typed_trees::TypedTrees;

pub(crate) struct ValidatedTypedProgram<'program> {
    pub(crate) proof_plan: ProofPlan<'program>,
    pub(crate) operations: OperationalPlan,
}

pub(crate) fn validate_typed_program(
    program: &TypedTrees,
) -> Result<ValidatedTypedProgram<'_>, Vec<Diagnostic>> {
    omega_validation::validate_program_after_generic_contract_entailment(program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;

    let operations = omega_effects::infer_operational_may(program);
    omega_validation::validate_behavior_plan(program, &operations)?;

    Ok(ValidatedTypedProgram {
        proof_plan,
        operations,
    })
}
