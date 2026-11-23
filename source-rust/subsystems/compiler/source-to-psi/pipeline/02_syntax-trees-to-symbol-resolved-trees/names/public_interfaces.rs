/*
    Public signatures name only public types.

    `wiki/spec/language/modules.md`, "Visibility": "Public interfaces cannot
    select private declarations as consumer-nameable API." A consumer could
    call `public machine reveal(s: Secret)` but could never name `Secret` to
    build its argument, so the signature is rejected where it names `Secret`.

    The check walks the parameter and result types of every selected public
    machine once, through references, arrays, slices, constraints, generic
    arguments and alias applications, and rejects a data declaration of the
    machine's own package that is not `public`. A private declaration of
    another package cannot be selected at all. The spec's exceptions (an
    authorized public conformance's retained implementation identities, a
    public domain's issuer catalog and access descriptors) are not signature
    types and are not read here. A domain written as a constraint
    (`x: u32 in Small`) is named by spelling at this stage and is not judged
    here yet.
*/

use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::types::TypeReference;

/// Every declaration symbol a type names, outermost first.
fn named_symbols(
    program: &SymbolResolvedTrees,
    reference: &TypeReference,
    out: &mut Vec<SymbolHandle>,
) {
    match reference {
        TypeReference::Named { symbol, .. } => out.push(*symbol),
        TypeReference::Generic(application) => {
            out.push(application.base_symbol);
            for argument in program.child_type_references(application.arguments) {
                named_symbols(program, argument, out);
            }
        }
        TypeReference::Reference(reference) => {
            named_symbols(
                program,
                program.child_type_reference(reference.referee),
                out,
            );
        }
        TypeReference::FixedArray(array) => {
            named_symbols(
                program,
                program.child_type_reference(array.element_type),
                out,
            );
        }
        TypeReference::Slice(slice) => {
            named_symbols(
                program,
                program.child_type_reference(slice.element_type),
                out,
            );
        }
        TypeReference::Constrained(constrained) => {
            named_symbols(
                program,
                program.child_type_reference(constrained.base_type),
                out,
            );
        }
        TypeReference::AliasUse { application, .. } => named_symbols(program, application, out),
        TypeReference::ConstExpression(_)
        | TypeReference::Callable { .. }
        | TypeReference::Unit => {}
    }
}

pub(crate) fn reject_private_types_in_public_signatures(
    program: &SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut private = program
        .data_definitions
        .iter()
        .filter(|definition| !definition.is_public)
        .map(|definition| definition.symbol)
        .collect::<Vec<_>>();
    private.sort_by_key(|symbol| symbol.arena_index());
    let is_private = |symbol: SymbolHandle| {
        private
            .binary_search_by_key(&symbol.arena_index(), |candidate| candidate.arena_index())
            .is_ok_and(|position| private[position] == symbol)
    };
    let mut diagnostics = Vec::new();
    let mut named = Vec::new();
    for machine in program.machines.iter().filter(|machine| {
        machine.is_public && program.symbols.symbol_source_is_selected(machine.symbol)
    }) {
        // The entry state carries the machine's signature; later states are
        // its private implementation.
        for state in program.machine_state_handles(machine.states).iter().take(1) {
            let state = program.machine_state(*state);
            let types = program
                .state_parameters(state.parameters)
                .iter()
                .map(|parameter| (&parameter.type_reference, parameter.name.source_span()))
                .chain(
                    state
                        .return_type
                        .iter()
                        .map(|result| (result, machine.name.source_span())),
                );
            for (reference, at) in types {
                named.clear();
                named_symbols(program, reference, &mut named);
                for symbol in named.iter().copied().filter(|symbol| is_private(*symbol)) {
                    let declaration = program.symbols.symbol_provenance_source_span(symbol);
                    // Another package's private declaration is not selectable,
                    // and a declaration with no source is not authored API.
                    if !declaration.is_some_and(|declaration| {
                        program.symbols.same_source_package(declaration, at)
                    }) {
                        continue;
                    }
                    let name = program.symbols.display_path(symbol, "::");
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "public machine `{}` names the private data declaration `{name}` in \
                             its signature: a consumer could call it but could not name `{name}`, \
                             and public interfaces cannot select private declarations; mark \
                             `{name}` `public` or keep the machine private",
                            machine.name.as_str(),
                        ))
                        .with_source_span(at),
                    );
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
