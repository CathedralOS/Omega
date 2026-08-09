use psi_diagnostics::Diagnostic;
use psi_effects::OperationalPlan;
use psi_proof::obligations::ProofPlan;
use psi_typed_trees::TypedTrees;

pub(crate) struct ValidatedTypedProgram<'program> {
    pub(crate) proof_plan: ProofPlan<'program>,
    pub(crate) operational: OperationalPlan,
    pub(crate) validation_facts: psi_validation::ProgramValidationFacts,
}

pub(crate) fn validate_typed_program(
    program: &TypedTrees,
) -> Result<ValidatedTypedProgram<'_>, Vec<Diagnostic>> {
    let validation_facts =
        psi_validation::validate_program_after_generic_contract_entailment_with_facts(program)?;

    let proof_plan = psi_proof::obligations::build_proof_plan(program);
    psi_proof::checker::check_proof_plan(&proof_plan)?;

    let operational = psi_effects::infer_operational_may(program);
    psi_validation::validate_behavior_plan(program, &operational)?;
    crate::call_acknowledgements::validate_call_acknowledgements(program, &operational)?;

    Ok(ValidatedTypedProgram {
        proof_plan,
        operational,
        validation_facts,
    })
}
