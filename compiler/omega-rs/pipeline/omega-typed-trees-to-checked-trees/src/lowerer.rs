use crate::checks;
use crate::facts::build_check_facts;
use crate::validation::validate_typed_program;
use omega_checked_trees::CheckedTrees;

pub(crate) fn lower_typed_trees(
    program: omega_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
    // Stage-1 machine monomorphization MUST precede validation: a generic
    // machine whose value calls agree on one instantiation is substituted to a
    // concrete machine here. Validation permits unused template bodies but the
    // generic-value-call fence still rejects any emitted concrete caller whose
    // callee remains generic (an incomplete specialization).
    let mut program = program;
    // MP2b must judge the authored requirement -> selected implementation edge
    // before MP4 consumes the call-site selections and clears the template's
    // parameter list.
    crate::specialize_static_machine_calls(&mut program)?;
    omega_validation::normalize_open_index_expressions(&mut program)?;
    crate::monomorphization::refresh_closed_domain_instance_identities(&mut program)
        .map_err(|diagnostic| vec![diagnostic])?;
    // F2b: unsuffixed float literals at declared f32/f64 destinations land
    // their format on the text carrier HERE, while the tree is still mutable
    // and before both engines fork off it -- every downstream read (native
    // and interpreter) then rounds once from the spelling.
    omega_validation::land_float_literal_destinations(&mut program);
    // Named-machine result overloads are provisionally bound to the first
    // same-named symbol during early resolution. Rebind them now, after domain
    // normalization and destination typing, before validation/backend facts
    // consume the call identity.
    omega_validation::resolve_named_result_overloads(&mut program)?;
    let validated = validate_typed_program(&program)?;
    let mut facts = build_check_facts(&program, &validated.proof_plan, validated.operations);

    // MP5: specialization selection happens before checked contract plans
    // exist. Bind the selected machines' normalized contract identities now,
    // validate the recorded relation, and make those identities part of the
    // instance fingerprint used by caches and artifacts.
    crate::monomorphization::bind_specialization_contract_identities(
        &mut program,
        &facts.contract_plans,
    )?;

    checks::check_checked_facts_recording(&program, &mut facts)?;

    Ok(CheckedTrees::with_roots(program, facts))
}
