//! Validates exact partial and nominal affine cleanup shape and custody.

use super::*;

pub(super) fn nominal_cleanup_contract_receiver(
    module: &TerminalModule,
    cleanup_machine: MachineId,
) -> Option<PlaceId> {
    module
        .machines
        .iter()
        .flat_map(|candidate| &candidate.blocks)
        .flat_map(|block| nominal_cleanups(&block.terminator))
        .find_map(|cleanup| {
            (cleanup.cleanup_machine == cleanup_machine)
                .then_some(cleanup.cleanup_receiver)
                .flatten()
        })
}

pub(super) fn nominal_cleanups(
    terminator: &Terminator,
) -> Box<dyn Iterator<Item = &psi_terminal::NominalAffineCleanup> + '_> {
    match terminator {
        Terminator::ReturnUnitNominalAffine { cleanups, .. } => Box::new(cleanups.iter()),
        Terminator::Return {
            cleanup_actions, ..
        } => Box::new(cleanup_actions.iter().filter_map(|action| match action {
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => Some(cleanup),
            TerminalAffineCleanupAction::DiscardRoot(_)
            | TerminalAffineCleanupAction::DiscardResidual(_) => None,
        })),
        _ => Box::new(std::iter::empty()),
    }
}

pub(super) fn validate_partial_affine_cleanup_shape(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let field_calls = machine
        .blocks
        .iter()
        .flat_map(|block| {
            block.operations.iter().filter_map(move |operation| {
                let OperationKind::CallUnit {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    ..
                } = &operation.kind
                else {
                    return None;
                };
                structural_arguments
                    .iter()
                    .any(|argument| {
                        machine
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == argument.place)
                            .is_some_and(|parameter| {
                                parameter.multiplicity == StructuralMultiplicity::Affine
                                    && is_bounded_partial_affine_path(
                                        module,
                                        parameter.structural_type,
                                        &argument.path,
                                    )
                            })
                    })
                    .then_some((
                        block,
                        operation,
                        *callee,
                        structural_arguments,
                        claim_transfers,
                    ))
            })
        })
        .collect::<Vec<_>>();
    let partial_returns = machine
        .blocks
        .iter()
        .filter(|block| matches!(block.terminator, Terminator::ReturnUnitPartialAffine { .. }))
        .collect::<Vec<_>>();
    if field_calls.is_empty() && partial_returns.is_empty() {
        return Ok(());
    }
    let invalid = |block: BlockId| ModuleError::InvalidPartialAffineCleanup {
        machine: machine.id,
        block,
    };
    let Some((block, ..)) = field_calls.first() else {
        return Err(invalid(
            partial_returns
                .first()
                .map_or(machine.entry, |block| block.id),
        ));
    };
    let partial_block = match partial_returns.as_slice() {
        [partial_block] => Some(*partial_block),
        [] => None,
        _ => return Err(invalid(block.id)),
    };
    if partial_block.is_some_and(|partial_block| partial_block.id != block.id)
        || field_calls
            .iter()
            .any(|(candidate, ..)| candidate.id != block.id)
        || !matches!(machine.result, TerminalMachineResult::Unit)
        || block.operations.len() != field_calls.len()
        || machine.structural_parameters.len() != 1
        || machine.structural_places.len() != 1
        || !machine.entry_claims.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
    {
        return Err(invalid(block.id));
    }
    let [root] = machine.structural_parameters.as_slice() else {
        unreachable!()
    };
    let fixed_array_root = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_type)
        .is_some_and(|declaration| {
            matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
        });
    if root.multiplicity != StructuralMultiplicity::Affine
        || !root.qualifications.is_empty()
        || !machine.structural_places.iter().any(|place| {
            place.id == root.place
                && place.kind
                    == StructuralPlaceKind::Parameter {
                        position: root.position,
                        is_self: root.is_self,
                    }
        })
    {
        return Err(invalid(block.id));
    }
    let mut moved_paths = BTreeSet::new();
    for (_, _, callee_id, arguments, claim_transfers) in &field_calls {
        let [argument] = arguments.as_slice() else {
            return Err(invalid(block.id));
        };
        if argument.place != root.place
            || argument.access != StructuralAccess::Owned
            || !moved_paths.insert(argument.path.clone())
        {
            return Err(invalid(block.id));
        }
        let Some(moved_type) =
            resolve_structural_path(module, root.structural_type, &argument.path)
        else {
            return Err(invalid(block.id));
        };
        let Some(callee) = machines.get(callee_id).copied() else {
            return Err(invalid(block.id));
        };
        let [callee_parameter] = callee.structural_parameters.as_slice() else {
            return Err(invalid(block.id));
        };
        if !claim_transfers.is_empty()
            || callee.result != TerminalMachineResult::Unit
            || !callee.parameters.is_empty()
            || callee_parameter.structural_type != moved_type
            || callee_parameter.multiplicity != StructuralMultiplicity::Affine
            || callee_parameter.position != 0
            || callee_parameter.is_self
            || callee_parameter.access != StructuralAccess::Owned
            || !callee_parameter.qualifications.is_empty()
            || (fixed_array_root && !exact_fixed_array_element_sink(callee, moved_type))
        {
            return Err(invalid(block.id));
        }
    }
    if partial_block.is_none() {
        let Some(StructuralTypeShape::FixedArray { element, length: 2 }) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == root.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(invalid(block.id));
        };
        let element_is_record = module.structural_types.iter().any(|declaration| {
            declaration.id == *element
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        });
        let expected_paths = BTreeSet::from([
            vec![StructuralPathSegment::FixedIndex(0)],
            vec![StructuralPathSegment::FixedIndex(1)],
        ]);
        let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &block.terminator
        else {
            return Err(invalid(block.id));
        };
        if machine.blocks.len() != 1
            || !block.parameters.is_empty()
            || field_calls.len() != 2
            || root.position != 0
            || root.is_self
            || root.access != StructuralAccess::Owned
            || !element_is_record
            || moved_paths != expected_paths
            || !trivial_affine_discards.is_empty()
            || !machine.published_service_ceiling.is_empty()
            || !machine.contract.requires.is_empty()
            || !machine.contract.ensures.is_empty()
            || !machine.contract.crash_routes.is_empty()
        {
            return Err(invalid(block.id));
        }
        return Ok(());
    }
    let Some(expected_residuals) =
        partial_affine_residuals(module, root.structural_type, &moved_paths)
    else {
        return Err(invalid(block.id));
    };
    let Terminator::ReturnUnitPartialAffine {
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        unreachable!()
    };
    if (fixed_array_root
        && (machine.blocks.len() != 1
            || block.id != machine.entry
            || !block.parameters.is_empty()
            || !machine.parameters.is_empty()
            || root.position != 0
            || root.is_self
            || root.access != StructuralAccess::Owned
            || !machine.published_service_ceiling.is_empty()
            || !machine.contract.requires.is_empty()
            || !machine.contract.ensures.is_empty()
            || !machine.contract.crash_routes.is_empty()))
        || expected_residuals.is_empty()
        || !trivial_affine_discards.is_empty()
        || residual_affine_discards.len() != expected_residuals.len()
        || residual_affine_discards.iter().zip(expected_residuals).any(
            |(residual, (path, structural_type))| {
                residual.place != root.place
                    || residual.path != path
                    || residual.structural_type != structural_type
            },
        )
    {
        return Err(invalid(block.id));
    }
    Ok(())
}

