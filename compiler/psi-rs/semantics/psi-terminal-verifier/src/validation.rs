use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentConservation, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, EvidenceTermId, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionContext, PropositionId, ScalarTerm,
    ScalarType, ServiceId, StructuralDomainId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId, content_conservation_fingerprint,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, ContentPartitionComposition,
    CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, EntryClaim, EvidenceContractLaneKind,
    OperationKind, OperationResult, PropositionBinderArgumentKind, PropositionBinderKind,
    PropositionEvidence, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator,
};

use crate::verification::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

mod call_graph;
mod content;
mod contracts;
mod control_flow;
mod error;
mod evidence;
mod foundation;
mod frontier;
mod machine;
mod operations;
mod propositions;

use call_graph::validate_call_graph;
pub use error::{ContractClauseKind, ModuleError};
use evidence::{validate_evidence_contract_lanes, validate_proposition_vocabulary};
pub use foundation::{ServiceCeilingOwner, StructuralSignatureOwner};
use foundation::{
    is_nonempty_field_path, partial_affine_residuals, resolve_structural_path,
    validate_structural_foundation,
};

#[derive(Debug, Clone, Copy)]
pub struct ValidatedTerminalModule<'module> {
    module: &'module TerminalModule,
}

impl<'module> ValidatedTerminalModule<'module> {
    pub const fn module(self) -> &'module TerminalModule {
        self.module
    }

    pub fn machine(self, id: MachineId) -> Option<&'module TerminalMachine> {
        self.module.machines.iter().find(|machine| machine.id == id)
    }

    pub fn value_context(
        self,
        machine: &TerminalMachine,
    ) -> Result<PropositionContext, ModuleError> {
        PropositionContext::from_value_types_and_places(
            machine_value_types(machine),
            machine
                .structural_places
                .iter()
                .map(|place| (place.id, place.kind))
                .chain(
                    nominal_cleanup_contract_receiver(self.module, machine.id).map(|receiver| {
                        (
                            receiver,
                            StructuralPlaceKind::Parameter {
                                position: 0,
                                is_self: true,
                            },
                        )
                    }),
                ),
        )
        .map_err(ModuleError::MalformedProposition)
    }
}

fn nominal_cleanup_contract_receiver(
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

fn nominal_cleanups(
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

pub fn validate_module(
    module: &TerminalModule,
) -> Result<ValidatedTerminalModule<'_>, ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Execution)?;
    Ok(ValidatedTerminalModule { module })
}

/// Validate the representation-wide invariants needed by canonical codecs.
///
/// This remains a distinct entry point so canonical representation checks do
/// not themselves confer an execution-grade `ValidatedTerminalModule`.
pub fn validate_module_representation(module: &TerminalModule) -> Result<(), ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Representation)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationPolicy {
    Execution,
    Representation,
}

fn validate_module_with_policy(
    module: &TerminalModule,
    policy: ValidationPolicy,
) -> Result<(), ModuleError> {
    if module.machines.is_empty() {
        return Err(ModuleError::EmptyModule);
    }
    validate_proposition_vocabulary(module)?;
    validate_structural_foundation(module)?;

    let mut registry = IdRegistry::default();
    for machine in &module.machines {
        insert_unique(
            &mut registry.machines,
            machine.id,
            ModuleError::DuplicateMachine,
        )?;
        insert_unique(
            &mut registry.contracts,
            machine.contract.id,
            ModuleError::DuplicateContract,
        )?;
    }
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    validate_evidence_contract_lanes(module, &machines)?;
    for machine in &module.machines {
        machine::validate_machine(module, machine, &machines, &mut registry, policy)?;
    }
    validate_call_graph(module)?;
    if !registry.machines.contains(&module.entry) {
        return Err(ModuleError::UnknownEntryMachine(module.entry));
    }

    Ok(())
}

