//! Join a sealed semantic leaf to its supplier's ordinary Build-native row.
//!
//! Catalogue meaning is independent from the physical mechanism. A foreign
//! import does not admit a hosted syscall bundle, and a catalogue declaration
//! does not supply a locator, provider choice or execution authority. Keep the
//! original native receipt and requirement schema when projecting its callable.

use super::provenance_replay::{DerivedProviderPlan, ProviderRowOrigin};
use checked_trees::typed_trees::{TypedTrees, machine::Machine};
use effects::provider_plan::ProviderBinding;

pub(crate) struct NativeCatalogueBacking<'a> {
    pub candidate: &'a DerivedProviderPlan,
    pub row: usize,
}

pub(crate) fn resolve<'a>(
    typed: &TypedTrees,
    machine: &Machine,
    target: Option<&str>,
    candidates: &'a [DerivedProviderPlan],
) -> Result<Option<NativeCatalogueBacking<'a>>, String> {
    let Some(catalogue) = typed.hosted_intrinsic_declaration(machine.symbol) else {
        return Ok(None);
    };
    // An already-derived fixed catalogue mechanism remains that declaration's
    // supply. A native choice for its public requirement does not replace the
    // physical meaning of an explicitly called private catalogue declaration.
    if candidates.iter().any(|candidate| {
        candidate
            .provenance
            .row_realizations
            .iter()
            .any(|origin| *origin == ProviderRowOrigin::Machine(machine.symbol))
    }) {
        return Ok(None);
    }
    let supplier = typed.symbols.symbol_package_identity(machine.symbol);
    let mut matches = candidates.iter().flat_map(|candidate| {
        candidate
            .provenance
            .row_requirements
            .iter()
            .enumerate()
            .filter_map(move |(row, requirement)| {
                (target == Some(candidate.plan.target.as_str())
                    && candidate.plan.origin_package_identity == supplier
                    && *requirement == catalogue.requirement.symbol
                    && matches!(
                        candidate.provenance.row_realizations.get(row),
                        Some(ProviderRowOrigin::BuildNative { .. })
                    )
                    && candidate.plan.rows.get(row).is_some_and(|row| {
                        matches!(
                            row.binding,
                            ProviderBinding::Import { .. } | ProviderBinding::Syscall { .. }
                        )
                    }))
                .then_some(NativeCatalogueBacking { candidate, row })
            })
    });
    let first = matches.next();
    if matches.next().is_some() {
        return Err("sealed catalogue leaf has multiple exact supplier-native backing rows".into());
    }
    Ok(first)
}