fn exact_fixed_array_element_sink(callee: &TerminalMachine, moved_type: StructuralTypeId) -> bool {
    let [parameter] = callee.structural_parameters.as_slice() else {
        return false;
    };
    let [place] = callee.structural_places.as_slice() else {
        return false;
    };
    let [block] = callee.blocks.as_slice() else {
        return false;
    };
    callee.result == TerminalMachineResult::Unit
        && callee.parameters.is_empty()
        && parameter.structural_type == moved_type
        && parameter.multiplicity == StructuralMultiplicity::Affine
        && parameter.position == 0
        && !parameter.is_self
        && parameter.access == StructuralAccess::Owned
        && parameter.qualifications.is_empty()
        && place.id == parameter.place
        && place.kind
            == StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            }
        && callee.entry_claims.is_empty()
        && callee.published_service_ceiling.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.content_identity_reshuffles.is_empty()
        && callee.content_partition_compositions.is_empty()
        && callee.entry == block.id
        && block.parameters.is_empty()
        && block.operations.is_empty()
        && matches!(
            &block.terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards.as_slice() == [parameter.place]
        )
        && callee.contract.requires.is_empty()
        && callee.contract.ensures.is_empty()
        && callee.contract.crash_routes.is_empty()
}

pub(super) fn validate_nominal_affine_cleanup_shape(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let nominal_returns = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::ReturnUnitNominalAffine { cleanups, .. } => Some((block, cleanups)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if nominal_returns.is_empty() {
        return Ok(());
    }
    let invalid = |block: BlockId| ModuleError::InvalidNominalAffineCleanup {
        machine: machine.id,
        block,
    };
    let [(block, cleanups)] = nominal_returns.as_slice() else {
        return Err(invalid(machine.entry));
    };
    if machine.result != TerminalMachineResult::Unit
        || module.entry != machine.id
        || machine.blocks.len() != 1
        || block.id != machine.entry
        || !block.parameters.is_empty()
        || !block.operations.is_empty()
        || machine.parameters.len() != 0
        || cleanups.is_empty()
        || machine.structural_parameters.len() != cleanups.len()
        || machine.structural_places.len() != cleanups.len()
        || !machine.entry_claims.is_empty()
        || !machine.published_service_ceiling.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
        || !machine.contract.crash_routes.is_empty()
        || !machine.contract.ensures.is_empty()
    {
        return Err(invalid(block.id));
    }
    let expected_parameters = machine
        .structural_parameters
        .iter()
        .rev()
        .collect::<Vec<_>>();
    let mut target_ids = BTreeSet::new();
    let mut helper_ids = BTreeSet::new();
    for (cleanup, parameter) in cleanups.iter().zip(expected_parameters) {
        if parameter.place != cleanup.place
            || parameter.structural_type != cleanup.structural_type
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || parameter.is_self
            || !parameter.qualifications.is_empty()
        {
            return Err(invalid(block.id));
        }
        let Some(source_type) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == cleanup.structural_type)
        else {
            return Err(invalid(block.id));
        };
        if !bounded_nominal_cleanup_receiver_shape(&source_type.shape) {
            return Err(invalid(block.id));
        }
        let Some(target) = machines.get(&cleanup.cleanup_machine).copied() else {
            return Err(invalid(block.id));
        };
        target_ids.insert(target.id);
        let [target_block] = target.blocks.as_slice() else {
            return Err(invalid(block.id));
        };
        if target.id == machine.id
            || cleanup.requirement_obligations.len() != target.contract.requires.len()
            || target.attachment != Some(cleanup.structural_type)
            || target.result != TerminalMachineResult::Unit
            || !target.parameters.is_empty()
            || !target.structural_parameters.is_empty()
            || !target.structural_places.is_empty()
            || !target.entry_claims.is_empty()
            || !target.published_service_ceiling.is_empty()
            || !target.content_entry_claims.is_empty()
            || !target.content_identity_reshuffles.is_empty()
            || !target.content_partition_compositions.is_empty()
            || target.entry != target_block.id
            || !target_block.parameters.is_empty()
            || !matches!(target_block.terminator, Terminator::ReturnUnit { ref trivial_affine_discards, .. } if trivial_affine_discards.is_empty())
            || !target.contract.crash_routes.is_empty()
            || !target.contract.ensures.is_empty()
            || !valid_nominal_cleanup_requirements(module, target, cleanup)
        {
            return Err(invalid(block.id));
        }
        let mut target_helper_ids = BTreeSet::new();
        for operation in &target_block.operations {
            let OperationKind::CallUnit {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } = &operation.kind
            else {
                return Err(invalid(block.id));
            };
            if operation.result != OperationResult::Unit
                || *callee == machine.id
                || *callee == target.id
                || cleanups
                    .iter()
                    .any(|candidate| candidate.cleanup_machine == *callee)
                || !target_helper_ids.insert(*callee)
                || !structural_arguments.is_empty()
                || !claim_transfers.is_empty()
                || !requirement_obligations.is_empty()
                || !crash_continuations.is_empty()
            {
                return Err(invalid(block.id));
            }
            helper_ids.insert(*callee);
            let Some(helper) = machines.get(callee).copied() else {
                return Err(invalid(block.id));
            };
            let Some(helper_attachment) = helper.attachment else {
                return Err(invalid(block.id));
            };
            let helper_attachment_is_empty = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == helper_attachment)
                .is_some_and(|declaration| {
                    matches!(
                        &declaration.shape,
                        StructuralTypeShape::Record { fields } if fields.is_empty()
                    )
                });
            let [helper_block] = helper.blocks.as_slice() else {
                return Err(invalid(block.id));
            };
            if !helper_attachment_is_empty
                || helper.result != TerminalMachineResult::Unit
                || !helper.parameters.is_empty()
                || !helper.structural_parameters.is_empty()
                || !helper.structural_places.is_empty()
                || !helper.entry_claims.is_empty()
                || !helper.published_service_ceiling.is_empty()
                || !helper.content_entry_claims.is_empty()
                || !helper.content_identity_reshuffles.is_empty()
                || !helper.content_partition_compositions.is_empty()
                || helper.entry != helper_block.id
                || !helper_block.parameters.is_empty()
                || !helper_block.operations.is_empty()
                || !matches!(
                    helper_block.terminator,
                    Terminator::ReturnUnit {
                        ref trivial_affine_discards,
                        ..
                    } if trivial_affine_discards.is_empty()
                )
                || !helper.contract.crash_routes.is_empty()
                || !helper.contract.requires.is_empty()
                || !helper.contract.ensures.is_empty()
            {
                return Err(invalid(block.id));
            }
        }
    }
    if module.machines.len() != 1 + target_ids.len() + helper_ids.len() {
        return Err(invalid(block.id));
    }
    Ok(())
}

