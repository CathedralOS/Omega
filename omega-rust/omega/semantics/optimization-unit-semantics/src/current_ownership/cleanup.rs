use super::*;

pub(super) fn validate_unit_cleanup_actions(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &CurrentOwnership,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let has_residual = actions
        .iter()
        .any(|action| matches!(action, TerminalAffineCleanupAction::DiscardResidual(_)));
    let has_nominal = actions
        .iter()
        .any(|action| matches!(action, TerminalAffineCleanupAction::InvokeNominal(_)));
    if has_residual && has_nominal {
        return Err(mismatch());
    }

    if has_residual {
        let first_residual = actions
            .iter()
            .position(|action| matches!(action, TerminalAffineCleanupAction::DiscardResidual(_)))
            .expect("residual action was observed");
        let roots = actions[..first_residual]
            .iter()
            .map(|action| match action {
                TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                TerminalAffineCleanupAction::DiscardResidual(_)
                | TerminalAffineCleanupAction::InvokeNominal(_) => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(mismatch)?;
        let residuals = actions[first_residual..]
            .iter()
            .map(|action| match action {
                TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard),
                TerminalAffineCleanupAction::DiscardRoot(_)
                | TerminalAffineCleanupAction::InvokeNominal(_) => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(mismatch)?;
        let root = residuals.first().ok_or_else(mismatch)?.place;
        if residuals.iter().any(|discard| discard.place != root)
            || function
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == root)
                .is_none_or(|parameter| {
                    parameter.is_self
                        || parameter.access != StructuralAccess::Owned
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                })
            || frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(root))
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == root)
        {
            return Err(mismatch());
        }
        let mut remaining = frontier.clone();
        let moved = remaining
            .partial_custody_paths
            .remove(&root)
            .ok_or_else(mismatch)?;
        let root_type = place_structural_type(function, root).ok_or_else(mismatch)?;
        let expected =
            partial_affine_residuals(structural_types, root_type, &moved, residuals.len())
                .ok_or_else(mismatch)?;
        if moved.is_empty()
            || residuals.len() != expected.len()
            || residuals
                .iter()
                .zip(expected)
                .any(|(actual, (path, structural_type))| {
                    actual.path != path || actual.structural_type != structural_type
                })
            || remaining.owned_places.remove(&root) != Some(StructuralMultiplicity::Affine)
            || roots != expected_trivial_affine_discards(function, &remaining)
            || !remaining.partial_custody_paths.is_empty()
        {
            return Err(mismatch());
        }
        return Ok(());
    }

    if has_nominal {
        let mut remaining = frontier.clone();
        for action in actions {
            let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
                return Err(mismatch());
            };
            if cleanup.cleanup_receiver.is_some()
                || !cleanup.requirement_obligations.is_empty()
                || remaining
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(cleanup.place))
                || remaining.owned_places.remove(&cleanup.place)
                    != Some(StructuralMultiplicity::Affine)
                || place_structural_type(function, cleanup.place) != Some(cleanup.structural_type)
                || !valid_nominal_cleanup(function, functions, structural_types, cleanup)
            {
                return Err(mismatch());
            }
        }
        if !remaining.partial_custody_paths.is_empty()
            || !remaining.claims.is_empty()
            || !remaining.owned_places.is_empty()
        {
            return Err(mismatch());
        }
        return Ok(());
    }

    let roots = actions
        .iter()
        .map(|action| match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
            TerminalAffineCleanupAction::DiscardResidual(_)
            | TerminalAffineCleanupAction::InvokeNominal(_) => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(mismatch)?;
    if !frontier.partial_custody_paths.is_empty()
        || roots != expected_trivial_affine_discards(function, frontier)
    {
        return Err(mismatch());
    }
    Ok(())
}