#[derive(Default)]
struct IdRegistry {
    machines: BTreeSet<MachineId>,
    blocks: BTreeSet<BlockId>,
    contracts: BTreeSet<ContractId>,
    operations: BTreeSet<OperationId>,
    edges: BTreeSet<EdgeId>,
    obligations: BTreeSet<ObligationId>,
    values: BTreeSet<ValueId>,
    places: BTreeSet<PlaceId>,
    content_projection_algebras: BTreeMap<ContentProjectionIdentity, ContentAlgebra>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StructuralRootKey {
    Parameter(u32),
    Result,
    TrivialAffineLocal(u32),
}

fn validate_unit_operation_static(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &psi_terminal::Operation,
) -> Result<(), ModuleError> {
    match &operation.kind {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            if callee.result != TerminalMachineResult::Unit || !callee.parameters.is_empty() {
                return Err(ModuleError::UnitCallTargetHasScalarSignature {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            if structural_arguments.iter().any(|argument| {
                !argument.path.is_empty()
                    && !matches!(
                        argument.path.as_slice(),
                        [StructuralPathSegment::FixedIndex(_)]
                    )
                    && !is_nonempty_field_path(&argument.path)
            }) {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: structural_arguments
                        .iter()
                        .position(|argument| {
                            !argument.path.is_empty()
                                && !matches!(
                                    argument.path.as_slice(),
                                    [StructuralPathSegment::FixedIndex(_)]
                                )
                                && !is_nonempty_field_path(&argument.path)
                        })
                        .unwrap_or_default() as u32,
                });
            }
            let projected = structural_arguments
                .iter()
                .any(|argument| !argument.path.is_empty());
            if projected
                && (machine.result != TerminalMachineResult::Unit
                    || !machine.parameters.is_empty()
                    || machine.structural_parameters.len() != 1
                    || structural_arguments.len() != 1
                    || callee.structural_parameters.len() != 1)
            {
                return Err(ModuleError::ProjectedUnitCallOutsideBoundedSlice {
                    operation: operation.id,
                });
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                true,
            )?;
            if let Some((argument_index, _)) = structural_arguments
                .iter()
                .zip(&callee.structural_parameters)
                .enumerate()
                .find(|(_, (argument, expected))| {
                    !argument.path.is_empty()
                        && (!expected.qualifications.is_empty()
                            || machine
                                .structural_parameters
                                .iter()
                                .find(|actual| actual.place == argument.place)
                                .is_some_and(|actual| !actual.qualifications.is_empty()))
                })
            {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: argument_index as u32,
                });
            }
            validate_unit_call_contract_places(callee, operation.id)?;
            if projected {
                let projected_parameter = callee.structural_parameters[0].place;
                if unit_call_contract_propositions(callee).any(|proposition| {
                    propositions::proposition_content_roots(proposition)
                        .contains(&projected_parameter)
                }) {
                    return Err(
                        ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                            operation: operation.id,
                            callee: callee.id,
                            place: projected_parameter,
                        },
                    );
                }
            }
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
            if requirement_obligations.len() != callee.contract.requires.len() {
                return Err(ModuleError::CallRequirementArityMismatch {
                    operation: operation.id,
                    expected: callee.contract.requires.len(),
                    actual: requirement_obligations.len(),
                });
            }
            validate_unit_call_claim_transfers(
                machine,
                callee,
                structural_arguments,
                claim_transfers,
                operation.id,
            )?;
            validate_unit_call_crash_continuations(
                module,
                machine,
                callee,
                structural_arguments,
                crash_continuations,
                operation.id,
            )?;
        }
        OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            completion_receipts,
            requirement_obligations,
        } => {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .ok_or(ModuleError::UnknownBoundaryCallTarget {
                    operation: operation.id,
                    boundary: *boundary,
                })?;
            if !requirement_obligations.is_empty() {
                return Err(ModuleError::BoundaryStructuralRequirementsMintObligations(
                    operation.id,
                ));
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &boundary.structural_parameters,
                operation.id,
                true,
            )?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &boundary.published_service_ceiling,
            )?;
            validate_boundary_requirements(machine, boundary, structural_arguments, operation.id)?;
            validate_boundary_completion_receipts(
                machine,
                structural_arguments,
                completion_receipts,
                operation.id,
            )?;
        }
        OperationKind::PortWrite { service, .. } => {
            if !module
                .services
                .iter()
                .any(|candidate| candidate.id == *service)
            {
                return Err(ModuleError::UnknownOperationService {
                    operation: operation.id,
                    service: *service,
                });
            }
            if !machine.published_service_ceiling.contains(service) {
                return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
                    operation: operation.id,
                    service: *service,
                });
            }
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return Err(ModuleError::UnknownTrivialAffineLocal {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = place.kind
            else {
                return Err(ModuleError::UnknownTrivialAffineLocal {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
            else {
                return Err(ModuleError::UnknownStructuralType(structural_type));
            };
            if !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                return Err(ModuleError::TrivialAffineLocalRequiresEmptyRecord {
                    operation: operation.id,
                    place: *destination,
                });
            }
        }
        _ => unreachable!("caller selects only structural/effect operations"),
    }
    Ok(())
}

/// Validate the complete bounded representation for a nonempty run of
/// pairwise-disjoint field transfers, followed by disposal of every maximal
/// residual sibling subtree in recursive reverse declaration order. This
/// partition is checked independently of producer facts before the ownership
/// walk relies on the path-sensitive terminator.
fn validate_partial_affine_cleanup_shape(
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
                    .any(|argument| is_nonempty_field_path(&argument.path))
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
    let [partial_block] = partial_returns.as_slice() else {
        return Err(invalid(block.id));
    };
    if partial_block.id != block.id
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
        if argument.place != root.place || !moved_paths.insert(argument.path.clone()) {
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
            || !callee_parameter.qualifications.is_empty()
        {
            return Err(invalid(block.id));
        }
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
    if expected_residuals.is_empty()
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

fn validate_nominal_affine_cleanup_shape(
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

fn bounded_nominal_cleanup_receiver_shape(shape: &StructuralTypeShape) -> bool {
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
                StructuralFieldType::Structural(_) | StructuralFieldType::Erased { .. } => false,
            }
    })
}

fn validate_boolean_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    source: PlaceId,
    field: StructuralFieldId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidBooleanStructuralField {
        operation,
        source,
        field,
    };
    if machine.id != module.entry {
        return Err(invalid());
    }
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .filter(|parameter| {
            parameter.multiplicity == StructuralMultiplicity::Affine
                && parameter.qualifications.is_empty()
        })
        .ok_or_else(invalid)?;
    if machine
        .entry_claims
        .iter()
        .any(|claim| claim.input == source)
        || !machine.content_entry_claims.is_empty()
        || !machine
            .parameters
            .iter()
            .any(|parameter| parameter.scalar_type == ScalarType::Boolean)
        || !every_scalar_return_nominally_cleans(machine, source)
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|candidate| {
                matches!(candidate.kind,
                OperationKind::BooleanStructuralField {
                    source: other_source,
                    field: other_field,
                } if (other_source, other_field) != (source, field))
            })
    {
        return Err(invalid());
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(invalid());
    };
    if !fields.iter().any(|candidate| {
        candidate.id == field
            && !candidate.relevance.is_erased()
            && candidate.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
    }) {
        return Err(invalid());
    }
    Ok(())
}

