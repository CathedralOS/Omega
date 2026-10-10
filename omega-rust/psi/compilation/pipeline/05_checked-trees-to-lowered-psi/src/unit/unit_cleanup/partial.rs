//! Partial-affine cleanup lowering: the ordinary closure's Unit returns carry
//! the residual complement stage 04 published for the partially moved owned
//! parameter.
use super::super::{StructuralAffineDiscard, lower_structural_path};
use super::{
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedTrees, LoweringError, Terminator,
    lookup_type_id, unsupported,
};
use crate::unit::attached_unit::bodies::UnitPlans;
use crate::unit::attached_unit::{UnitClosureRequest, lower_unit_closure};
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;

pub(super) fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    let plan = &partial.machine;
    // The lane's dispatcher joins the ordinary roster for this closure.
    let mut source_mapped = lower_unit_closure(
        checked,
        &UnitClosureRequest {
            plans: UnitPlans::with_staged(&checked.facts.flow.terminal_unit_effects, plan),
            ..UnitClosureRequest::unit(checked, plan.machine, &[plan.machine])
        },
    )?;
    // A root fully consumed by its projections publishes no residual row and
    // keeps the ordinary return edge.
    if partial.residual_affine_discards.is_empty() {
        return Ok(source_mapped);
    }
    let lowered = &mut source_mapped.terminal;
    let terminal_type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "partial affine Unit entry machine was not lowered",
        ))?;
    // Stage 04 publishes the complement of one partially moved owned
    // parameter; its structural parameters lower in plan order, so the
    // row's dense index names the entry's place.
    let residual_affine_discards = partial
        .residual_affine_discards
        .iter()
        .map(|residual| {
            let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
                residual.source
            else {
                return unsupported(
                    "partial affine residual rooted outside a structural parameter has no return-edge route",
                );
            };
            Ok(terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                StructuralAffineDiscard {
                    place: entry.structural_parameters[parameter_index as usize].place,
                    path: lower_structural_path(&residual.path)?,
                    structural_type: terminal_type_ids
                    .iter()
                    .find(|(identity, _)| {
                        identity
                            == &checked
                                .type_identity_text(residual.type_identity)
                                .unwrap_or_default()
                    })
                    .map(|(_, id)| *id)
                    .ok_or(LoweringError::Unsupported(
                        "partial affine residual lost its structural type",
                    ))?,
                },
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    // An ordinary body is one operation sequence, so every Unit return follows
    // all of its projected moves and owes the same complement. The residual
    // tail follows the whole-root schedule the return edge already carries.
    for block in &mut entry.blocks {
        if let Terminator::ReturnUnit {
            cleanup_actions, ..
        } = &mut block.terminator
        {
            cleanup_actions.extend(residual_affine_discards.iter().cloned());
        }
    }
    Ok(source_mapped)
}
