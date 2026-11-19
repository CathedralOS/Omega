//! Source declarations reached by the entry, bindings and supplies of one compilation.

use crate::source_state::TypedTrees;

/// Every declaration the request reaches from its entries, selections,
/// bindings, subjects and supplies through the declarations each one names,
/// sorted. A const argument is demanded when one of these declarations
/// holds it.
pub(crate) fn demanded_declarations(
    program: &TypedTrees,
    request: crate::production::TerminalSourceCompilation<'_>,
) -> Result<crate::source_state::references::DeclarationSelection, String> {
    let mut pending = request
        .entries
        .iter()
        .chain(request.selected)
        .copied()
        .collect::<Vec<_>>();
    for binding in request.bindings {
        pending.extend([
            binding.implementation,
            binding.semantic_schema_symbol,
            binding.requirement_symbol,
        ]);
    }
    if !request.subjects.is_empty() {
        for symbol in crate::source_state::references::collect_program_declarations(program) {
            if crate::source_state::source_subjects::contains(
                program,
                symbol,
                request.subjects,
                request.purpose,
            )? {
                pending.push(symbol);
            }
        }
    }
    // Supply bodies are roots of this request too. Match their genuine typed
    // conformance rows; structural judgment still checks exact slot coverage
    // and refinement, so this nomination cannot create a supplied proof.
    for supply in request.supplies {
        pending.extend([supply.slot, supply.provider]);
        let requirements = if let Some(owner) = program.trait_definition_by_symbol(supply.slot) {
            program
                .trait_machine_signatures(owner)
                .iter()
                .map(|row| row.symbol)
                .collect::<Vec<_>>()
        } else {
            vec![supply.slot]
        };
        for machine in program.machines() {
            if crate::source_state::provider_declarations::satisfaction_provider_declaration(
                program, machine,
            ) == Some(supply.provider)
                && (program
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|row| requirements.contains(&row.requirement_symbol))
                    || program
                        .owned_conformance_realization(machine.symbol)
                        .is_some_and(|(_, row)| requirements.contains(&row.requirement)))
            {
                pending.push(machine.symbol);
            }
        }
    }
    crate::source_state::references::DeclarationSelection::from_computation_roots(program, pending)
        .map_err(str::to_owned)
}