pub(super) fn validate_scalar_cleanup_actions(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &CurrentOwnership,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let mut remaining = frontier.clone();
    let mut actions = actions.iter();

    let mut locals = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if remaining.owned_places.contains_key(&place.id) => {
                Some((declaration_ordinal, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    locals.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    for (_, place) in locals {
        if actions.next() != Some(&TerminalAffineCleanupAction::DiscardRoot(place)) {
            return Err(mismatch());
        }
        remaining.owned_places.remove(&place);
    }

    for parameter in function.structural_parameters.iter().rev() {
        if !remaining.owned_places.contains_key(&parameter.place) {
            continue;
        }
        if parameter.multiplicity != StructuralMultiplicity::Affine
            || remaining
                .claims
                .values()
                .any(|claim| claim.input == Some(parameter.place))
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
        {
            return Err(mismatch());
        }
        if let Some(moved) = remaining.partial_custody_paths.remove(&parameter.place) {
            let residuals = partial_affine_residuals(
                structural_types,
                parameter.structural_type,
                &moved,
                actions.len(),
            )
            .ok_or_else(mismatch)?;
            if moved.is_empty() || residuals.is_empty() {
                return Err(mismatch());
            }
            for (path, structural_type) in residuals {
                let expected =
                    TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                        place: parameter.place,
                        path,
                        structural_type,
                    });
                if actions.next() != Some(&expected) {
                    return Err(mismatch());
                }
            }
        } else {
            let Some(action) = actions.next() else {
                return Err(mismatch());
            };
            match action {
                TerminalAffineCleanupAction::DiscardRoot(place) if *place == parameter.place => {}
                TerminalAffineCleanupAction::InvokeNominal(cleanup)
                    if cleanup.place == parameter.place
                        && cleanup.structural_type == parameter.structural_type
                        && cleanup.cleanup_receiver.is_none()
                        && cleanup.requirement_obligations.is_empty()
                        && valid_nominal_cleanup(
                            function,
                            functions,
                            structural_types,
                            cleanup,
                        ) => {}
                TerminalAffineCleanupAction::DiscardRoot(_)
                | TerminalAffineCleanupAction::DiscardResidual(_)
                | TerminalAffineCleanupAction::InvokeNominal(_) => return Err(mismatch()),
            }
        }
        remaining.owned_places.remove(&parameter.place);
    }

    if actions.next().is_some()
        || !remaining.owned_places.is_empty()
        || !remaining.partial_custody_paths.is_empty()
    {
        return Err(mismatch());
    }
    Ok(())
}

pub(super) fn valid_nominal_cleanup(
    caller: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cleanup: &terminal_psi::NominalAffineCleanup,
) -> bool {
    let Some(source) = structural_types.get(&cleanup.structural_type) else {
        return false;
    };
    let Some(target) = functions.get(&cleanup.cleanup_machine).copied() else {
        return false;
    };
    cleanup.cleanup_machine != caller.machine
        && bounded_nominal_cleanup_receiver_shape(&source.shape)
        && target.attachment == Some(cleanup.structural_type)
        && matches!(target.result, AbstractFunctionResult::Unit)
        && target.parameters.is_empty()
        && target.structural_parameters.is_empty()
        && target.structural_places.is_empty()
        && target.entry_claim_declarations.is_empty()
        && target.content_entry_claims.is_empty()
        && target.published_service_ceiling.is_empty()
        && target.verified_contract.as_ref().is_none_or(|contract| {
            contract.ensures.is_empty()
                && contract.outcome_specific_ensures.is_empty()
                && contract.crash_routes.is_empty()
        })
}

pub(super) fn bounded_nominal_cleanup_receiver_shape(shape: &StructuralTypeShape) -> bool {
    let StructuralTypeShape::Record { fields } = shape else {
        return false;
    };
    fields.iter().all(|field| {
        !field.relevance.is_erased()
            && match field.field_type {
                StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean) => true,
                StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(integer)) => {
                    matches!(integer.bits(), 8 | 16 | 32 | 64)
                        && (!integer.is_address() || integer.bits() == 64)
                }
                StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::IeeeFloat(_))
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::Structural(_)
                | StructuralFieldType::Erased { .. } => false,
            }
    })
}
