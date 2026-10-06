//! Validates exact partial affine and owner-authorized nominal cleanup custody.

use super::{
    BTreeMap, BTreeSet, CanonicalStructuralPathSegment, MachineId, ModuleError, OperationKind,
    PlaceId, Proposition, ScalarTerm, ScalarType, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralPathSegment, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, partial_affine_root_type, resolve_structural_path,
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

pub(crate) fn nominal_cleanups(
    terminator: &Terminator,
) -> Box<dyn Iterator<Item = &terminal_psi::NominalAffineCleanup> + '_> {
    match terminator {
        Terminator::Return {
            cleanup_actions, ..
        }
        | Terminator::ReturnUnit {
            cleanup_actions, ..
        }
        | Terminator::ReturnStructural {
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
        // The common ordered return-action frontier validates every residual
        // against this exact exit, including mixed roots and nominal actions.
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
        && parameter.access == StructuralAccess::Owned
        && parameter.qualifications.is_empty()
        && place.id == parameter.place
        && place.kind == StructuralPlaceKind::Parameter { position: 0 }
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
                cleanup_actions,
                ..
            } if cleanup_actions.as_slice() == [TerminalAffineCleanupAction::DiscardRoot(parameter.place)]
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
    if let Some(subject) = machine.nominal_cleanup_subject {
        if machine.attachment != Some(subject)
            || !module
                .structural_types
                .iter()
                .any(|declaration| declaration.id == subject)
            || module
                .machines
                .iter()
                .filter(|candidate| candidate.nominal_cleanup_subject == Some(subject))
                .count()
                != 1
        {
            return Err(ModuleError::InvalidNominalAffineCleanup {
                machine: machine.id,
                block: machine.entry,
            });
        }
    }
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
            if !valid_nominal_cleanup_target(module, machine.id, target, cleanup)
                || !super::call_graph::has_finite_normal_completion(module, target.id)
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
        && target.nominal_cleanup_subject == Some(cleanup.structural_type)
        && (cleanup.consumed_claims.is_empty() || target.published_service_ceiling.is_empty())
        && module
            .machines
            .iter()
            .filter(|candidate| candidate.nominal_cleanup_subject == Some(cleanup.structural_type))
            .count()
            == 1
        && cleanup
            .consumed_claims
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && target.entry_claims.is_empty()
        && target.content_entry_claims.is_empty()
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
