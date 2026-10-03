//! Validates exact partial and nominal affine cleanup shape and custody.

use super::{
    BTreeMap, BTreeSet, BlockId, CanonicalStructuralPathSegment, MachineId, ModuleError,
    OperationKind, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralAccess,
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceKind,
    StructuralTypeId, StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, partial_affine_root_type,
    resolve_structural_path,
};
mod continuation;

pub(super) fn nominal_cleanup_contract_receiver(
    module: &TerminalModule,
    cleanup_machine: MachineId,
) -> Option<PlaceId> {
    let target = module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup_machine)?;
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
        // Executable receivers already have their authored parameter kind in
        // the proposition context. Only an absent, proof-only root needs the
        // synthetic cleanup declaration and special contract scope.
        .filter(|receiver| {
            !target
                .structural_places
                .iter()
                .any(|place| place.id == *receiver)
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

/// Reconstruct each operation's finite owned transfer before residual cleanup.
/// Calls bind the selected subtree to a callee formal; constructors bind it to
/// a declared field. Both leave the same partial root for frontier replay.
fn projected_transfers<'a>(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &'a terminal_psi::Operation,
) -> Option<Vec<&'a terminal_psi::StructuralArgument>> {
    let mut arguments = Vec::new();
    match &operation.kind {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            let projected = structural_arguments
                .iter()
                .enumerate()
                .filter(|(_, argument)| {
                    argument.access == StructuralAccess::Owned
                        && !argument.path.is_empty()
                        && partial_affine_root_type(machine, argument.place).is_some()
                })
                .collect::<Vec<_>>();
            if projected.is_empty() {
                return Some(arguments);
            }
            let callee = machines.get(callee)?;
            if !claim_transfers.is_empty()
                || callee.result != TerminalMachineResult::Unit
                || !callee.parameters.is_empty()
            {
                return None;
            }
            for (index, argument) in projected {
                let root = partial_affine_root_type(machine, argument.place)?;
                let moved_type = resolve_structural_path(module, root, &argument.path)?;
                let parameter = callee.structural_parameters.get(index)?;
                if parameter.structural_type != moved_type
                    || parameter.multiplicity != StructuralMultiplicity::Affine
                    || parameter.position != index as u32
                    || parameter.is_self
                    || parameter.access != StructuralAccess::Owned
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || (argument
                        .path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
                        && !exact_fixed_array_element_sink(callee, moved_type))
                {
                    return None;
                }
                arguments.push(argument);
            }
        }
        OperationKind::EstablishRecord { .. }
        | OperationKind::EstablishStructuralCase { .. }
        | OperationKind::EstablishStructuralArray { .. } => {
            for argument in operation.kind.constructor_arguments() {
                if argument.path.is_empty() {
                    continue;
                }
                super::structural::result_contracts::constructor_argument_signature(
                    module, machine, argument,
                )?;
                arguments.push(argument);
            }
        }
        _ => {}
    }
    Some(arguments)
}

