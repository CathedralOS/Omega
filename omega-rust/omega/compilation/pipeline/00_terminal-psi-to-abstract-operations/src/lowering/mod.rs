//! Terminal-to-abstract lowering: consume the selected native work domain and
//! geometry, lower its machines operation by operation, and retain canonical
//! identity. Explicit artifact inspection instead lowers the complete catalog.

use abstract_operations::AbstractOperationPlan;
use terminal_codec::terminal_psi_identity;
mod error;
mod machine;

pub use error::LoweringError;

use machine::lower_machine;
use terminal_psi::TerminalModule;

/// Lower a decoded module with its already installed executable entry,
/// retaining canonical identity and all control-flow/operation relationships.
pub(crate) fn lower_decoded_module(
    module: &TerminalModule,
    entry: semantic_vocabulary::MachineId,
) -> Result<AbstractOperationPlan, LoweringError> {
    lower_selected_module(module, entry, module.structural_types.clone(), None)
}

/// Consume the invocation's work domain and geometry before lowering any body.
/// The unselected artifact inspection route supplies no scope or replacements.
pub(crate) fn lower_selected_module(
    module: &TerminalModule,
    entry: semantic_vocabulary::MachineId,
    structural_types: Vec<terminal_psi::StructuralTypeDeclaration>,
    scope: Option<&crate::native_execution_scope::NativeExecutionScope>,
) -> Result<AbstractOperationPlan, LoweringError> {
    // Boundary crash contracts ride through `boundary_machines` unchanged.
    // Terminal verification already covered every caller continuation against
    // them; the realized provider body or builtin settlement commits the trap
    // natively, so no abstract operation needs a separate continuation lane.
    if !module.machine(entry).is_some() {
        return Err(LoweringError::VerifiedEntryMachineMissing(entry));
    }
    let functions = module
        .machines
        .iter()
        .filter(|machine| scope.is_none_or(|scope| scope.contains_machine(machine.id)))
        .map(|machine| lower_machine(module, machine, &structural_types))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AbstractOperationPlan {
        psi: terminal_psi_identity(module).map_err(LoweringError::SemanticIdentity)?,
        entry,
        structural_types: abstract_operations::StructuralTypeCatalog::new(
            structural_types,
            module.structural_referents.clone(),
        )
        .with_placed_views(module.placed_view_plans.clone())
        .with_placed_view_inputs(module.placed_view_inputs.clone())
        .with_placement_lifecycles(module.placement_lifecycle_plans.clone())
        .map_err(LoweringError::SemanticIdentity)?
        .with_quotients(module.quotients.clone())
        .map_err(LoweringError::SemanticIdentity)?,
        boundary_machines: module
            .boundary_machines
            .iter()
            .filter(|boundary| scope.is_none_or(|scope| scope.contains_boundary(boundary.id)))
            .cloned()
            .collect(),
        provider_candidates: module
            .provider_candidates
            .iter()
            .enumerate()
            .filter(|(position, _)| {
                scope.is_none_or(|scope| scope.contains_candidate_position(*position))
            })
            .map(|(_, candidate)| candidate.clone())
            .collect(),
        dynamic_dispatch: module.dynamic_dispatch.clone(),
        identities: module.identities.clone(),
        functions,
    })
}
