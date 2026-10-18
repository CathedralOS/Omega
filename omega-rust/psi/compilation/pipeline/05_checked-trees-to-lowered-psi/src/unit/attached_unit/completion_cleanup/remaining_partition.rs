//! Subtract separately admitted descriptor vacancies from checked normal death.
//! Cleanup authority and order remain in the ordinary checked action stream.
use super::{CleanupRegion, Evaluation, LoweringError, OperationBuffer};

/// The checked action owns disposition. An earlier, separately admitted
/// descriptor release owns only its exact absent partition, never a nominal
/// receiver. Subtract that partition without creating a second whole death.
pub(super) fn retain(
    evaluation: &Evaluation,
    regions: Vec<CleanupRegion>,
    types: &[terminal_psi::StructuralTypeDeclaration],
    operations: &OperationBuffer,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    use terminal_psi::TerminalAffineCleanupAction as Action;
    let mut remaining = Vec::new();
    for region in regions {
        let place = region.action.place();
        let retired = evaluation
            .selection_cleanups
            .iter()
            .flat_map(|cleanup| &cleanup.retired_reference_paths)
            .filter(|(source, _)| *source == place)
            .map(|(_, path)| path)
            .collect::<Vec<_>>();
        if retired.is_empty() {
            remaining.push(region);
            continue;
        }
        let partition = evaluation.normal_discard_regions(place, types, operations)?;
        match &region.action {
            Action::DiscardRoot(_) => {
                for mut part in partition {
                    part.declaration_order = region.declaration_order;
                    part.selected_region = region.selected_region.clone();
                    for condition in &region.conditions {
                        if !part.conditions.contains(condition) {
                            part.conditions.push(condition.clone());
                        }
                    }
                    remaining.push(part);
                }
            }
            Action::DiscardResidual(discard)
                if retired.iter().any(|path| {
                    path.starts_with(&discard.path) || discard.path.starts_with(path)
                }) =>
            {
                for part in &partition {
                    let Action::DiscardResidual(residual) = &part.action else {
                        continue;
                    };
                    if !residual.path.starts_with(&discard.path) {
                        continue;
                    }
                    let mut selected = part.clone();
                    selected.declaration_order = region.declaration_order;
                    selected.selected_region = region.selected_region.clone();
                    for condition in &region.conditions {
                        if !selected.conditions.contains(condition) {
                            selected.conditions.push(condition.clone());
                        }
                    }
                    remaining.push(selected);
                }
            }
            Action::InvokeNominal(cleanup)
                if retired.iter().any(|path| {
                    path.starts_with(&cleanup.path) || cleanup.path.starts_with(path)
                }) =>
            {
                return Err(LoweringError::Unsupported(
                    "selected complement nominal receiver was dismantled before its checked hook",
                ));
            }
            _ => remaining.push(region),
        }
    }
    Ok(remaining)
}
