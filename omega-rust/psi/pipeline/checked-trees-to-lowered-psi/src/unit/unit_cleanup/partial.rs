//! Partial-affine cleanup lowering and maximal residual reconstruction.
use super::super::{
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, StructuralAffineDiscard, lower_structural_path,
};
use super::{
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldType, LoweringError, Multiplicity, PrimitiveType, Terminator,
    lookup_type_id, unique_unit_machine, unsupported,
};
use crate::producer_result::SourceMappedLowered;
use crate::unit::attached_unit::bodies::UnitPlans;
use crate::unit::attached_unit::{UnitClosureRequest, lower_unit_closure};
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan};

mod anonymous;
mod residuals;

pub(crate) use anonymous::validate_permissions as validate_anonymous_partial_permissions;

pub(super) fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
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
    // The general route carries ordinary bodies whose calls projected an
    // owned structural parameter — residual rows rooted at the parameter or
    // an empty complement when the projections covered it — while
    // result-local and anonymous roots keep their structural-result sources
    // on the dedicated path.
    if plan
        .operations
        .iter()
        .flat_map(|operation| operation.with_value_calls())
        .any(|operation| {
            let CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            } = operation
            else {
                return false;
            };
            structural_arguments.iter().any(|argument| {
                argument.access == CheckedStructuralAccess::Owned
                    && !argument.path.is_empty()
                    && matches!(
                        argument.source,
                        CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
                    )
            })
        })
    {
        return lower_general_partial_affine_unit_cleanup_machine(checked, partial);
    }
    let Some((return_operation, call_operations)) = plan.operations.split_last() else {
        return unsupported("partial affine Unit cleanup operation sequence drifted");
    };
    let CheckedUnitEffectOperationPlan::Complete {
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
            crate::expression_preparation::source_custody::authored_state(checked, plan.state)?;
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
    let published = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    for (_, moved_type, target_machine) in &moved_paths {
        let target = unique_unit_machine(published, *target_machine)?;
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
    // checked lane: the lane's dispatcher and shapes join the ordinary roster
    // for this closure, and a shape both lanes publish must agree.
    for shape in partial_plans {
        if published
            .structural_type(&shape.identity)
            .is_some_and(|existing| existing != shape)
        {
            return unsupported("partial affine Unit structural type conflicts with its closure");
        }
    }
    let closure = lower_unit_closure(
        checked,
        &UnitClosureRequest {
            plans: UnitPlans::with_staged(
                &checked.facts.flow.terminal_unit_effects,
                plan,
                partial_plans,
            ),
            ..UnitClosureRequest::unit(checked, plan.machine, &[plan.machine])
        },
    )?;
    let mut source_mapped = SourceMappedLowered::new(closure.lowered, closure.machine_ids)?;
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

/// The general route: residual rows rooted at one structural parameter of an
/// ordinary checked body. Unlike the dedicated lane, the body keeps its full
/// operation sequence and may carry other whole-root discards on the same
/// return edge; validation re-derives the moved paths from the checked call
/// arguments and requires every residual row to name the same parameter
/// root, matching the producer's one-partial-root rule.
fn lower_general_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    let plan = &partial.machine;
    let mut residual_roots = std::collections::BTreeSet::new();
    for residual in &partial.residual_affine_discards {
        if !checked_partial_affine_path(&residual.path) {
            return unsupported("partial affine Unit cleanup is not an exact field path");
        }
        let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            residual.source
        else {
            return unsupported("general partial affine residual is not parameter-rooted");
        };
        residual_roots.insert(parameter_index);
    }
    if residual_roots.len() > 1 {
        return unsupported("general partial affine cleanup has more than one residual root");
    }
    // A fully consumed parameter publishes no residual row: every projected
    // root is then validated as fully covered and the ordinary terminator
    // stays.
    let parameter_index = residual_roots.iter().next().copied();
    if let Some(parameter_index) = parameter_index {
        let Some(root_parameter) = plan.structural_parameters.get(parameter_index as usize) else {
            return unsupported("general partial affine residual root is not a parameter");
        };
        if root_parameter.access != CheckedStructuralAccess::Owned
            || root_parameter.multiplicity != Multiplicity::Affine
            || !root_parameter.qualifications.is_empty()
        {
            return unsupported("general partial affine residual root custody drifted");
        }
    }
    let mut moved_paths = Vec::<(&[CheckedUnitStructuralPathSegment], &str)>::new();
    let mut other_root_moves =
        std::collections::BTreeMap::<u32, Vec<(&[CheckedUnitStructuralPathSegment], &str)>>::new();
    for operation in plan
        .operations
        .iter()
        .flat_map(|operation| operation.with_value_calls())
    {
        match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    if argument.access != CheckedStructuralAccess::Owned {
                        continue;
                    }
                    if let Some(index) = argument.source_parameter_index() {
                        if argument.path.is_empty() {
                            if parameter_index == Some(index) {
                                // A residual root also passed whole is fully
                                // consumed and cannot owe a complement.
                                return unsupported(
                                    "general partial affine residual root moved whole",
                                );
                            }
                        } else if parameter_index == Some(index) {
                            moved_paths
                                .push((argument.path.as_slice(), argument.type_identity.as_str()));
                        } else {
                            other_root_moves
                                .entry(index)
                                .or_default()
                                .push((argument.path.as_slice(), argument.type_identity.as_str()));
                        }
                    } else if !argument.path.is_empty()
                        && argument
                            .source_structural_result_binding_ordinal()
                            .is_none()
                    {
                        return unsupported(
                            "general partial affine projected argument has no residual lane",
                        );
                    }
                }
            }
            CheckedUnitEffectOperationPlan::EstablishReference { source, .. } => {
                if !source.path.is_empty() && parameter_index == source.source_parameter_index() {
                    return unsupported(
                        "general partial affine residual root moved whole through a reference",
                    );
                }
            }
            _ => {}
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
    // Any other projected parameter must be fully consumed by its moves;
    // a nonempty complement would be a second residual root this terminator
    // cannot name.
    for (other_index, other_paths) in &other_root_moves {
        let Some(other_parameter) = plan.structural_parameters.get(*other_index as usize) else {
            return unsupported("general partial affine projected parameter drifted");
        };
        let other_residuals = checked_partial_affine_residuals(
            partial_plans,
            &CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: *other_index,
            },
            &other_parameter.type_identity,
            other_paths,
            usize::MAX,
        )?;
        if !other_residuals.is_empty() {
            return unsupported("general partial affine cleanup has a second residual root");
        }
    }
    if let Some(parameter_index) = parameter_index {
        let root_parameter = plan
            .structural_parameters
            .get(parameter_index as usize)
            .ok_or(LoweringError::Unsupported(
                "general partial affine residual root is not a parameter",
            ))?;
        let expected_residuals = checked_partial_affine_residuals(
            partial_plans,
            &CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
            &root_parameter.type_identity,
            &moved_paths,
            partial.residual_affine_discards.len(),
        )?;
        if partial.residual_affine_discards != expected_residuals {
            return unsupported("partial affine Unit residual field partition drifted");
        }
    }
    let published = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    for shape in partial_plans {
        if published
            .structural_type(&shape.identity)
            .is_some_and(|existing| existing != shape)
        {
            return unsupported("partial affine Unit structural type conflicts with its closure");
        }
    }
    let closure = lower_unit_closure(
        checked,
        &UnitClosureRequest {
            plans: UnitPlans::with_staged(
                &checked.facts.flow.terminal_unit_effects,
                plan,
                partial_plans,
            ),
            ..UnitClosureRequest::unit(checked, plan.machine, &[plan.machine])
        },
    )?;
    let mut source_mapped = SourceMappedLowered::new(closure.lowered, closure.machine_ids)?;
    let lowered = &mut source_mapped.terminal;
    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "partial affine Unit entry machine was not lowered",
        ))?;
    // Ordinary bodies can lower to several terminal blocks — a shared
    // continuation on a dying temporary, for example — so the residual
    // swap targets the unique ordinary return edge rather than assuming a
    // single-block shape.
    let mut return_blocks = entry
        .blocks
        .iter_mut()
        .filter(|block| matches!(block.terminator, Terminator::ReturnUnit { .. }));
    let (Some(block), None) = (return_blocks.next(), return_blocks.next()) else {
        return unsupported("partial affine Unit terminal control drifted");
    };
    let Some(parameter_index) = parameter_index else {
        // Every projected root was covered: the ordinary terminator stays.
        return Ok(source_mapped);
    };
    let root_place = entry
        .structural_parameters
        .get(parameter_index as usize)
        .map(|parameter| parameter.place)
        .ok_or(LoweringError::Unsupported(
            "general partial affine terminal root drifted",
        ))?;
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards: lowered_trivial_discards,
    } = &block.terminator
    else {
        return unsupported("partial affine Unit terminal return drifted");
    };
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
        // Whole-root discards for the other dying roots keep their place on
        // the same edge; only the residual root closes through the
        // path-sensitive list.
        trivial_affine_discards: lowered_trivial_discards.clone(),
        residual_affine_discards,
    };
    Ok(source_mapped)
}

pub(crate) fn checked_partial_affine_residuals(
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
    // Reconstruct the producer's no-cleanup scalar classification without
    // stripping the numeric restrictions needed by contract proof lowering.
    matches!(
        field_type,
        CheckedUnitStructuralFieldType::Structural { .. }
            | CheckedUnitStructuralFieldType::BoundedInteger(_)
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
