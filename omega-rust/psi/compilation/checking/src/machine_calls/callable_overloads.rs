use diagnostics::Diagnostic;
use typed_trees::type_identity::CallableIdentityId;

/// Declaration-side half of named result-domain overload resolution.
///
/// Predicate-only result refinements collapse in the shared identity, so they
/// diagnose as duplicates. Distinct dispatch-bearing sets remain distinct and
/// are selected later from the call site's expected result type. Fixed
/// operators deliberately use their separate operand-directed validator.
pub(crate) fn validate_named_callable_overload_declarations(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_machine_overloads(program, diagnostics);
    validate_trait_requirement_overloads(program, diagnostics);
}

fn validate_machine_overloads(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen: Vec<(CallableIdentityId, symbols::SymbolHandle)> = Vec::new();
    // A target sibling shares its family's name and signature by design; each
    // target's one body per path is validated before resolution.
    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.target.is_none())
    {
        let identity = program.machine_callable_identity(machine);
        if !identity.is_valid() {
            continue;
        }
        if seen.iter().any(|(previous_identity, previous_symbol)| {
            *previous_identity == identity
                && !program
                    .symbols
                    .source_scopes_separate(*previous_symbol, machine.symbol)
        }) {
            diagnostics.push(duplicate_diagnostic(program, "machine", identity));
        } else {
            seen.push((identity, machine.symbol));
        }
    }
}

fn validate_trait_requirement_overloads(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for trait_definition in program.traits() {
        let mut seen: Vec<(CallableIdentityId, symbols::SymbolHandle)> = Vec::new();
        for requirement in program.trait_machine_signatures(trait_definition) {
            let identity =
                program.trait_requirement_callable_identity(trait_definition, requirement);
            if seen.iter().any(|(previous_identity, previous_symbol)| {
                *previous_identity == identity
                    && !program
                        .symbols
                        .source_scopes_separate(*previous_symbol, requirement.symbol)
            }) {
                diagnostics.push(duplicate_diagnostic(program, "requirement", identity));
            } else {
                seen.push((identity, requirement.symbol));
            }
        }
    }
}

fn duplicate_diagnostic(
    program: &dyn crate::CheckingInput,
    kind: &str,
    identity: CallableIdentityId,
) -> Diagnostic {
    let result_dispatch = program.callable_identity_result_dispatch(identity);
    let dispatch = if program.callable_result_dispatch_is_empty(result_dispatch) {
        "<empty>".to_owned()
    } else {
        program.callable_result_dispatch_text(result_dispatch)
    };
    Diagnostic::error(format!(
        "duplicate named {kind} overload `{}` with parameter signature `{}` and result dispatch set `{dispatch}`; predicate-only result refinements do not distinguish overloads",
        program.callable_path_text(program.callable_identity_path(identity)),
        program.callable_parameters_text(program.callable_identity_parameters(identity)),
    ))
}
