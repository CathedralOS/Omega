use omega_checked_trees::{CheckFacts, Program};
use omega_core::diagnostics::Diagnostic;

pub fn lower_typed_trees(program: &omega_typed_trees::Program) -> Result<Program, Vec<Diagnostic>> {
    omega_validation::validate_program(program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;

    Ok(Program {
        typed: program.clone(),
        facts: CheckFacts {
            proof_obligation_count: proof_plan.obligations.len(),
        },
    })
}

pub fn lower_typed_program(
    program: &omega_typed_trees::Program,
) -> Result<Program, Vec<Diagnostic>> {
    lower_typed_trees(program)
}
