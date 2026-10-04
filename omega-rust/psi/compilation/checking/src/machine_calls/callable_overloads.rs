use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::type_identity::NormalizedNamedCallableIdentity;

/// Declaration-side half of named result-domain overload resolution.
///
/// Predicate-only result refinements collapse in the shared identity, so they
/// diagnose as duplicates. Distinct dispatch-bearing sets remain distinct and
/// are selected later from the call site's expected result type. Fixed
/// operators deliberately use their separate operand-directed validator.
pub(crate) fn validate_named_callable_overload_declarations(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_machine_overloads(program, diagnostics);
    validate_trait_requirement_overloads(program, diagnostics);
}

fn validate_machine_overloads(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let mut seen: Vec<(NormalizedNamedCallableIdentity, symbols::SymbolHandle)> = Vec::new();
    // A target sibling shares its family's name and signature by design; each
    // target's one body per path is validated before resolution.
    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.target.is_none())
    {
        let Some(identity) = program.normalized_machine_overload_identity(machine) else {
            continue;
        };
        if seen.iter().any(|(previous_identity, previous_symbol)| {
            previous_identity == &identity
                && !program
                    .symbols
                    .source_scopes_separate(*previous_symbol, machine.symbol)
        }) {
            diagnostics.push(duplicate_diagnostic("machine", &identity));
        } else {
            seen.push((identity, machine.symbol));
        }
    }
}

fn validate_trait_requirement_overloads(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for trait_definition in program.traits() {
        let mut seen: Vec<(NormalizedNamedCallableIdentity, symbols::SymbolHandle)> = Vec::new();
        for requirement in program.trait_machine_signatures(trait_definition) {
            let identity = program
                .normalized_trait_requirement_overload_identity(trait_definition, requirement);
            if seen.iter().any(|(previous_identity, previous_symbol)| {
                previous_identity == &identity
                    && !program
                        .symbols
                        .source_scopes_separate(*previous_symbol, requirement.symbol)
            }) {
                diagnostics.push(duplicate_diagnostic("requirement", &identity));
            } else {
                seen.push((identity, requirement.symbol));
            }
        }
    }
}

fn duplicate_diagnostic(kind: &str, identity: &NormalizedNamedCallableIdentity) -> Diagnostic {
    let dispatch = if identity.result_dispatch().is_empty() {
        "<empty>".to_owned()
    } else {
        identity.result_dispatch().identity()
    };
    Diagnostic::error(format!(
        "duplicate named {kind} overload `{}` with parameter signature `{}` and result dispatch set `{dispatch}`; predicate-only result refinements do not distinguish overloads",
        identity.path(),
        identity.parameters(),
    ))
}
