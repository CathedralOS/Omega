//! Optional whole-root structural call selection.

use crate::selection::constraints::row;
use crate::selection::shared::*;

use super::layout;

pub(super) fn build(
    function: usize,
    source: &SourceStructuralUnitFunction,
    plan: &LegalizedOperationPlan,
    layout: SelectedMicrosoftX64OwnedIndirectPairLayout,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<Option<SelectedStructuralUnitCallInstruction>, SelectedInstructionError> {
    source
        .call
        .as_ref()
        .map(|call| {
            let callee = plan
                .structural_unit_functions
                .iter()
                .find(|candidate| candidate.machine == call.callee)
                .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
            let callee_layout = layout::reconstruct(function, callee)?;
            if callee.call_plan != source.call_plan
                || callee_layout != layout
                || call.arguments.len() != 2
                || call.arguments.iter().enumerate().any(|(index, argument)| {
                    argument.semantic.access != StructuralAccess::Owned
                        || !argument.semantic.path.is_empty()
                        || argument.target.place != argument.semantic.place
                        || argument.target.access != argument.semantic.access
                        || argument.target.path != argument.semantic.path
                        || argument.target.root_structural_type
                            != source.parameters[index].semantic.structural_type
                        || argument.target.structural_type
                            != callee.parameters[index].semantic.structural_type
                        || argument.target.source_byte_offset != 0
                        || argument.target.fixed_array_length.is_some()
                        || argument.target.element_stride.is_some()
                        || argument.target.shape != source.parameters[index].target.shape
                        || argument.target.source != source.parameters[index].target.placement
                        || argument.target.destination != callee.parameters[index].target.placement
                })
            {
                return Err(SelectedInstructionError::UnsupportedSourceShape { function });
            }
            let constraint = keys
                .structural_unit_call
                .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
            let row = row(catalog, constraint)?;
            if !row.operands.is_empty() {
                return Err(SelectedInstructionError::MissingConstraint(constraint));
            }
            Ok(SelectedStructuralUnitCallInstruction {
                id: SelectedInstructionId(0),
                source: call.source.clone(),
                operation: call.operation,
                callee: call.callee,
                caller_call_plan: source.call_plan.clone(),
                callee_call_plan: callee.call_plan.clone(),
                arguments: call
                    .arguments
                    .iter()
                    .map(|argument| SelectedStructuralUnitCallArgument {
                        semantic: argument.semantic.clone(),
                        target: argument.target.clone(),
                    })
                    .collect(),
                claim_transfers: call.claim_transfers.clone(),
                requirement_obligations: call.requirement_obligations.clone(),
                crash_continuations: call.crash_continuations.clone(),
                layout,
                constraint: row.key,
                implicit_uses: row.implicit_uses.clone(),
                implicit_defs: row.implicit_defs.clone(),
                clobbers: row.clobbers.clone(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![call.operation],
                    fuel: call.fuel.clone(),
                    ..Default::default()
                },
                effect: call.effect,
                ownership: call.ownership.clone(),
            })
        })
        .transpose()
}
