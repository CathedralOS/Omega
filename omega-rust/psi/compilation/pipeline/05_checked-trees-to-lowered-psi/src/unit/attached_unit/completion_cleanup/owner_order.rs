//! Merge checked root deaths with anonymous production chronology. A covering
//! write changes the value occupying a declaration, never its declaration order.
use super::{CleanupRegion, Evaluation, LoweringError, OperationBuffer};
use checked_trees::CheckedUnitEffectOperationPlan;
use semantic_vocabulary::PlaceId;

/// One declaring owner's cleanup may share a home with a moved field's later
/// declaration. Physical coalescing must not merge their independent schedules.
struct CleanupOwner {
    place: PlaceId,
    declaration: Option<(u8, u32)>,
    order: (u8, u32, usize),
}

/// Name the checked declaring owner before physical storage aliases can lose
/// the distinction between a replacement's supplier and receiving declaration.
pub(super) fn declaring_order(
    checked: &checked_trees::CheckedTrees,
    state: symbols::SymbolHandle,
    source: super::NormalExitSource<'_>,
    storage: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
) -> Result<Option<(u8, u32)>, LoweringError> {
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
    let named = |symbol| -> Result<_, LoweringError> {
        if let Some(local) = checked.facts.state_locals.named(state, symbol) {
            return Ok(Some((1, local.statement_index)));
        }
        let state = checked
            .typed
            .state_by_symbol(state)
            .ok_or(LoweringError::Unsupported(
                "cleanup declaring state is absent from the checked program",
            ))?;
        let position = checked
            .typed
            .state_parameters(state)
            .iter()
            .position(|parameter| parameter.symbol == symbol)
            .ok_or(LoweringError::Unsupported(
                "cleanup owner has no checked declaration",
            ))?;
        Ok(Some((
            0,
            u32::try_from(position).map_err(|_| {
                LoweringError::Unsupported(
                    "cleanup parameter declaration exceeds its checked coordinate",
                )
            })?,
        )))
    };
    match *storage {
        Source::Parameter { parameter_index } => {
            let parameter = source.parameters.get(parameter_index as usize).ok_or(
                LoweringError::Unsupported("cleanup parameter declaration is absent"),
            )?;
            Ok(Some((0, parameter.position)))
        }
        Source::StructuralLocal { symbol } => named(symbol),
        Source::CapturedStorage {
            selection,
            source_ordinal,
        } => {
            let receipt = checked.facts.flow.ownership.owned_selections.get(selection);
            if receipt.state != state {
                return Err(LoweringError::Unsupported(
                    "captured cleanup belongs to another state",
                ));
            }
            let candidate = checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .get(source_ordinal as usize)
                .ok_or(LoweringError::Unsupported(
                    "captured cleanup source declaration is absent",
                ))?;
            named(candidate.symbol)
        }
        Source::StructuralResult { binding_ordinal } => {
            let result = source
                .operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                .find(|result| result.binding_ordinal == binding_ordinal)
                .ok_or(LoweringError::Unsupported(
                    "cleanup result declaration is absent",
                ))?;
            if result.local.is_named() {
                named(result.local.symbol)
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

pub(crate) fn selection(
    state: symbols::SymbolHandle,
    planned: &[CheckedUnitEffectOperationPlan],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
    actions: &mut Vec<CleanupRegion>,
) -> Result<(), LoweringError> {
    let results = planned
        .iter()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
        .collect::<Vec<_>>();
    let mut declarations = Vec::new();
    for result in &results {
        if result.local.is_named()
            && !declarations
                .iter()
                .any(|(symbol, _)| *symbol == result.local.symbol)
        {
            declarations.push((result.local.symbol, result.statement_index));
        }
    }
    for operation in planned {
        if let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::Structural {
                symbol,
                statement_index,
                ..
            },
        ) = operation
            && !declarations.iter().any(|(declared, _)| declared == symbol)
        {
            declarations.push((*symbol, *statement_index));
        }
    }
    let mut roots = Vec::<CleanupOwner>::new();
    for action in actions.iter() {
        let place = action.action.place();
        let explicit = action.declaration_order;
        if roots
            .iter()
            .any(|root| root.place == place && root.declaration == explicit)
        {
            continue;
        }
        let parameter = evaluation
            .structural_parameters
            .iter()
            .find(|(_, declaration)| declaration.place == place);
        // Both the retained old source and the current replacement belong to
        // one named declaration. Current storage is an exact published alias;
        // an equal type or a younger producer cannot establish this relation.
        let current = evaluation
            .structural_locals
            .iter()
            .find(|local| local.argument.place == place && local.argument.path.is_empty())
            .and_then(|local| local.root.as_symbol())
            .and_then(|symbol| {
                declarations
                    .iter()
                    .find(|(declared, _)| *declared == symbol)
            })
            .map(|(_, statement)| (1, *statement, usize::MAX));
        let produced = operations
            .structural_values
            .iter()
            .find(|(_, binding)| binding.place() == place && binding.path().is_empty())
            .and_then(|(ordinal, _)| {
                results
                    .iter()
                    .find(|result| result.binding_ordinal == *ordinal)
                    .copied()
            });
        let declared_result = produced
            .filter(|result| result.local.is_named())
            .map(|result| (1, result.statement_index, usize::MAX));
        let physical = operations
            .structural_establishments
            .iter()
            .position(|root| *root == place);
        let parameter = if let Some((position, current)) = parameter {
            let materialized_ingress = explicit.is_none()
                && current.access == terminal_psi::StructuralAccess::Owned
                && evaluation.previous_parameter_homes.iter().any(|previous| {
                    previous.position == current.position
                        && previous.place != current.place
                        && previous.access == terminal_psi::StructuralAccess::SharedBorrow
                });
            if materialized_ingress {
                // Borrowed ingress owns no descriptor death. A later binding
                // adopts a fresh completed carrier; order its intrinsic expiry
                // at that receiving event, before the backing it then borrows.
                // Explicit source dispositions retain their declared order.
                let receiving = evaluation
                    .structural_value_owners
                    .iter()
                    .find(|owner| owner.value.place == place)
                    .map(|owner| owner.statement)
                    .or_else(|| produced.map(|result| result.statement_index))
                    .or(operations.structural_materialization_statement(place, state)?)
                    .ok_or(LoweringError::Unsupported(
                        "rebound borrowed formal has no completed receiving occurrence",
                    ))?;
                Some((
                    1,
                    receiving,
                    physical.ok_or(LoweringError::Unsupported(
                        "rebound borrowed formal has no materialized descriptor",
                    ))?,
                ))
            } else {
                Some((0, *position, 0))
            }
        } else {
            None
        };
        let declared_order = explicit
            .map(|(kind, coordinate)| (kind, coordinate, if kind == 0 { 0 } else { usize::MAX }))
            .or(parameter)
            .or(current)
            .or(declared_result);
        let order = if let Some(order) = declared_order {
            order
        } else {
            // Multiplicity and a surviving source alias do not describe where
            // a home was born. Nested plain backing has neither an affine owner
            // nor an authored result ordinal, but retains its materialization.
            operations
                .structural_materialization_statement(place, state)?
                .zip(physical)
                .map(|(statement, ordinal)| (1, statement, ordinal))
                .ok_or(LoweringError::Unsupported(
                    "normal cleanup owner has no checked declaration or completed producer coordinate",
                ))?
        };
        roots.push(CleanupOwner {
            place,
            declaration: explicit,
            order,
        });
    }
    // Stable ties preserve the checked per-declaration schedule: the current
    // occupant dies before its older retained candidates. Anonymous siblings
    // at one statement retain reverse actual establishment order. Each owner's
    // parent/descendant action sequence remains intact, even when another
    // declaration owns a moved field in the same physical home.
    roots.sort_by_key(|root| std::cmp::Reverse(root.order));
    let previous = std::mem::take(actions);
    for root in roots {
        actions.extend(
            previous
                .iter()
                .filter(|region| {
                    region.action.place() == root.place
                        && region.declaration_order == root.declaration
                })
                .cloned(),
        );
    }
    Ok(())
}
