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
    // MP2b must judge the authored requirement -> selected implementation edge
    // before MP4 consumes the call-site selections and clears the template's
    // parameter list.
    omega_validation::validate_static_machine_selections(&program)?;
    crate::monomorphization::monomorphize_generic_machine_value_calls(&mut program)?;
    // F2b: unsuffixed float literals at declared f32/f64 destinations land
    // their format on the text carrier HERE, while the tree is still mutable
    // and before both engines fork off it -- every downstream read (native
    // and interpreter) then rounds once from the spelling.
    omega_validation::land_float_literal_destinations(&mut program);
    let validated = validate_typed_program(&program)?;
    let facts = build_check_facts(&program, &validated.proof_plan, validated.effects);

    checks::check_checked_facts(&program, &facts)?;

    Ok(CheckedTrees::with_roots(program, facts))
}
