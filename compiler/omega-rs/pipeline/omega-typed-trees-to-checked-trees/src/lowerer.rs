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
    // F2b: unsuffixed float literals at declared f32/f64 destinations land
    // their format on the text carrier HERE, while the tree is still mutable
    // and before both engines fork off it -- every downstream read (native
    // and interpreter) then rounds once from the spelling.
    omega_validation::land_float_literal_destinations(&mut program);
    checks::termination::inherit_requirement_guarantees(&mut program);
    checks::termination::elaborate_canonical_ranking_views(&mut program);
    let validated = validate_typed_program(&program)?;
    let facts = build_check_facts(&program, &validated.proof_plan, validated.effects);

    checks::check_checked_facts(&program, &facts)?;

    let termination_summaries =
        checks::termination::checked_termination_summaries(&program);

    Ok(CheckedTrees::with_termination_summaries(
        program,
        facts,
        termination_summaries,
    ))
}