pub(super) fn bounded_nominal_cleanup_receiver_shape(shape: &StructuralTypeShape) -> bool {
    let StructuralTypeShape::Record { fields } = shape else {
        return false;
    };
    fields.iter().all(|field| {
        !field.relevance.is_erased()
            && match field.field_type {
                StructuralFieldType::Scalar(ScalarType::Boolean) => true,
                StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                    matches!(integer.bits(), 8 | 16 | 32 | 64)
                        && (!integer.is_address() || integer.bits() == 64)
                }
                StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::Structural(_)
                | StructuralFieldType::Erased { .. } => false,
            }
    })
}

pub(super) fn every_scalar_return_nominally_cleans(
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    let mut saw_return = false;
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Return {
                cleanup_actions, ..
            } => {
                saw_return = true;
                if !cleanup_actions.iter().any(|action| {
                    matches!(
                        action,
                        TerminalAffineCleanupAction::InvokeNominal(cleanup)
                            if cleanup.place == source
                    )
                }) {
                    return false;
                }
            }
            Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. } => return false,
            Terminator::Jump { .. } | Terminator::Conditional { .. } | Terminator::Crash { .. } => {
            }
        }
    }
    saw_return
}

pub(super) fn valid_nominal_cleanup_requirements(
    module: &TerminalModule,
    target: &TerminalMachine,
    cleanup: &psi_terminal::NominalAffineCleanup,
) -> bool {
    if target.contract.requires.is_empty() {
        return cleanup.cleanup_receiver.is_none() && cleanup.requirement_obligations.is_empty();
    }

    let Some(receiver) = cleanup.cleanup_receiver else {
        return false;
    };
    if cleanup.requirement_obligations.len() != target.contract.requires.len()
        || module
            .machines
            .iter()
            .flat_map(|machine| &machine.structural_places)
            .any(|place| place.id == receiver)
        || module.machines.iter().any(|machine| {
            machine.blocks.iter().any(|block| {
                nominal_cleanups(&block.terminator).any(|candidate| {
                    candidate.cleanup_machine != target.id
                        && candidate.cleanup_receiver == Some(receiver)
                })
            })
        })
    {
        return false;
    }

    let Some(fields) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
        .and_then(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => Some(fields),
            StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::FixedArray { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => None,
        })
    else {
        return false;
    };
    let mut previous_key = None;
    for requirement in &target.contract.requires {
        let Proposition::Equal(
            ScalarTerm::Boolean(expected),
            ScalarTerm::BooleanField { root, path },
        ) = requirement
        else {
            return false;
        };
        let [CanonicalStructuralPathSegment::Field(field)] = path.as_slice() else {
            return false;
        };
        let key = (*expected, field.get().to_le_bytes());
        if *root != receiver
            || previous_key.is_some_and(|previous| previous >= key)
            || !fields.iter().any(|candidate| {
                candidate.id == *field
                    && !candidate.relevance.is_erased()
                    && candidate.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
            })
        {
            return false;
        }
        previous_key = Some(key);
    }
    true
}