fn every_scalar_return_nominally_cleans(machine: &TerminalMachine, source: PlaceId) -> bool {
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

fn valid_nominal_cleanup_requirements(
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
            StructuralTypeShape::FixedArray { .. } => None,
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

fn validate_unit_call_contract_places(
    callee: &TerminalMachine,
    operation: OperationId,
) -> Result<(), ModuleError> {
    let parameters = callee
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    for proposition in unit_call_contract_propositions(callee) {
        if let Some(place) = propositions::proposition_content_roots(proposition)
            .into_iter()
            .find(|place| !parameters.contains(place))
        {
            return Err(ModuleError::UnitCallContractPlaceHasNoArgument {
                operation,
                callee: callee.id,
                place,
            });
        }
    }
    Ok(())
}

fn unit_call_contract_propositions(callee: &TerminalMachine) -> impl Iterator<Item = &Proposition> {
    callee
        .contract
        .requires
        .iter()
        .chain(
            callee
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        )
        .chain(
            callee
                .contract
                .crash_routes
                .iter()
                .flat_map(|bucket| &bucket.alternatives)
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => None,
                    CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
                }),
        )
}

fn validate_structural_arguments(
    module: &TerminalModule,
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    operation: OperationId,
    allow_projected: bool,
) -> Result<(), ModuleError> {
    if arguments.len() != expected.len() {
        return Err(ModuleError::StructuralArgumentArityMismatch {
            operation,
            expected: expected.len(),
            actual: arguments.len(),
        });
    }
    for (index, (argument, expected)) in arguments.iter().zip(expected).enumerate() {
        let Some(actual) = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
        else {
            return Err(ModuleError::UnknownStructuralArgument {
                operation,
                argument_index: index as u32,
                place: argument.place,
            });
        };
        if !allow_projected && !argument.path.is_empty() {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        }
        let Some(actual_type) =
            resolve_structural_path(module, actual.structural_type, &argument.path)
        else {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        };
        if actual_type != expected.structural_type {
            return Err(ModuleError::StructuralArgumentTypeMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.structural_type,
                actual: actual_type,
            });
        }
        let actual_multiplicity = if argument.path.is_empty() {
            actual.multiplicity
        } else if expected.multiplicity == StructuralMultiplicity::Affine
            && is_nonempty_field_path(&argument.path)
            && actual.multiplicity == StructuralMultiplicity::Affine
        {
            StructuralMultiplicity::Affine
        } else {
            StructuralMultiplicity::Linear
        };
        if actual_multiplicity != expected.multiplicity {
            return Err(ModuleError::StructuralArgumentMultiplicityMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.multiplicity,
                actual: actual_multiplicity,
            });
        }
        for qualification in &expected.qualifications {
            if !argument.path.is_empty() || !actual.qualifications.contains(qualification) {
                return Err(ModuleError::StructuralArgumentMissingQualification {
                    operation,
                    argument_index: index as u32,
                    domain: *qualification,
                });
            }
        }
    }
    Ok(())
}

fn validate_service_reach(
    operation: OperationId,
    caller: &[ServiceId],
    reached: &[ServiceId],
) -> Result<(), ModuleError> {
    if let Some(service) = reached.iter().find(|service| !caller.contains(service)) {
        return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
            operation,
            service: *service,
        });
    }
    Ok(())
}

