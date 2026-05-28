use crate::checks;
use crate::facts::build_check_facts;
use crate::validation::validate_typed_program;
use omega_checked_trees::CheckedTrees;

pub(crate) fn lower_typed_trees(
    program: omega_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
    let validated = validate_typed_program(&program)?;
    let facts = build_check_facts(&program, &validated.proof_plan, validated.effects);

    checks::check_checked_facts(&program, &facts)?;

    Ok(CheckedTrees {
        typed: program,
        facts,
    })
}
