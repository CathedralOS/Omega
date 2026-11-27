//! Source declarations reached by the entry, bindings and supplies of one compilation.

use crate::source_state::TypedTrees;
use crate::source_state::references::{
    ComputationDependencyRows, DeclarationSelection, conformance_header_dependencies,
};
use crate::source_state::trait_definition::Supply;
use checked_program_to_psi as production;
use symbols::SymbolHandle;

/// Every declaration the request reaches from its entries, selections,
/// bindings, subjects and supplies through the declarations each one names,
/// sorted. A const argument is demanded when one of these declarations
/// holds it.
///
/// Each target's layout scope adds the providers that target selected. The
/// Product request names none of them, yet lowering reads their members, so
/// a domain or constant only a selected provider uses is demanded too: a
/// Windows filesystem adapter's handle domain needs its access plan.
pub(crate) fn demanded_declarations(
    program: &TypedTrees,
    request: production::TerminalSourceCompilation<'_>,
    scopes: &[super::FixedLayoutTargetScope],
) -> Result<DeclarationSelection, String> {
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
    let mut supplies = request.supplies.to_vec();
    for scope in scopes {
        pending.extend(scope.entry_declarations.iter().copied());
        supplies.extend(scope.selected_supplies());
    }
    let dependencies = ComputationDependencyRows::of(program);
    from_supplied_roots(program, &dependencies, pending, supplies).map_err(str::to_owned)
}

impl super::FixedLayoutTargetScope {
    /// `dependencies` must be built from `program` as it is now; one
    /// inventory serves every scope of an unchanged program.
    pub(crate) fn demanded_declarations(
        &self,
        program: &TypedTrees,
        dependencies: &ComputationDependencyRows,
    ) -> Result<DeclarationSelection, &'static str> {
        from_supplied_roots(
            program,
            dependencies,
            self.entry_declarations.clone(),
            self.selected_supplies(),
        )
    }

    /// The supplies of every provider this target selected or defaulted to.
    fn selected_supplies(&self) -> impl Iterator<Item = Supply> + '_ {
        self.provider_declarations
            .iter()
            .flat_map(crate::compile_product::provider_settlement::selected_supplies)
    }
}

/// A supplied member needs its provider's type arguments and its chosen body.
/// An independent request for the entire provider still reaches every member.
pub(crate) fn from_supplied_roots(
    program: &TypedTrees,
    dependencies: &ComputationDependencyRows,
    mut roots: Vec<SymbolHandle>,
    supplies: impl IntoIterator<Item = Supply>,
) -> Result<DeclarationSelection, &'static str> {
    let mut headers = Vec::new();
    for supply in supplies {
        roots.push(supply.slot);
        if !program.symbols.symbols().nodes().is_valid(supply.slot) {
            continue;
        }
        if let Some(provider) = program.conformance_by_symbol(supply.provider) {
            if !headers.contains(&provider.symbol) {
                headers.push(provider.symbol);
                roots.extend(conformance_header_dependencies(program, provider));
            }
        } else {
            roots.push(supply.provider);
        }
        for requirement in supply.requirements(program) {
            roots.push(requirement);
            // The structural reader diagnoses missing or ambiguous realizations.
            if let Ok(machine) = supply.realization(program, requirement) {
                roots.push(machine.symbol);
            }
        }
    }
    let mut selected = DeclarationSelection::from_computation_roots(program, dependencies, roots)?;
    for header in headers {
        // Const arguments and layout occurrences belong to this header. Following
        // its ordinary declaration edges would also request every member body.
        selected.include(header);
    }
    Ok(selected)
}
