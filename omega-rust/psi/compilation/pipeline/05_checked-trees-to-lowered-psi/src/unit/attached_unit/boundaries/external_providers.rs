//! Retain the checked external satisfaction relation before source is released.
//! Selection and host authority remain independent, target-owned operations.

use crate::LoweringError;
use crate::machine_lowering::declarations;
use checked_trees::CheckedTrees;
use language_semantics::ExternalBindingIdentity;
use symbols::SymbolHandle;
use terminal_psi::{
    ExternalProviderConformance, ExternalProviderOwner, TerminalIdentityTableBuilder,
};
use typed_trees::machine::{Machine, resolve_satisfied_declaration};

pub(super) struct ExternalSatisfaction<'checked> {
    requirement: SymbolHandle,
    realization: &'checked Machine,
    binding: &'checked ExternalBindingIdentity,
}

/// Scan the checked declarations once, independent of target selection or the
/// number of retained boundary calls. The exact edge, not a matching signature
/// or machine spelling, associates each candidate with its requirement.
pub(super) fn collect(checked: &CheckedTrees) -> Vec<ExternalSatisfaction<'_>> {
    let typed = &checked.typed;
    let mut edges = Vec::new();
    for machine in typed.machines() {
        for conformance in typed.machine_trait_conformances(machine) {
            let Some(binding) = conformance
                .external_binding
                .and_then(|binding| typed.external_bindings.identity(binding))
            else {
                continue;
            };
            let Some(requirement) = resolve_satisfied_declaration(typed, machine, conformance)
            else {
                continue;
            };
            edges.push(ExternalSatisfaction {
                requirement: requirement.symbol(),
                realization: machine,
                binding,
            });
        }
    }
    edges
}

pub(super) fn lower(
    edges: &[ExternalSatisfaction<'_>],
    requirement: SymbolHandle,
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
) -> Result<Vec<ExternalProviderConformance>, LoweringError> {
    let mut candidates = Vec::new();
    for edge in edges.iter().filter(|edge| edge.requirement == requirement) {
        let machine = edge.realization;
        let provider = if machine.attached_data_symbol.is_valid() {
            Some(ExternalProviderOwner {
                declaration: declarations::nominal(
                    identities,
                    checked,
                    sources,
                    machine.attached_data_symbol,
                )
                .map_err(LoweringError::Unsupported)?,
                identity: identities.intern(&checked.typed.attached_data_path(machine).ok_or(
                    LoweringError::Unsupported("external provider has no attached owner identity"),
                )?),
                package: checked
                    .symbols
                    .symbol_package_identity(machine.attached_data_symbol),
            })
        } else {
            None
        };
        candidates.push(ExternalProviderConformance {
            realization: declarations::identity(identities, checked, sources, machine.symbol)
                .map_err(LoweringError::InvalidDeclarationIdentity)?,
            realization_package: checked.symbols.symbol_package_identity(machine.symbol),
            provider,
            binding: edge.binding.clone(),
        });
    }
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}
