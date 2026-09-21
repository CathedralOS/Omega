//! Validates exact partial and nominal affine cleanup shape and custody.

use super::{
    BTreeMap, BTreeSet, BlockId, CanonicalStructuralPathSegment, MachineId, ModuleError,
    OperationKind, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralAccess,
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceKind,
    StructuralTypeId, StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, is_partial_affine_path,
    partial_affine_residuals, partial_affine_root_type, resolve_structural_path,
};
mod continuation;

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
) -> Box<dyn Iterator<Item = &terminal_psi::NominalAffineCleanup> + '_> {
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
    for block in &machine.blocks {
        continuation::validate(module, machine, block, machines)?;
    }
    let field_calls = machine
        .blocks
        .iter()
        .filter(|block| !matches!(block.terminator, Terminator::Jump { .. }))
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
                        argument.access == StructuralAccess::Owned
                            && partial_affine_root_type(machine, argument.place).is_some_and(
                                |structural_type| {
                                    is_partial_affine_path(module, structural_type, &argument.path)
                                },
                            )
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
    let Some((block, _, _, arguments, _)) = field_calls.first() else {
        return Err(invalid(
            partial_returns
                .first()
                .map_or(machine.entry, |block| block.id),
        ));
    };
    let [root_argument] = arguments.as_slice() else {
        return Err(invalid(block.id));
    };
    let root_place = root_argument.place;
    let root_type =
        partial_affine_root_type(machine, root_place).ok_or_else(|| invalid(block.id))?;
    let root_parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root_place);
    let root_is_self = root_parameter.is_some_and(|parameter| parameter.is_self);
    let root_position = root_parameter.map_or(0, |parameter| parameter.position);
    let producer = if root_parameter.is_none() {
        let Some(StructuralPlaceKind::OperationResult { producer, .. }) = machine
            .structural_places
            .iter()
            .find(|place| place.id == root_place)
            .map(|place| &place.kind)
        else {
            return Err(invalid(block.id));
        };
        Some(*producer)
    } else {
        None
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
        || block.operations.len() != field_calls.len() + usize::from(producer.is_some())
        || if let Some(producer) = producer {
            machine.blocks.len() != 1
                || block.id != machine.entry
                || block
                    .operations
                    .first()
                    .is_none_or(|operation| operation.id != producer)
                || machine.structural_parameters.len() > 1
                || machine.structural_places.len() != machine.structural_parameters.len() + 1
                || machine.structural_parameters.iter().any(|parameter| {
                    parameter.is_self
                        || partial_affine_root_type(machine, parameter.place).is_none()
                })
        } else {
            machine.structural_parameters.len() != 1 || machine.structural_places.len() != 1
        }
        || !machine.entry_claims.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
    {
        return Err(invalid(block.id));
    }
    let indexed_projection = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root_type)
        .is_some_and(|declaration| {
            matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
        })
        || field_calls.iter().any(|(_, _, _, arguments, _)| {
            arguments.iter().any(|argument| {
                argument
                    .path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
            })
        });
    if root_parameter.is_some_and(|root| {
        !machine.structural_places.iter().any(|place| {
            place.id == root_place
                && place.kind
                    == StructuralPlaceKind::Parameter {
                        position: root.position,
                        is_self: root.is_self,
                    }
        })
    }) {
        return Err(invalid(block.id));
    }
    let mut moved_paths = BTreeSet::new();
    for (_, _, callee_id, arguments, claim_transfers) in &field_calls {
        let [argument] = arguments.as_slice() else {
            return Err(invalid(block.id));
        };
        if argument.place != root_place
            || argument.access != StructuralAccess::Owned
            || !moved_paths.insert(argument.path.clone())
        {
            return Err(invalid(block.id));
        }
        let Some(moved_type) = resolve_structural_path(module, root_type, &argument.path) else {
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
            || (indexed_projection && !exact_fixed_array_element_sink(callee, moved_type))
        {
            return Err(invalid(block.id));
        }
    }
    // Crashes abandon the current frontier; they never invent a disposal edge.
    if matches!(block.terminator, Terminator::Crash { .. }) {
        return Ok(());
    }
    if partial_block.is_none() {
        let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &block.terminator
        else {
            return Err(invalid(block.id));
        };
        if machine.blocks.len() != 1
            || !block.parameters.is_empty()
            || root_position != 0
            || root_is_self
            || partial_affine_residuals(module, root_type, &moved_paths, 0)
                .is_none_or(|residuals| !residuals.is_empty())
            || !trivial_affine_discards.is_empty()
            || (producer.is_none() && !machine.published_service_ceiling.is_empty())
            || !machine.contract.requires.is_empty()
            || !machine.contract.ensures.is_empty()
            || !machine.contract.crash_routes.is_empty()
        {
            return Err(invalid(block.id));
        }
        return Ok(());
    }
    let Some(expected_residuals) = partial_affine_residuals(
        module,
        root_type,
        &moved_paths,
        match &block.terminator {
            Terminator::ReturnUnitPartialAffine {
                residual_affine_discards,
                ..
            } => residual_affine_discards.len(),
            _ => 0,
        },
    ) else {
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
    if (indexed_projection
        && (machine.blocks.len() != 1
            || block.id != machine.entry
            || !block.parameters.is_empty()
            || !machine.parameters.is_empty()
            || root_position != 0
            || root_is_self
            || (producer.is_none() && !machine.published_service_ceiling.is_empty())
            || !machine.contract.requires.is_empty()
            || !machine.contract.ensures.is_empty()
            || !machine.contract.crash_routes.is_empty()))
        || expected_residuals.is_empty()
        || !trivial_affine_discards.is_empty()
        || residual_affine_discards.len() != expected_residuals.len()
        || residual_affine_discards.iter().zip(expected_residuals).any(
            |(residual, (path, structural_type))| {
                residual.place != root_place
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
    // The exact shape is what matters: one empty block, one cleanup per
    // structural parameter, empty claims and contracts. A generic consuming
    // member (`drop<T>`) carries the same terminator inside a caller's module
    // while the entry machine keeps the dispatched-cleanup shape.
    if machine.result != TerminalMachineResult::Unit
        || machine.blocks.len() != 1
        || block.id != machine.entry
        || !block.parameters.is_empty()
        || !block.operations.is_empty()
        || !machine.parameters.is_empty()
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
        // The hook body is ordinary terminal control — it is verified as an
        // ordinary machine elsewhere. What stays exact here is the selection
        // pairing: the target is owned by the consumed type, returns `Unit`,
        // takes no scalar arguments, and carries at most the borrowed `self`
        // receiver the edge lends.
        let borrowed_receiver = |parameter: &terminal_psi::StructuralParameterDeclaration| {
            parameter.is_self
                && parameter.structural_type == cleanup.structural_type
                && parameter.access != StructuralAccess::Owned
        };
        if target.id == machine.id
            || cleanup.requirement_obligations.len() != target.contract.requires.len()
            || target.attachment != Some(cleanup.structural_type)
            || target.result != TerminalMachineResult::Unit
            || !target.parameters.is_empty()
            || target.structural_parameters.len() > 1
            || target
                .structural_parameters
                .iter()
                .any(|parameter| !borrowed_receiver(parameter))
            || !target.contract.crash_routes.is_empty()
            || !valid_nominal_cleanup_requirements(module, target, cleanup)
        {
            return Err(invalid(block.id));
        }
        // A declared `self` receiver is exactly the place the edge lends —
        // `cleanup_receiver` names it. A contextual proof root without a
        // declared parameter keeps its validity-scoped identity.
        if let Some(receiver) = target.structural_parameters.first()
            && cleanup.cleanup_receiver != Some(receiver.place)
        {
            return Err(invalid(block.id));
        }
    }
    // On the dispatched entry lane the module is the transitive call closure
    // of the entry machine plus each cleanup target: whatever ordinary
    // machines the hook bodies reach are carried with them. A `drop<T>`
    // specialization member shares its caller's module, so the closure check
    // applies only to the entry.
    if machine.id == module.entry {
        let mut reachable = BTreeSet::new();
        reachable.insert(machine.id);
        reachable.extend(target_ids.iter().copied());
        let mut pending: Vec<MachineId> = reachable.iter().copied().collect();
        while let Some(current) = pending.pop() {
            let Some(current_machine) = machines.get(&current).copied() else {
                return Err(invalid(block.id));
            };
            for current_block in &current_machine.blocks {
                for operation in &current_block.operations {
                    let callee = match &operation.kind {
                        OperationKind::Call { callee, .. }
                        | OperationKind::CallUnit { callee, .. }
                        | OperationKind::CallStructuralScalar { callee, .. }
                        | OperationKind::CallStructural { callee, .. }
                        | OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
                            *callee
                        }
                        _ => continue,
                    };
                    if reachable.insert(callee) {
                        pending.push(callee);
                    }
                }
            }
        }
        if module
            .machines
            .iter()
            .any(|candidate| !reachable.contains(&candidate.id))
        {
            return Err(invalid(block.id));
        }
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
                StructuralFieldType::Scalar(ScalarType::IeeeFloat(_))
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::Structural(_)
                | StructuralFieldType::Erased { .. } => false,
            }
    })
}

