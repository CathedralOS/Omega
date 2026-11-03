use crate::CheckingInput;
use diagnostics::Diagnostic;
use typed_trees::type_identity::CallableIdentityId;

/// Declaration-side half of named result-domain overload resolution.
///
/// Predicate-only result refinements collapse in the shared identity, so they
/// diagnose as duplicates. Distinct dispatch-bearing sets remain distinct and
/// are selected later from the call site's expected result type. Fixed
/// operators deliberately use their separate operand-directed validator.
pub(crate) fn validate_named_callable_overload_declarations(
    program: &dyn CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_machine_overloads(program, diagnostics);
    validate_trait_requirement_overloads(program, diagnostics);
}

fn validate_machine_overloads(program: &dyn CheckingInput, diagnostics: &mut Vec<Diagnostic>) {
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
            diagnostics.push(duplicate_diagnostic(
                program,
                "machine",
                &lexical_machine_name(program, machine),
                identity,
            ));
        } else {
            seen.push((identity, machine.symbol));
        }
    }
}

fn validate_trait_requirement_overloads(
    program: &dyn CheckingInput,
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
                diagnostics.push(duplicate_diagnostic(
                    program,
                    "requirement",
                    &format!("{}::{}", trait_definition.name, requirement.name),
                    identity,
                ));
            } else {
                seen.push((identity, requirement.symbol));
            }
        }
    }
}

/// The authored name of one machine with its enclosing machines, such as
/// `outer::helper`. The callable identity's path is a package-qualified
/// encoding for comparison and export; a reader of the diagnostic needs the
/// name as written. Duplicates share a scope, so no module prefix is needed.
fn lexical_machine_name(
    program: &dyn CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> String {
    let mut names = vec![machine.name.as_str()];
    let mut parent = program.symbols.get(machine.symbol).parent;
    while parent.is_valid() && program.symbols.get(parent).kind == symbols::SymbolKind::Machine {
        names.push(program.symbols.name(parent));
        parent = program.symbols.get(parent).parent;
    }
    names.reverse();
    names.join("::")
}

fn duplicate_diagnostic(
    program: &dyn CheckingInput,
    kind: &str,
    name: &str,
    identity: CallableIdentityId,
) -> Diagnostic {
    let result_dispatch = program.callable_identity_result_dispatch(identity);
    let dispatch = if program.callable_result_dispatch_is_empty(result_dispatch) {
        "<empty>".to_owned()
    } else {
        program.callable_result_dispatch_text(result_dispatch)
    };
    Diagnostic::error(format!(
        "duplicate named {kind} overload `{name}` with parameter signature `{}` and result dispatch set `{dispatch}`; predicate-only result refinements do not distinguish overloads",
        program.callable_parameters_text(program.callable_identity_parameters(identity)),
    ))
}
