//! Rejoin no-code disposal eligibility before constructing path-specific cleanup.

use checked_trees::{
    CheckedSemanticDependencyKind, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitStructuralParameterPlan,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;

use crate::{LoweringError, unsupported};

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: &checked_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
) -> Result<(), LoweringError> {
    if !parameters
        .iter()
        .any(|parameter| parameter.access == CheckedStructuralAccess::Owned)
    {
        return Ok(());
    }
    // Retained executable cleanup is incompatible with this no-code route.
    // Parameter validation also checks the typed contents recursively; this
    // dependency list alone does not establish absence of nested cleanup.
    if checked
        .facts
        .flow
        .semantic_dependencies
        .iter()
        .any(|dependency| {
            dependency.consumer_machine == machine
                && dependency.kind == CheckedSemanticDependencyKind::AutomaticCleanupMachine
        })
    {
        return unsupported("owned scalar graph requires executable nominal cleanup");
    }
    let ownership = &checked.facts.flow.ownership;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let prefix_end = statements
        .iter()
        .position(|statement| {
            matches!(
                statement,
                checked_trees::statement::StatementNode::Transition(_)
            )
        })
        .unwrap_or(statements.len());
    let mut expected = Vec::new();
    for parameter in checked.state_parameters(state).iter().rev() {
        if checked.type_multiplicity(parameter.type_reference) != Multiplicity::Affine {
            continue;
        }
        let root = facts::PlaceRoot::Symbol(parameter.symbol);
        let transferred_in_prefix = ownership.permissions.iter().any(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state.symbol
                && event.root == root
                && event.kind == PermissionEventKind::Transfer
                && matches!(event.source, PermissionEventSource::Call { statement_index, .. } if statement_index < prefix_end)
        });
        if !transferred_in_prefix {
            expected.push(root);
        }
    }
    let mut actual = Vec::new();
    // Parameter origins and fresh local origins have different lifetimes.
    // Rejoin each retained local producer before excluding its ledger here;
    // bindings later checks complete statement coverage and selected cleanup.
    let mut local_roots = Vec::new();
    if let Some(retained) = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .and_then(|graph| {
            graph
                .states
                .iter()
                .find(|candidate| candidate.state == state.symbol)
        })
    {
        for operation in &retained.unit_operations {
            let checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                ..
            } = operation
            else {
                continue;
            };
            crate::attached_unit::structural_values::source_custody::validate(
                checked,
                machine,
                state.symbol,
                operation,
            )?;
            let Some(checked_trees::statement::StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                return unsupported("owned scalar graph local lost its authored declaration");
            };
            local_roots.push(facts::PlaceRoot::Symbol(local.symbol));
        }
    }
    for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine
            && event.state_symbol == state.symbol
            && event.access == PermissionAccess::Owned
            && !local_roots.contains(&event.root)
    }) {
        if event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != PermissionProvenance::Unknown
            || event.obligation_live
            || ownership
                .segments
                .span(event.segments)
                .is_none_or(|segments| !segments.is_empty())
        {
            return unsupported("owned scalar graph contains unsupported structural claims");
        }
        if event.kind == PermissionEventKind::AffineDrop
            && event.source == PermissionEventSource::StateExit
        {
            if event.multiplicity != Multiplicity::Affine {
                return unsupported("owned scalar graph disposal changes source multiplicity");
            }
            actual.push(event.root);
        }
    }
    if actual != expected {
        return unsupported(
            "owned scalar graph disposal eligibility differs from source parameters",
        );
    }
    Ok(())
}
