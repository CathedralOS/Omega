//! Settle the provider values the accepted Psi program actually chooses.
//!
//! Source selection and Build-over-default precedence finish before judgment.
//! This owner consumes canonical service schemas, selected declaration values,
//! proved candidate rows and actual native execution inputs. Retained code and
//! schemas are not invocation roots; the caller supplies the reached boundaries.

mod plan_realization;
mod realized_supply;

pub use plan_realization::{
    PublishedComponentSuppliers, TargetSupplyExecutionInputs, TargetSupplyRealizationError,
    TargetSupplyRealizationRequest, realize_target_supply,
};
pub use realized_supply::{RealizedTargetSupply, SelectedProviderRealization};

mod installation_reach;
pub use installation_reach::NormalizedNativeSupply;

/// A direct leaf supplies a checked boundary when both carry the same
/// declaration and callable identities. Comparing keys lets callers sort leaves
/// or boundaries once and binary-search them, instead of comparing every pair.
type DirectLeafKey<'text> = (&'text str, &'text str);

fn direct_leaf_key(leaf: &direct_boundary_bindings::DirectBoundaryLeaf) -> DirectLeafKey<'_> {
    (&leaf.declaration_identity, &leaf.callable_identity)
}

/// The key a leaf must carry to supply this boundary declaration. A
/// declaration that lost either identity has no key, and no leaf supplies it.
fn boundary_leaf_key<'text>(
    identities: &'text terminal_psi::TerminalIdentityTable,
    kind: semantic_vocabulary::BoundaryRequirementKind,
    declaration: &terminal_psi::CheckedBoundaryCallable,
) -> Option<DirectLeafKey<'text>> {
    let declaration_identity = if kind == semantic_vocabulary::BoundaryRequirementKind::Trait
        || declaration.execution != terminal_psi::CheckedBoundaryExecution::Provider
    {
        declaration.source.semantic_declaration
    } else {
        declaration.source.machine.identity()
    };
    Some((
        identities.text(declaration_identity)?,
        identities.text(declaration.source.callable)?,
    ))
}

/// Each leaf's key with its position in `leaves`, sorted by key.
fn sorted_leaf_keys(
    leaves: &[direct_boundary_bindings::DirectBoundaryLeaf],
) -> Vec<(DirectLeafKey<'_>, usize)> {
    let mut keys = leaves
        .iter()
        .enumerate()
        .map(|(position, leaf)| (direct_leaf_key(leaf), position))
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

/// The entries of a sorted key index whose key equals `key`.
fn entries_with_key<'index, 'text>(
    sorted: &'index [(DirectLeafKey<'text>, usize)],
    key: DirectLeafKey<'_>,
) -> &'index [(DirectLeafKey<'text>, usize)] {
    let first = sorted.partition_point(|(entry, _)| *entry < key);
    let count = sorted[first..].partition_point(|(entry, _)| *entry == key);
    &sorted[first..first + count]
}