/// Validate transfer contracts and cleanup declarations without approximating
/// current ownership by a machine-wide union of moves. The frontier replays each
/// root at each edge and verifies its exact remaining complement independently.
pub(super) fn validate_partial_affine_cleanup_shape(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    for block in &machine.blocks {
        continuation::validate(module, machine, block)?;
        let invalid = || ModuleError::InvalidPartialAffineCleanup {
            machine: machine.id,
            block: block.id,
        };
        // Repeated or ancestor-overlapping moves within one straight-line block
        // cannot describe independent transfers. A new producer occurrence starts
        // a new root; cross-block availability belongs exclusively to the frontier.
        let mut moved: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>> = BTreeMap::new();
        for operation in &block.operations {
            for argument in
                projected_transfers(module, machine, machines, operation).ok_or_else(invalid)?
            {
                let paths = moved.entry(argument.place).or_default();
                if paths
                    .iter()
                    .any(|path| path.starts_with(&argument.path) || argument.path.starts_with(path))
                {
                    return Err(invalid());
                }
                paths.insert(argument.path.clone());
            }
            if let Some(result) = operation.result.structural() {
                moved.remove(&result.place);
            }
        }
        let Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            ..
        } = &block.terminator
        else {
            continue;
        };
        let first = residual_affine_discards.first().ok_or_else(invalid)?;
        let root_type = partial_affine_root_type(machine, first.place).ok_or_else(invalid)?;
        let mut paths = BTreeSet::new();
        for residual in residual_affine_discards {
            if residual.place != first.place
                || residual.path.is_empty()
                || resolve_structural_path(module, root_type, &residual.path)
                    != Some(residual.structural_type)
                || paths.iter().any(|path: &Vec<StructuralPathSegment>| {
                    path.starts_with(&residual.path) || residual.path.starts_with(path)
                })
            {
                return Err(invalid());
            }
            paths.insert(residual.path.clone());
        }
        // Ordering, missing siblings, already moved children, other live roots,
        // and whole-root disposal are checked against this return's live frontier
        // by close_return_unit_partial_affine, not against another block's moves.
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
    // Cleanup invokes its target on this normal edge just as an operation
    // invokes a callee. The caller must cover the target's entire published
    // reach; an empty body does not erase this executable dependency.
    for block in &machine.blocks {
        for cleanup in nominal_cleanups(&block.terminator) {
            let target = machines.get(&cleanup.cleanup_machine).ok_or(
                ModuleError::InvalidNominalAffineCleanup {
                    machine: machine.id,
                    block: block.id,
                },
            )?;
            if !super::call_graph::has_finite_normal_completion(module, target.id)
                || target
                    .published_service_ceiling
                    .iter()
                    .any(|service| !machine.published_service_ceiling.contains(service))
            {
                return Err(ModuleError::InvalidNominalAffineCleanup {
                    machine: machine.id,
                    block: block.id,
                });
            }
        }
    }
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
        if !valid_nominal_cleanup_target(module, machine.id, target, cleanup) {
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
            for callee in super::call_graph::executable_callees(module, current_machine) {
                if reachable.insert(callee) {
                    pending.push(callee);
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

/// Unit and scalar returns lend the same exact owner to an ordinary hook.
/// Its declared parameter place determines custody; receiver spelling adds no authority.
pub(super) fn valid_nominal_cleanup_target(
    module: &TerminalModule,
    caller: MachineId,
    target: &TerminalMachine,
    cleanup: &terminal_psi::NominalAffineCleanup,
) -> bool {
    target.id != caller
        && target.attachment == Some(cleanup.structural_type)
        && target.result == TerminalMachineResult::Unit
        && target.parameters.is_empty()
        && target.structural_parameters.len() <= 1
        && target.structural_parameters.iter().all(|parameter| {
            parameter.position == 0
                && parameter.structural_type == cleanup.structural_type
                && matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
                && cleanup.cleanup_receiver == Some(parameter.place)
        })
        && target.contract.crash_routes.is_empty()
        && cleanup.requirement_obligations.len() == target.contract.requires.len()
        && valid_nominal_cleanup_requirements(module, target, cleanup)
}

pub(super) fn bounded_nominal_cleanup_receiver_shape(shape: &StructuralTypeShape) -> bool {
    let StructuralTypeShape::Record { fields } = shape else {
        return false;
    };
    fields.iter().all(|field| {
        !field.relevance.is_erased()
            && match field.field_type {
                StructuralFieldType::Scalar(ScalarType::Boolean)
                | StructuralFieldType::Atomic(ScalarType::Boolean) => true,
                StructuralFieldType::Scalar(ScalarType::Integer(integer))
                | StructuralFieldType::Atomic(ScalarType::Integer(integer)) => {
                    matches!(integer.bits(), 8 | 16 | 32 | 64)
                        && (!integer.is_address() || integer.bits() == 64)
                }
                StructuralFieldType::Scalar(ScalarType::IeeeFloat(_))
                | StructuralFieldType::Atomic(ScalarType::IeeeFloat(_))
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::PredicateByteSequence { .. }
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
        // The receiver may still name the hook's borrowed
        // parameter — that place doubles as the proof root — but no
        // obligations may exist without requirement clauses.
        return cleanup.requirement_obligations.is_empty();
    }

    let Some(receiver) = cleanup.cleanup_receiver else {
        return false;
    };
    // A receiver may name the target's own borrowed parameter — the
    // place the edge lends — but never any other declared structural place.
    let receiver_parameter = target.structural_parameters.iter().find(|parameter| {
        parameter.position == 0
            && parameter.structural_type == cleanup.structural_type
            && matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            )
            && parameter.place == receiver
    });
    if cleanup.requirement_obligations.len() != target.contract.requires.len()
        || module
            .machines
            .iter()
            .flat_map(|machine| &machine.structural_places)
            .any(|place| {
                place.id == receiver
                    && !(receiver_parameter.is_some_and(|parameter| {
                        place.kind
                            == StructuralPlaceKind::Parameter {
                                position: parameter.position,
                                is_self: parameter.is_self,
                            }
                    }) && target
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
            | StructuralTypeShape::FusedServiceBinding
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