fn validate_unit_call_claim_transfers(
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for (argument_index, (argument, parameter)) in arguments
        .iter()
        .zip(&callee.structural_parameters)
        .enumerate()
    {
        if !argument.path.is_empty() {
            let callee_claims = callee
                .entry_claims
                .iter()
                .filter(|claim| claim.input == parameter.place)
                .collect::<Vec<_>>();
            let claim_free_direct_affine = is_nonempty_field_path(&argument.path)
                && parameter.multiplicity == StructuralMultiplicity::Affine
                && callee_claims.is_empty()
                && caller
                    .entry_claims
                    .iter()
                    .all(|claim| claim.input != argument.place);
            if !claim_free_direct_affine
                && !matches!(callee_claims.as_slice(), [claim] if claim.path.is_empty())
            {
                return Err(ModuleError::UnitCallClaimPresenceMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
            if caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
                || callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
            {
                return Err(ModuleError::UnitCallContentClaimMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
        }
        let mut caller_claim_paths = caller
            .entry_claims
            .iter()
            .filter(|claim| claim.input == argument.place && claim.path.starts_with(&argument.path))
            .map(|claim| &claim.path[argument.path.len()..])
            .collect::<Vec<_>>();
        let mut callee_claim_paths = callee
            .entry_claims
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        caller_claim_paths.sort();
        callee_claim_paths.sort();
        if caller_claim_paths != callee_claim_paths {
            return Err(ModuleError::UnitCallClaimPresenceMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
        let mut caller_content = caller
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == argument.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        let mut callee_content = callee
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == parameter.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        caller_content.sort();
        callee_content.sort();
        if caller_content != callee_content {
            return Err(ModuleError::UnitCallContentClaimMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
    }
    let callee_claims = callee
        .entry_claims
        .iter()
        .map(|claim| (claim.claim, claim.input))
        .chain(
            callee
                .content_entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.input.root)),
        )
        .collect::<BTreeMap<_, _>>();
    for (claim, input) in &callee_claims {
        if !callee
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == *input)
        {
            return Err(ModuleError::UnitCallClaimHasNoStructuralArgument {
                operation,
                claim: *claim,
            });
        }
    }
    if transfers.len() != callee_claims.len() {
        return Err(ModuleError::UnitCallClaimTransferCountMismatch {
            operation,
            expected: callee_claims.len(),
            actual: transfers.len(),
        });
    }
    let mut caller_claims = BTreeSet::new();
    for transfer in transfers {
        if !caller_claims.insert(transfer.claim) {
            return Err(ModuleError::DuplicateUnitCallClaimTransfer(operation));
        }
        let Some(argument) = arguments.get(transfer.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: transfer.argument_index,
            });
        };
        let Some((claim_input, claim_path)) = claim_input(caller, transfer.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: transfer.claim,
            });
        };
        let target_place = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
            .map(|parameter| parameter.place);
        let structural_path_matches = claim_path.starts_with(&argument.path)
            && callee.entry_claims.iter().any(|claim| {
                Some(claim.input) == target_place && claim.path == claim_path[argument.path.len()..]
            });
        let content_matches = argument.path.is_empty()
            && caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.claim == transfer.claim && claim.input.root == argument.place)
            && callee
                .content_entry_claims
                .iter()
                .any(|claim| Some(claim.input.root) == target_place);
        if claim_input != argument.place || (!structural_path_matches && !content_matches) {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: transfer.claim,
                argument_index: transfer.argument_index,
            });
        }
    }
    for input in callee_claims.into_values() {
        let argument_index = callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == input)
            .expect("callee entry claims were validated against its signature")
            as u32;
        if !transfers
            .iter()
            .any(|transfer| transfer.argument_index == argument_index)
        {
            return Err(ModuleError::MissingUnitCallClaimTransfer {
                operation,
                argument_index,
            });
        }
    }
    if transfers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalUnitCallClaimTransfers(operation));
    }
    Ok(())
}

fn claim_input(
    machine: &TerminalMachine,
    claim: ClaimId,
) -> Option<(PlaceId, &[StructuralPathSegment])> {
    machine
        .entry_claims
        .iter()
        .find_map(|candidate| {
            (candidate.claim == claim).then_some((candidate.input, candidate.path.as_slice()))
        })
        .or_else(|| {
            machine.content_entry_claims.iter().find_map(|candidate| {
                (candidate.claim == claim)
                    .then_some((candidate.input.root, &[] as &[StructuralPathSegment]))
            })
        })
}

fn validate_unit_call_crash_continuations(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    continuations: &[CrashRouteBucket],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let boolean_roots = callee
        .contract
        .crash_routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
        .flat_map(propositions::proposition_boolean_field_roots)
        .collect::<BTreeSet<_>>();
    let substitutions = callee
        .structural_parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let prefix = structural_argument_canonical_prefix(module, caller, argument);
            if prefix.is_none() && boolean_roots.contains(&parameter.place) {
                return Err(
                    ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                        operation,
                        callee: callee.id,
                        place: parameter.place,
                    },
                );
            }
            Ok((
                parameter.place,
                (argument.place, prefix.unwrap_or_default()),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let expected = substitute_crash_route_places(&callee.contract.crash_routes, &substitutions);
    if continuations != expected {
        return Err(ModuleError::CallCrashContinuationsMismatch {
            operation,
            callee: callee.id,
        });
    }
    for continuation in continuations {
        let covered = caller.contract.crash_routes.iter().any(|published| {
            published.cause == continuation.cause
                && (published.alternatives == [CrashRouteGuard::Truth]
                    || continuation
                        .alternatives
                        .iter()
                        .all(|route| published.alternatives.contains(route)))
        });
        if !covered {
            return Err(ModuleError::CallCrashContinuationUncovered {
                operation,
                cause: continuation.cause,
            });
        }
    }
    Ok(())
}

fn substitute_crash_route_places(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .map(|guard| match guard {
                    CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                    CrashRouteGuard::Predicate(predicate) => CrashRouteGuard::Predicate(
                        CrashPredicateTerm::new(substitute_proposition_structural_places(
                            predicate.proposition(),
                            substitutions,
                        )),
                    ),
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            }
        })
        .collect()
}

