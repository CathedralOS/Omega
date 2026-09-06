//! Optimizer module role: executable entrance. Structural Unit selection: ABI layout, optional call, then exact return.

mod call;
mod layout;

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

pub(super) fn build(
    function: usize,
    source: &SourceStructuralUnitFunction,
    plan: &LegalizedOperationPlan,
    keys: &SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralUnitFunction, SelectedInstructionError> {
    if plan.target != target::NativeTarget::uefi_x64() {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    let layout = layout::reconstruct(function, source)?;
    let call = call::build(function, source, plan, layout, keys, catalog)?;
    let return_instruction = instruction(
        SelectedInstructionId(u32::from(call.is_some())),
        SelectedInstructionKind::ReturnUnit,
        keys.return_unit,
        &[],
        SelectedInstructionProvenance {
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    Ok(SelectedStructuralUnitFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        structural_types: source.structural_types.clone(),
        abi: SelectedStructuralUnitAbi {
            recipe: SelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1,
            call_plan: source.call_plan.clone(),
            parameters: source
                .parameters
                .iter()
                .map(|parameter| SelectedStructuralUnitParameter {
                    semantic: parameter.semantic.clone(),
                    target: parameter.target.clone(),
                })
                .collect(),
            layout,
        },
        structural_places: source.structural_places.clone(),
        entry_claims: source.entry_claims.clone(),
        published_service_ceiling: source.published_service_ceiling.clone(),
        entry_block: SelectedBlockId(0),
        source_entry_block: source.entry_block,
        boundary_settlements: source.boundary_settlements.clone(),
        call,
        terminator: SelectedStructuralUnitReturn {
            instruction: return_instruction,
            psi_return_edge: source.return_edge,
            effect: source.return_effect,
            ownership: source.return_ownership.clone(),
        },
    })
}
