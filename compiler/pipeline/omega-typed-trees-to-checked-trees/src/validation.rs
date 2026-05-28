use omega_core::diagnostics::Diagnostic;
use omega_effects::EffectPlan;
use omega_proof::obligations::ProofPlan;
use omega_typed_trees::TypedTrees;

pub(crate) struct ValidatedTypedProgram<'program> {
    pub(crate) proof_plan: ProofPlan<'program>,
    pub(crate) effects: EffectPlan,
}

pub(crate) fn validate_typed_program(
    program: &TypedTrees,
) -> Result<ValidatedTypedProgram<'_>, Vec<Diagnostic>> {
    omega_validation::validate_program(program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;

    let effects = omega_effects::infer_effects(program);
    omega_validation::validate_effect_plan(program, &effects)?;

    Ok(ValidatedTypedProgram {
        proof_plan,
        effects,
    })
}
