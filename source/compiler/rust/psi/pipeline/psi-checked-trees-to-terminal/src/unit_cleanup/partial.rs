//! Partial-affine cleanup lowering and maximal residual reconstruction.

use super::*;

pub(super) fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
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
    let [parameter] = plan.structural_parameters.as_slice() else {
        return unsupported("partial affine Unit cleanup requires one structural parameter");
    };
    if partial.residual_affine_discards.is_empty() {
        return unsupported("partial affine Unit cleanup requires residual actions");
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
    let mut moved_paths = Vec::<(
        &[CheckedUnitStructuralPathSegment],
        &str,
        psi_symbols::SymbolHandle,
    )>::new();
    for (operation_ordinal, operation) in call_operations.iter().enumerate() {
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
        if argument.path.is_empty()
            || argument
                .path
                .iter()
                .any(|segment| !matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
        {
            return unsupported("partial affine Unit transfer is not an exact field path");
        }
        if coordinate.statement_index
            != u32::try_from(operation_ordinal)
                .map_err(|_| LoweringError::Unsupported("partial affine call count exceeds u32"))?
            || coordinate.call_ordinal != 0
            || !claim_transfers.is_empty()
            || argument.source_parameter_index != 0
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
    if partial.residual_affine_discards.iter().any(|residual| {
        residual.path.is_empty()
            || residual
                .path
                .iter()
                .any(|segment| !matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
    }) {
        return unsupported("partial affine Unit cleanup is not an exact field path");
    }
    if parameter.position != 0
        || parameter.is_self
        || parameter.multiplicity != Multiplicity::Affine
        || !parameter.qualifications.is_empty()
        || !plan.trivial_affine_locals.is_empty()
        || !plan.entry_claims.is_empty()
        || !plan.body_qualifications.is_empty()
        || usize::try_from(*statement_index).ok() != Some(call_operations.len())
        || partial
            .residual_affine_discards
            .iter()
            .any(|residual| residual.source_parameter_index != 0)
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return unsupported("partial affine Unit cleanup signature or coordinates drifted");
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
    let source_shape = partial_plans
        .iter()
        .find(|candidate| candidate.identity == parameter.type_identity)
        .ok_or(LoweringError::Unsupported(
            "partial affine Unit parameter type is absent from its checked shapes",
        ))?;
    let CheckedUnitStructuralTypeShape::Record { fields } = &source_shape.shape else {
        return unsupported("partial affine Unit parameter is not a record");
    };
    if fields.len() < 2 {
        return unsupported("partial affine Unit record has fewer than two fields");
    }
    if fields.iter().enumerate().any(|(index, field)| {
        field.relevance.is_erased()
            || !checked_partial_affine_field_type(&field.field_type)
            || fields[..index]
                .iter()
                .any(|earlier| earlier.identity == field.identity)
    }) {
        return unsupported("partial affine Unit field path or type identity drifted");
    }
    let expected_residuals = checked_partial_affine_residuals(
        partial_plans,
        &parameter.type_identity,
        &moved_paths
            .iter()
            .map(|(path, moved_type, _)| (*path, *moved_type))
            .collect::<Vec<_>>(),
    )
    .ok_or(LoweringError::Unsupported(
        "partial affine Unit field path or type identity drifted",
    ))?;
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
    let mut lowered = lower_attached_unit_closure(&staged, plan.machine)?;
    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "partial affine Unit entry machine was not lowered",
        ))?;
    let [terminal_parameter] = entry.structural_parameters.as_slice() else {
        return unsupported("partial affine Unit terminal parameter drifted");
    };
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
                place: terminal_parameter.place,
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
    Ok(lowered)
}

fn checked_partial_affine_residuals(
    types: &[CheckedUnitStructuralTypePlan],
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    fn visit(
        types: &[CheckedUnitStructuralTypePlan],
        current_type: &str,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
        prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
        residuals: &mut Vec<CheckedUnitPartialAffineDiscardPlan>,
    ) -> Option<()> {
        if moved_paths.is_empty()
            || moved_paths.iter().any(|(path, _)| {
                !matches!(
                    path.first(),
                    Some(CheckedUnitStructuralPathSegment::Field(_))
                )
            })
        {
            return None;
        }
        let declaration = types
            .iter()
            .find(|declaration| declaration.identity == current_type)?;
        let CheckedUnitStructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        if fields.is_empty()
            || fields.iter().enumerate().any(|(index, field)| {
                field.relevance.is_erased()
                    || !checked_partial_affine_field_type(&field.field_type)
                    || fields[..index]
                        .iter()
                        .any(|earlier| earlier.identity == field.identity)
            })
        {
            return None;
        }
        let mut matched = 0_usize;
        for field in fields.iter().rev() {
            let matching = moved_paths
                .iter()
                .filter(|(path, _)| {
                    matches!(path.first(), Some(CheckedUnitStructuralPathSegment::Field(identity))
                        if identity == &field.identity)
                })
                .copied()
                .collect::<Vec<_>>();
            matched += matching.len();
            prefix.push(CheckedUnitStructuralPathSegment::Field(
                field.identity.clone(),
            ));
            let CheckedUnitStructuralFieldType::Structural { type_identity } = &field.field_type
            else {
                if !matching.is_empty() {
                    return None;
                }
                prefix.pop();
                continue;
            };
            if matching.is_empty() {
                residuals.push(CheckedUnitPartialAffineDiscardPlan {
                    source_parameter_index: 0,
                    path: prefix.clone(),
                    type_identity: type_identity.clone(),
                });
                prefix.pop();
                continue;
            }
            let whole = matching
                .iter()
                .filter(|(path, _)| path.len() == 1)
                .collect::<Vec<_>>();
            if !whole.is_empty() {
                if whole.len() != 1 || matching.len() != 1 || whole[0].1 != type_identity {
                    return None;
                }
                prefix.pop();
                continue;
            }
            let nested = matching
                .iter()
                .map(|(path, moved_type)| (&path[1..], *moved_type))
                .collect::<Vec<_>>();
            visit(types, type_identity, &nested, prefix, residuals)?;
            prefix.pop();
        }
        (matched == moved_paths.len()).then_some(())
    }

    if moved_paths.is_empty() {
        return None;
    }
    let mut residuals = Vec::new();
    visit(
        types,
        root_type,
        moved_paths,
        &mut Vec::new(),
        &mut residuals,
    )?;
    Some(residuals)
}

fn checked_partial_affine_field_type(field_type: &CheckedUnitStructuralFieldType) -> bool {
    matches!(
        field_type,
        CheckedUnitStructuralFieldType::Structural { .. }
            | CheckedUnitStructuralFieldType::ByteSequence(
                psi_checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
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