pub(crate) fn structural_argument_canonical_prefix(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut structural_type = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)?
        .structural_type;
    let mut prefix = Vec::with_capacity(argument.path.len());
    for segment in &argument.path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                let field = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match &declaration.shape {
                        StructuralTypeShape::Record { fields } => fields.iter().find(|field| {
                            field.identity == *identity && !field.relevance.is_erased()
                        }),
                        StructuralTypeShape::FixedArray { .. } => None,
                    })?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                structural_type = next;
            }
            StructuralPathSegment::FixedIndex(index) => {
                let element = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::FixedArray { element, length } if *index < length => {
                            Some(element)
                        }
                        _ => None,
                    })?;
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    Some(prefix)
}

fn validate_boundary_requirements(
    caller: &TerminalMachine,
    boundary: &BoundaryMachineDeclaration,
    arguments: &[StructuralArgument],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for requirement in &boundary.requires {
        let argument = &arguments[requirement.argument_index as usize];
        let actual = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .expect("structural arguments were validated before requirements");
        if !actual.qualifications.contains(&requirement.domain) {
            return Err(ModuleError::BoundaryArgumentMissingQualification {
                operation,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

fn validate_boundary_completion_receipts(
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller
                .entry_claims
                .iter()
                .filter_map(move |claim| {
                    (claim.input == argument.place
                        && (argument.path.is_empty() || claim.path == argument.path))
                        .then_some((index as u32, claim.claim))
                })
                .chain(caller.content_entry_claims.iter().filter_map(move |claim| {
                    (claim.input.root == argument.place).then_some((index as u32, claim.claim))
                }))
        })
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    let mut claims = BTreeSet::new();
    for receipt in receipts {
        if !actual.insert((receipt.argument_index, receipt.claim)) || !claims.insert(receipt.claim)
        {
            return Err(ModuleError::DuplicateBoundaryCompletionReceipt(operation));
        }
        let Some(argument) = arguments.get(receipt.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: receipt.argument_index,
            });
        };
        let Some((claim_input, claim_path)) = claim_input(caller, receipt.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: receipt.claim,
            });
        };
        if claim_input != argument.place
            || (!argument.path.is_empty() && claim_path != argument.path.as_slice())
        {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: receipt.claim,
                argument_index: receipt.argument_index,
            });
        }
    }
    if actual != expected {
        return Err(ModuleError::BoundaryCompletionReceiptMismatch(operation));
    }
    if receipts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalBoundaryCompletionReceipts(
            operation,
        ));
    }
    Ok(())
}

fn substitute_crash_routes(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .filter_map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => Some(CrashRouteGuard::Truth),
                    CrashRouteGuard::Predicate(predicate) => {
                        match substitute_proposition_values(predicate.proposition(), substitutions)
                        {
                            Proposition::Truth => Some(CrashRouteGuard::Truth),
                            Proposition::Falsehood => None,
                            proposition => Some(CrashRouteGuard::Predicate(
                                CrashPredicateTerm::new(proposition),
                            )),
                        }
                    }
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            (!alternatives.is_empty()).then_some(CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            })
        })
        .collect()
}

