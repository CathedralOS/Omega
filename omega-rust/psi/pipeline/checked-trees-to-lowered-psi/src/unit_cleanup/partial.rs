//! Partial-affine cleanup lowering and maximal residual reconstruction.

use super::*;
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan};

mod anonymous;
mod residuals;

pub(super) fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<crate::machine_dispatch::SourceMappedLowered, LoweringError> {
    let plan = &partial.machine;
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .any(|candidate| candidate.machine == plan.machine)
    {
        return unsupported("partial affine Unit machine is also published in the root-only lane");
    }
    let Some((return_operation, call_operations)) = plan.operations.split_last() else {
        return unsupported("partial affine Unit cleanup operation sequence drifted");
    };
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index,
        trivial_affine_local_discard_ordinals,
        trivial_affine_discards,
    } = return_operation
    else {
        return unsupported("partial affine Unit cleanup operation sequence drifted");
    };
    if call_operations.is_empty() {
        return unsupported("partial affine Unit cleanup requires projected calls");
    }
    let (root_source, root_type, producer_count, anonymous) = match &call_operations[0] {
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            result,
            discard_result_on_return,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            result,
            discard_result_on_return,
            ..
        } => {
            if coordinate.statement_index != 0
                || coordinate.call_ordinal > 1
                || (coordinate.call_ordinal == 1 && call_operations.len() != 2)
                || result.binding_ordinal != 0
                || result.statement_index != 0
                || result.multiplicity != Multiplicity::Affine
                || *discard_result_on_return
                || plan.structural_parameters.len() > 1
            {
                return unsupported("partial affine result producer custody drifted");
            }
            (
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                },
                result.type_identity.as_str(),
                1,
                coordinate.call_ordinal == 1,
            )
        }
        _ => {
            let [parameter] = plan.structural_parameters.as_slice() else {
                return unsupported(
                    "partial affine Unit cleanup requires one structural parameter",
                );
            };
            (
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
                parameter.type_identity.as_str(),
                0,
                false,
            )
        }
    };
    let projected_calls = &call_operations[producer_count..];
    if projected_calls.is_empty() {
        return unsupported("partial affine Unit cleanup requires projected calls");
    }
    let mut moved_paths = Vec::<(
        &[CheckedUnitStructuralPathSegment],
        &str,
        symbols::SymbolHandle,
    )>::new();
    for (operation_ordinal, operation) in projected_calls.iter().enumerate() {
        let CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            structural_arguments,
            claim_transfers,
            ..
        } = operation
        else {
            return unsupported("partial affine Unit cleanup operation sequence drifted");
        };
        let [argument] = structural_arguments.as_slice() else {
            return unsupported("partial affine Unit cleanup requires one structural argument");
        };
        if !checked_partial_affine_path(&argument.path) {
            return unsupported("partial affine Unit transfer is not an exact field path");
        }
        if coordinate.statement_index
            != u32::try_from(operation_ordinal + producer_count - usize::from(anonymous))
                .map_err(|_| LoweringError::Unsupported("partial affine call count exceeds u32"))?
            || coordinate.call_ordinal != 0
            || !claim_transfers.is_empty()
            || argument.source != root_source
            || argument.access != CheckedStructuralAccess::Owned
            || moved_paths.iter().any(|(earlier, _, _)| {
                earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
            })
        {
            return unsupported("partial affine Unit cleanup signature or coordinates drifted");
        }
        moved_paths.push((
            argument.path.as_slice(),
            argument.type_identity.as_str(),
            *target_machine,
        ));
    }
    if partial
        .residual_affine_discards
        .iter()
        .any(|residual| !checked_partial_affine_path(&residual.path))
    {
        return unsupported("partial affine Unit cleanup is not an exact field path");
    }
    if plan.structural_parameters.iter().any(|parameter| {
        parameter.position != 0
            || parameter.is_self
            || parameter.access != CheckedStructuralAccess::Owned
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
    }) || !plan.scalar_parameters.is_empty()
        || !plan.trivial_affine_locals.is_empty()
        || !plan.entry_claims.is_empty()
        || !plan.body_qualifications.is_empty()
        || usize::try_from(*statement_index).ok()
            != Some(call_operations.len() - usize::from(anonymous))
        || partial
            .residual_affine_discards
            .iter()
            .any(|residual| residual.source != root_source)
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return unsupported("partial affine Unit cleanup signature or coordinates drifted");
    }
    if anonymous {
        anonymous::validate(checked, partial)?;
        let (source_machine, source_state) =
            crate::scalar_source_custody::authored_state(checked, plan.state)?;
        if source_machine.symbol != plan.machine
            || checked
                .statement_table
                .statements(source_state.statement_nodes)
                .len()
                != 1
        {
            return unsupported(
                "anonymous projected result must die at its enclosing call continuation",
            );
        }
    }

    let partial_plans = &checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types;
    if partial_plans
        .iter()
        .any(|candidate| candidate.identity.is_empty())
        || partial_plans.iter().enumerate().any(|(index, candidate)| {
            partial_plans[..index]
                .iter()
                .any(|earlier| earlier.identity == candidate.identity)
        })
    {
        return unsupported("partial affine Unit structural types are empty or duplicated");
    }
    let expected_residuals = checked_partial_affine_residuals(
        partial_plans,
        &root_source,
        root_type,
        &moved_paths
            .iter()
            .map(|(path, moved_type, _)| (*path, *moved_type))
            .collect::<Vec<_>>(),
        partial.residual_affine_discards.len(),
    )?;
    if partial.residual_affine_discards != expected_residuals {
        return unsupported("partial affine Unit residual field partition drifted");
    }
    for (_, moved_type, target_machine) in &moved_paths {
        let target =
            unique_unit_machine(&checked.facts.flow.terminal_unit_effects, *target_machine)?;
        let [target_parameter] = target.structural_parameters.as_slice() else {
            return unsupported("partial affine Unit target signature drifted");
        };
        if target_parameter.type_identity != *moved_type
            || target_parameter.access != CheckedStructuralAccess::Owned
            || !target.scalar_parameters.is_empty()
            || target_parameter.is_self
            || target_parameter.multiplicity != Multiplicity::Affine
            || !target_parameter.qualifications.is_empty()
        {
            return unsupported("partial affine Unit target parameter drifted");
        }
    }

    // Reuse the ordinary closure lowerer only after validating the separate
    // checked lane. The staged copy is local producer state; no compatibility
    // or alternate artifact path escapes this function.
    let mut staged = checked.clone();
    let staged_unit = &mut staged.facts.flow.terminal_unit_effects;
    for shape in partial_plans {
        match staged_unit
            .structural_types
            .iter()
            .find(|candidate| candidate.identity == shape.identity)
        {
            Some(existing) if existing != shape => {
                return unsupported(
                    "partial affine Unit structural type conflicts with its closure",
                );
            }
            Some(_) => {}
            None => staged_unit.structural_types.push(shape.clone()),
        }
    }
    staged_unit.machines.push(plan.clone());
    let mut source_mapped = lower_unit_effect_closure(&staged, plan.machine)?;
    let lowered = &mut source_mapped.terminal;
    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "partial affine Unit entry machine was not lowered",
        ))?;
    let root_place = match &root_source {
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => entry
            .structural_parameters
            .get(*parameter_index as usize)
            .map(|parameter| parameter.place),
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. } => entry
            .blocks
            .first()
            .and_then(|block| block.operations.first())
            .and_then(|operation| operation.result.structural())
            .map(|result| result.place),
        _ => None,
    }
    .ok_or(LoweringError::Unsupported(
        "partial affine Unit terminal root drifted",
    ))?;
    let [block] = entry.blocks.as_mut_slice() else {
        return unsupported("partial affine Unit terminal control drifted");
    };
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards: lowered_trivial_discards,
    } = &block.terminator
    else {
        return unsupported("partial affine Unit terminal return drifted");
    };
    if !lowered_trivial_discards.is_empty() {
        return unsupported("partial affine Unit return acquired root-only cleanup");
    }
    if partial.residual_affine_discards.is_empty() {
        return Ok(source_mapped);
    }
    let terminal_type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let residual_affine_discards = partial
        .residual_affine_discards
        .iter()
        .map(|residual| {
            Ok(StructuralAffineDiscard {
                place: root_place,
                path: lower_structural_path(&residual.path),
                structural_type: lookup_type_id(&terminal_type_ids, &residual.type_identity)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    block.terminator = Terminator::ReturnUnitPartialAffine {
        edge: *edge,
        trivial_affine_discards: Vec::new(),
        residual_affine_discards,
    };
    Ok(source_mapped)
}

fn checked_partial_affine_residuals(
    types: &[CheckedUnitStructuralTypePlan],
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    max_residuals: usize,
) -> Result<Vec<CheckedUnitPartialAffineDiscardPlan>, LoweringError> {
    residuals::reconstruct(types, source, root_type, moved_paths, max_residuals)
}

fn checked_partial_affine_path(path: &[CheckedUnitStructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| {
            matches!(
                segment,
                CheckedUnitStructuralPathSegment::Field(_)
                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
            )
        })
}

fn checked_partial_affine_field_type(field_type: &CheckedUnitStructuralFieldType) -> bool {
    matches!(
        field_type,
        CheckedUnitStructuralFieldType::Structural { .. }
            | CheckedUnitStructuralFieldType::ByteSequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
            )
            | CheckedUnitStructuralFieldType::Scalar(
                PrimitiveType::Bool
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::Addr
                    | PrimitiveType::F32
                    | PrimitiveType::F64
            )
    )
}
