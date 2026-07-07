use crate::checks;
use crate::facts::build_check_facts;
use crate::validation::validate_typed_program;
use omega_checked_trees::CheckedTrees;

pub(crate) fn lower_typed_trees(
    program: omega_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
    // Stage-1 machine monomorphization MUST precede validation: a generic
    // machine whose value calls agree on one instantiation is substituted to a
    // concrete machine here, so the generic-value-call fence in validation only
    // rejects the machines that remain generic (uninferable or conflicting).
    let mut program = program;
    crate::monomorphization::monomorphize_generic_machine_value_calls(&mut program);
    let validated = validate_typed_program(&program)?;
    let facts = build_check_facts(&program, &validated.proof_plan, validated.effects);

    checks::check_checked_facts(&program, &facts)?;

    Ok(CheckedTrees::with_roots(program, facts))
}