fn validate_crash_frontiers(
    module: &TerminalModule,
    machine: &TerminalMachine,
    context: &PropositionContext,
    contract_values: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if machine
        .contract
        .crash_routes
        .windows(2)
        .any(|pair| pair[0].cause >= pair[1].cause)
    {
        return Err(ModuleError::NonCanonicalCrashRoutes(machine.id));
    }
    for bucket in &machine.contract.crash_routes {
        if bucket.alternatives.is_empty() {
            return Err(ModuleError::EmptyCrashRouteBucket {
                machine: machine.id,
                cause: bucket.cause,
            });
        }
        if bucket
            .alternatives
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || (bucket.alternatives.contains(&CrashRouteGuard::Truth)
                && bucket.alternatives != [CrashRouteGuard::Truth])
        {
            return Err(ModuleError::NonCanonicalCrashRouteAlternatives {
                machine: machine.id,
                cause: bucket.cause,
            });
        }
        for guard in &bucket.alternatives {
            let CrashRouteGuard::Predicate(predicate) = guard else {
                continue;
            };
            if matches!(
                predicate.proposition(),
                Proposition::Truth | Proposition::Falsehood
            ) {
                return Err(ModuleError::NonCanonicalCrashRouteAlternatives {
                    machine: machine.id,
                    cause: bucket.cause,
                });
            }
            context
                .validate(predicate.proposition())
                .map_err(ModuleError::MalformedProposition)?;
            validate_boolean_field_terms(
                module,
                machine,
                predicate.proposition(),
                &machine.contract.requires,
            )?;
            contracts::validate_contract_scope(
                predicate.proposition(),
                contract_values,
                machine.contract.id,
                ContractClauseKind::Crash,
            )?;
        }
    }
    for block in &machine.blocks {
        let Terminator::Crash {
            cause,
            site_guard,
            frontier_lower_bound,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if site_guard.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ModuleError::NonCanonicalCrashSiteGuard(block.id));
        }
        for predicate in site_guard {
            if matches!(
                predicate.proposition(),
                Proposition::Truth | Proposition::Falsehood
            ) {
                return Err(ModuleError::NonCanonicalCrashSiteGuard(block.id));
            }
            context
                .validate(predicate.proposition())
                .map_err(ModuleError::MalformedProposition)?;
            validate_boolean_field_terms(
                module,
                machine,
                predicate.proposition(),
                &machine.contract.requires,
            )?;
        }
        let covered = machine
            .contract
            .crash_routes
            .iter()
            .filter(|bucket| bucket.cause == *cause)
            .any(|bucket| {
                bucket.alternatives.iter().any(|route| match route {
                    CrashRouteGuard::Truth => true,
                    CrashRouteGuard::Predicate(predicate) => site_guard.contains(predicate),
                })
            });
        if !covered {
            return Err(ModuleError::CrashRouteUncovered {
                block: block.id,
                cause: *cause,
            });
        }
        if frontier_lower_bound
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalCrashFrontier(block.id));
        }
    }
    Ok(())
}