pub(super) fn valid_nominal_cleanup_requirements(
    module: &TerminalModule,
    target: &TerminalMachine,
    cleanup: &terminal_psi::NominalAffineCleanup,
) -> bool {
    if target.contract.requires.is_empty() {
        // The receiver is still allowed to name the hook's borrowed `self`
        // parameter — that place doubles as the proof root — but no
        // obligations may exist without requirement clauses.
        return cleanup.requirement_obligations.is_empty();
    }

    let Some(receiver) = cleanup.cleanup_receiver else {
        return false;
    };
    // A receiver may name the target's own borrowed `self` parameter — the
    // place the edge lends — but never any other declared structural place.
    let receiver_is_self_parameter = target.structural_parameters.iter().any(|parameter| {
        parameter.is_self
            && parameter.access != StructuralAccess::Owned
            && parameter.place == receiver
    });
    if cleanup.requirement_obligations.len() != target.contract.requires.len()
        || module
            .machines
            .iter()
            .flat_map(|machine| &machine.structural_places)
            .any(|place| {
                place.id == receiver
                    && !(receiver_is_self_parameter
                        && place.kind
                            == StructuralPlaceKind::Parameter {
                                position: target
                                    .structural_parameters
                                    .first()
                                    .map_or(0, |parameter| parameter.position),
                                is_self: true,
                            }
                        && target
                            .structural_places
                            .iter()
                            .any(|candidate| candidate.id == receiver))
            })
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
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. }
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
