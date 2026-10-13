//! Exact closed selection identity, independent of proof-output or execution rungs.
use symbols::SymbolHandle;
use typed_trees::typed_trees::StaticRequirementDispatch;

/// A rewritten private target is not selection authority. Join the owner's
/// full application commitment to one public requirement and one private row.
/// Callable admission, contracts and operand custody remain with the caller.
pub fn closed_requirement_dispatch_is_valid(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    target: SymbolHandle,
    dispatch: &StaticRequirementDispatch,
) -> bool {
    if dispatch.application_commitment.is_zero()
        || dispatch.application_report_fingerprint == 0
        || target != dispatch.realization_state
    {
        return false;
    }
    let Some((machine, state)) = crate::find_machine_by_entry_state(program, target) else {
        return false;
    };
    if machine.symbol != dispatch.realization_machine || state.symbol != dispatch.realization_state
    {
        return false;
    }
    let applications = typed_trees::closed_conformance_applications_of(
        &program.machine_specializations,
        owner,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
    let [application] = applications.as_slice() else {
        return false;
    };
    let Some(application_trait) = program.trait_definition_by_symbol(application.trait_definition)
    else {
        return false;
    };
    let mut pending = vec![application_trait];
    let mut visited = Vec::new();
    let mut declares = false;
    while let Some(definition) = pending.pop() {
        if visited.contains(&definition.symbol) {
            continue;
        }
        visited.push(definition.symbol);
        if definition.symbol == dispatch.declaring_trait {
            declares = true;
            break;
        }
        for requirement in program.trait_requirements(definition) {
            if let Some(parent) = program.trait_definition_by_symbol(requirement.symbol) {
                pending.push(parent);
            }
        }
    }
    if !declares {
        return false;
    }
    let Some(declaration) = program.trait_definition_by_symbol(dispatch.declaring_trait) else {
        return false;
    };
    if program
        .trait_machine_signatures(declaration)
        .iter()
        .filter(|signature| signature.symbol == dispatch.requirement)
        .count()
        != 1
    {
        return false;
    }
    let Some(selected) = program.conformance_by_symbol(application.declaration) else {
        return false;
    };
    // A requirement owns one selection, not merely one matching tuple among
    // conflicting alternatives. Check cardinality before the private identity.
    let selected_rows = selected
        .rows
        .as_slice()
        .iter()
        .filter(|row| {
            row.declaring_trait == dispatch.declaring_trait
                && row.requirement == dispatch.requirement
        })
        .collect::<Vec<_>>();
    let application_rows = application
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait == dispatch.declaring_trait
                && row.requirement == dispatch.requirement
        })
        .collect::<Vec<_>>();
    let ([selected_row], [application_row]) =
        (selected_rows.as_slice(), application_rows.as_slice())
    else {
        return false;
    };
    selected_row.realization_machine == dispatch.realization_machine
        && selected_row.realization_state == dispatch.realization_state
        && application_row.realization_machine == dispatch.realization_machine
        && application_row.realization_state == dispatch.realization_state
}