fn validate_boolean_field_terms(
    module: &TerminalModule,
    machine: &TerminalMachine,
    proposition: &Proposition,
    runtime_requirements: &[Proposition],
) -> Result<(), ModuleError> {
    fn validate_term(
        module: &TerminalModule,
        machine: &TerminalMachine,
        term: &ScalarTerm,
        runtime_requirements: &[Proposition],
    ) -> Result<(), ModuleError> {
        fn safe_exact_divisor(
            integer_type: IntegerType,
            dividend: &ScalarTerm,
            divisor: &ScalarTerm,
            requirements: &[Proposition],
        ) -> bool {
            match divisor {
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(value),
                } => return *scalar_type == integer_type && *value != 0,
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Signed(value),
                } => return *scalar_type == integer_type && *value != 0 && *value != -1,
                _ => {}
            }
            let one = match integer_type.sign() {
                IntegerSign::Unsigned => IntegerValue::Unsigned(1),
                IntegerSign::Signed => IntegerValue::Signed(1),
            };
            if ScalarTerm::integer(integer_type, one).is_ok_and(|one| {
                requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
            }) {
                return true;
            }
            if integer_type.sign() != IntegerSign::Signed {
                return false;
            }
            if ScalarTerm::integer(integer_type, IntegerValue::Signed(-2)).is_ok_and(
                |negative_two| {
                    requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_two))
                },
            ) {
                return true;
            }
            let negative_one = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1))
                .expect("every signed fixed integer admits negative one");
            if !requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_one)) {
                return false;
            }
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed fixed integer has a signed minimum")
            };
            ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
            )
            .is_ok_and(|minimum_plus_one| {
                requirements.contains(&Proposition::LessOrEqual(
                    minimum_plus_one,
                    dividend.clone(),
                ))
            })
        }

        fn safe_policy_divisor(
            integer_type: IntegerType,
            divisor: &ScalarTerm,
            requirements: &[Proposition],
        ) -> bool {
            match divisor {
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(value),
                } => return *scalar_type == integer_type && *value != 0,
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Signed(value),
                } => return *scalar_type == integer_type && *value != 0,
                _ => {}
            }
            let one = match integer_type.sign() {
                IntegerSign::Unsigned => IntegerValue::Unsigned(1),
                IntegerSign::Signed => IntegerValue::Signed(1),
            };
            if ScalarTerm::integer(integer_type, one).is_ok_and(|one| {
                requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
            }) {
                return true;
            }
            if integer_type.sign() != IntegerSign::Signed {
                return false;
            }
            [IntegerValue::Signed(-1), IntegerValue::Signed(-2)]
                .into_iter()
                .filter_map(|bound| ScalarTerm::integer(integer_type, bound).ok())
                .any(|bound| {
                    requirements.contains(&Proposition::LessOrEqual(divisor.clone(), bound))
                })
        }

        fn nonnegative_shift_count(value: IntegerValue) -> Option<u32> {
            match value {
                IntegerValue::Unsigned(value) => u32::try_from(value).ok(),
                IntegerValue::Signed(value) => u32::try_from(value).ok(),
            }
        }

        fn exact_shift_maximum_count(
            value_type: IntegerType,
            count_type: IntegerType,
            count: &ScalarTerm,
            requirements: &[Proposition],
        ) -> Option<u32> {
            if count.scalar_type() != ScalarType::Integer(count_type) {
                return None;
            }
            if let Some((literal_type, literal)) = count.integer_value() {
                let literal = nonnegative_shift_count(literal)?;
                return (literal_type == count_type && literal < u32::from(value_type.bits()))
                    .then_some(literal);
            }
            if count_type.sign() == IntegerSign::Signed {
                let zero = ScalarTerm::integer(count_type, IntegerValue::Signed(0)).ok()?;
                if !requirements.contains(&Proposition::LessOrEqual(zero, count.clone())) {
                    return None;
                }
            }
            let width = u32::from(value_type.bits());
            let intrinsic_maximum = nonnegative_shift_count(count_type.maximum_value())?;
            if intrinsic_maximum < width {
                return Some(intrinsic_maximum);
            }
            requirements
                .iter()
                .filter_map(|requirement| match requirement {
                    Proposition::LessOrEqual(left, right) if left == count => {
                        let (right_type, right) = right.integer_value()?;
                        let right = nonnegative_shift_count(right)?;
                        (right_type == count_type && right < width).then_some(right)
                    }
                    Proposition::LessThan(left, right) if left == count => {
                        let (right_type, right) = right.integer_value()?;
                        let right = nonnegative_shift_count(right)?;
                        (right_type == count_type && right > 0 && right <= width)
                            .then_some(right - 1)
                    }
                    _ => None,
                })
                .min()
        }

        fn safe_exact_shift(
            left_shift: bool,
            value_type: IntegerType,
            count_type: IntegerType,
            value: &ScalarTerm,
            count: &ScalarTerm,
            requirements: &[Proposition],
        ) -> bool {
            if value.scalar_type() != ScalarType::Integer(value_type) {
                return false;
            }
            let Some(maximum_count) =
                exact_shift_maximum_count(value_type, count_type, count, requirements)
            else {
                return false;
            };
            if !left_shift || maximum_count == 0 {
                return true;
            }
            if let Some((literal_type, literal)) = value.integer_value() {
                let maximum_count_value = match count_type.sign() {
                    IntegerSign::Signed => IntegerValue::Signed(i128::from(maximum_count)),
                    IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(maximum_count)),
                };
                return literal_type == value_type
                    && value_type
                        .exact_shift_left(literal, count_type, maximum_count_value)
                        .is_some();
            }
            match value_type.sign() {
                IntegerSign::Unsigned => {
                    let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                        unreachable!("unsigned fixed integer has an unsigned maximum")
                    };
                    ScalarTerm::integer(
                        value_type,
                        IntegerValue::Unsigned(maximum >> maximum_count),
                    )
                    .is_ok_and(|maximum| {
                        requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
                    })
                }
                IntegerSign::Signed => {
                    let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                        (value_type.minimum_value(), value_type.maximum_value())
                    else {
                        unreachable!("signed fixed integer has signed bounds")
                    };
                    let minimum = ScalarTerm::integer(
                        value_type,
                        IntegerValue::Signed(minimum >> maximum_count),
                    );
                    let maximum = ScalarTerm::integer(
                        value_type,
                        IntegerValue::Signed(maximum >> maximum_count),
                    );
                    minimum.is_ok_and(|minimum| {
                        requirements.contains(&Proposition::LessOrEqual(minimum, value.clone()))
                    }) && maximum.is_ok_and(|maximum| {
                        requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
                    })
                }
            }
        }

        match term {
            ScalarTerm::BooleanField { root, path } => {
                let mut structural_type = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == *root)
                    .map(|parameter| parameter.structural_type);
                let mut valid = !path.is_empty();
                for (index, segment) in path.iter().enumerate() {
                    let Some(current_type) = structural_type else {
                        valid = false;
                        break;
                    };
                    let is_last = index + 1 == path.len();
                    match segment {
                        CanonicalStructuralPathSegment::Field(field_id) => {
                            let field = module
                                .structural_types
                                .iter()
                                .find(|declaration| declaration.id == current_type)
                                .and_then(|declaration| match &declaration.shape {
                                    StructuralTypeShape::Record { fields } => {
                                        fields.iter().find(|candidate| candidate.id == *field_id)
                                    }
                                    StructuralTypeShape::FixedArray { .. } => None,
                                });
                            let Some(field) = field.filter(|field| !field.relevance.is_erased())
                            else {
                                valid = false;
                                break;
                            };
                            match (&field.field_type, is_last) {
                                (StructuralFieldType::Structural(next), false) => {
                                    structural_type = Some(*next);
                                }
                                (StructuralFieldType::Scalar(ScalarType::Boolean), true) => {
                                    structural_type = None;
                                }
                                _ => {
                                    valid = false;
                                    break;
                                }
                            }
                        }
                        CanonicalStructuralPathSegment::FixedIndex(fixed_index) => {
                            let element = module
                                .structural_types
                                .iter()
                                .find(|declaration| declaration.id == current_type)
                                .and_then(|declaration| match declaration.shape {
                                    StructuralTypeShape::FixedArray { element, length }
                                        if *fixed_index < length =>
                                    {
                                        Some(element)
                                    }
                                    _ => None,
                                });
                            let Some(element) = element.filter(|_| !is_last) else {
                                valid = false;
                                break;
                            };
                            structural_type = Some(element);
                        }
                    }
                }
                if !valid {
                    return Err(ModuleError::InvalidBooleanFieldTerm {
                        machine: machine.id,
                        root: *root,
                        path: path.clone(),
                    });
                }
            }
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
            } => {
                let mut structural_type = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == *root)
                    .map(|parameter| parameter.structural_type);
                let mut valid = !path.is_empty();
                for (index, segment) in path.iter().enumerate() {
                    let Some(current_type) = structural_type else {
                        valid = false;
                        break;
                    };
                    let is_last = index + 1 == path.len();
                    match segment {
                        CanonicalStructuralPathSegment::Field(field_id) => {
                            let field = module
                                .structural_types
                                .iter()
                                .find(|declaration| declaration.id == current_type)
                                .and_then(|declaration| match &declaration.shape {
                                    StructuralTypeShape::Record { fields } => {
                                        fields.iter().find(|candidate| candidate.id == *field_id)
                                    }
                                    StructuralTypeShape::FixedArray { .. } => None,
                                });
                            let Some(field) = field.filter(|field| !field.relevance.is_erased())
                            else {
                                valid = false;
                                break;
                            };
                            match (&field.field_type, is_last) {
                                (StructuralFieldType::Structural(next), false) => {
                                    structural_type = Some(*next);
                                }
                                (
                                    StructuralFieldType::Scalar(ScalarType::Integer(actual)),
                                    true,
                                ) if actual == scalar_type => {
                                    structural_type = None;
                                }
                                _ => {
                                    valid = false;
                                    break;
                                }
                            }
                        }
                        CanonicalStructuralPathSegment::FixedIndex(fixed_index) => {
                            let element = module
                                .structural_types
                                .iter()
                                .find(|declaration| declaration.id == current_type)
                                .and_then(|declaration| match declaration.shape {
                                    StructuralTypeShape::FixedArray { element, length }
                                        if *fixed_index < length =>
                                    {
                                        Some(element)
                                    }
                                    _ => None,
                                });
                            let Some(element) = element.filter(|_| !is_last) else {
                                valid = false;
                                break;
                            };
                            structural_type = Some(element);
                        }
                    }
                }
                if !valid {
                    return Err(ModuleError::InvalidIntegerFieldTerm {
                        machine: machine.id,
                        root: *root,
                        path: path.clone(),
                        scalar_type: *scalar_type,
                    });
                }
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                validate_term(module, machine, operand, runtime_requirements)?;
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                if !safe_policy_divisor(*scalar_type, right, runtime_requirements) {
                    return Err(ModuleError::UnsafeStructuralCrashPolicyDivisor {
                        machine: machine.id,
                        scalar_type: *scalar_type,
                    });
                }
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                if !safe_exact_divisor(*scalar_type, left, right, runtime_requirements) {
                    return Err(ModuleError::UnsafeStructuralCrashExactDivisor {
                        machine: machine.id,
                        scalar_type: *scalar_type,
                    });
                }
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
                validate_term(module, machine, value, runtime_requirements)?;
                validate_term(module, machine, count, runtime_requirements)?;
            }
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => {
                let left_shift = matches!(term, ScalarTerm::ExactIntegerShiftLeft { .. });
                if !safe_exact_shift(
                    left_shift,
                    *value_type,
                    *count_type,
                    value,
                    count,
                    runtime_requirements,
                ) {
                    return Err(ModuleError::UnsafeStructuralCrashExactShift {
                        machine: machine.id,
                        value_type: *value_type,
                        count_type: *count_type,
                        left_shift,
                    });
                }
                validate_term(module, machine, value, runtime_requirements)?;
                validate_term(module, machine, count, runtime_requirements)?;
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
        Ok(())
    }

    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term(module, machine, left, runtime_requirements)?;
            validate_term(module, machine, right, runtime_requirements)?;
        }
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_boolean_field_terms(module, machine, proposition, runtime_requirements)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_boolean_field_terms(module, machine, premise, runtime_requirements)?;
            validate_boolean_field_terms(module, machine, conclusion, runtime_requirements)?;
        }
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => {}
    }
    Ok(())
}

pub(crate) fn machine_value_types(
    machine: &TerminalMachine,
) -> impl Iterator<Item = (ValueId, ScalarType)> + '_ {
    machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|declaration| (declaration.id, declaration.scalar_type))
}

fn insert_value(
    values: &mut BTreeMap<ValueId, ScalarType>,
    module_values: &mut BTreeSet<ValueId>,
    id: ValueId,
    scalar_type: ScalarType,
) -> Result<(), ModuleError> {
    if values.insert(id, scalar_type).is_some() || !module_values.insert(id) {
        return Err(ModuleError::DuplicateValue(id));
    }
    Ok(())
}

fn insert_unique<T: Ord + Copy>(
    set: &mut BTreeSet<T>,
    value: T,
    error: impl FnOnce(T) -> ModuleError,
) -> Result<(), ModuleError> {
    if !set.insert(value) {
        return Err(error(value));
    }
    Ok(())
}
